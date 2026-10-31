//! Sprites et tuiles: chargement des planches, dessin de la salle, animation des personnages.
//!
//! Les planches sont générées par `tools/gen_sprites.py` (placeholders). Chaque case fait 16x16.

use std::f32::consts::FRAC_PI_4;

use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    prelude::*,
    sprite::Anchor,
};
use castle_shared::*;

/// Planche des héros (rendue par `tools/sprite_forge`): une ligne par classe et par direction (8),
/// 51 images par ligne, lues à 25 images par seconde: 25 de repos (respiration, cycle d'une seconde), 16 de marche puis
/// 10 d'attaque (0,4 s).
pub const HERO_IDLE_FRAMES: usize = 25;
pub const HERO_WALK_FRAMES: usize = 16;
pub const HERO_ATTACK_FRAMES: usize = 10;
/// Images par ligne, pour les héros comme pour les ennemis (repos, marche, attaque).
const HERO_FRAMES: usize = HERO_IDLE_FRAMES + HERO_WALK_FRAMES + HERO_ATTACK_FRAMES;
/// Cadence de toutes les animations lues à l'horloge: repos, battement d'ailes, objets (images par seconde).
const ANIMATION_FPS: f32 = 25.0;
/// Durée du geste d'une attaque, en secondes (la frappe tombe vers 60 % du geste).
const ATTACK_DURATION: f32 = 0.4;
/// Durée du geste d'attaque d'un ennemi: plus court, il enchaîne les coups.
const MONSTER_ATTACK_DURATION: f32 = 0.4;
/// Côté d'une case d'un personnage, d'un monstre ou d'un objet dans sa planche (voir `DETAIL` dans `tools/sprite_forge`).

/// Colonnes des planches de personnages et de monstres: une direction fait trois lignes (voir `tools/sprite_forge`).
const SHEET_COLUMNS: u32 = 17;
const SHEET_ROWS: u32 = 8 * HERO_FRAMES as u32 / SHEET_COLUMNS;
/// Taille d'affichage d'un héros, d'un ennemi ou d'un objet, en unités du monde (la case fait 96 px d'art).
pub const SPRITE_SIZE: f32 = 32.0;
/// Les ennemis et les objets partagent le découpage de la planche des héros pour les ennemis (8 directions,
/// 51 images), et 20 images d'animation par objet.
const PROP_FRAMES: usize = 20;
use bevy::asset::RenderAssetUsages;
use castle_shared::tileart::{TILE_CELL, TILE_COLUMNS, TILE_ROWS, tile_column};
/// Distance parcourue pendant un cycle de marche complet (8 images), en unités du monde.
const HERO_STRIDE: f32 = 44.0;

/// Jeu de sprites, d'animations et de décors en cours: aventure colorée (manga) ou ambiance sanglante.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArtTheme {
    Manga,
    Gore,
    Medieval,
}

impl ArtTheme {
    fn folder(self) -> &'static str {
        match self {
            Self::Manga => "manga",
            Self::Gore => "gore",
            Self::Medieval => "medieval",
        }
    }

    /// Le thème suivant dans le cycle de la touche F2: manga, sanglant, médiéval, puis retour au manga.
    pub fn next(self) -> Self {
        match self {
            Self::Manga => Self::Gore,
            Self::Gore => Self::Medieval,
            Self::Medieval => Self::Manga,
        }
    }

    /// Le jeu n'utilise que le mode médiéval; les autres jeux de sprites restent désactivés.
    fn initial() -> Self {
        Self::Medieval
    }
}

/// Les planches de tous les thèmes, déjà chargées: F2 passe de l'un à l'autre sans attendre.
#[derive(Resource)]
pub struct ArtLibrary(Vec<(ArtTheme, Art)>);

/// Marque une trace de sang posée au sol (effacée en quittant le mode sanglant).
#[derive(Component)]
pub struct Stain;

pub struct ArtPlugin;

impl Plugin for ArtPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ThemeTransition>()
            .add_systems(Startup, (load_art, spawn_transition_overlay))
            .add_systems(
                Update,
                (log_big_images, start_transition, run_transition, rebuild_world, tint_tiles, animate_heroes, animate_monsters, animate_props).chain(),
            );
    }
}

#[derive(Clone)]
pub struct Atlas {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

impl Atlas {
    /// Sprite de la case `index`, affiché à la taille donnée (en unités du monde).
    pub fn sprite_sized(&self, index: usize, size: Vec2) -> Sprite {
        Sprite {
            image: self.image.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: self.layout.clone(),
                index,
            }),
            custom_size: Some(size),
            ..default()
        }
    }
}

#[derive(Resource, Clone)]
pub struct Art {
    pub tiles: Atlas,
    /// Sorties de niveau, dessinées en grand (cases de deux fois `CELL` pixels).
    pub exits: Atlas,
    pub projectiles: Atlas,
    pub fx: Atlas,
    pub decals: Atlas,
    /// Vignettes des quatre héros (une case de 128 px par classe).
    pub portraits: Atlas,
    /// Décor posé sur le sol (herbes, cailloux...): une image lue par le processeur, pour être assemblée au sol de chaque bloc.
    pub decor: Handle<Image>,
    /// Une planche par personnage, par monstre et par objet, chargée à la première demande (un monstre absent du niveau ne
    /// prend pas de mémoire graphique).
    sheets: Sheets,
}

/// Les planches individuelles: dossier du thème, découpage de chacune, et planches déjà demandées.
#[derive(Clone)]
struct Sheets {
    assets: AssetServer,
    folder: String,
    /// Côté de la case d'origine, en pixels (voir `sprite_meta`).
    cell: u32,
    /// Découpage de chaque planche (`dossier/nom`), et zone de la case d'origine qu'elle conserve.
    layouts: std::collections::HashMap<String, (Handle<TextureAtlasLayout>, SheetCrop)>,
    loaded: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Handle<Image>>>>,
}

/// Zone conservée dans les cases d'une planche: les images sont rognées à ce qu'elles dessinent (voir `sprite_meta`).
#[derive(Clone, Copy)]
struct SheetCrop {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Sheets {
    /// L'image de la planche `art/<thème>/<dossier>/<nom>.basis` (texture compressée, décompressée vers le format de la carte
    /// graphique au chargement).
    fn image(&self, dir: &str, name: &str) -> Handle<Image> {
        let key = format!("{dir}/{name}");
        let mut loaded = self.loaded.lock().unwrap();
        loaded
            .entry(key.clone())
            .or_insert_with(|| {
                self.assets
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        settings.sampler = ImageSampler::linear();
                        // Les planches ne servent qu'à la carte graphique: on ne garde pas de copie en mémoire centrale.
                        settings.asset_usage = RenderAssetUsages::RENDER_WORLD;
                    })
                    .load(format!("art/{}/{key}.basis", self.folder))
            })
            .clone()
    }

    /// Le sprite de la case `index` de la planche `dir/name`, affiché comme le serait la case entière à la taille `size`
    /// (la zone rognée garde la même échelle), avec l'ancrage `base` d'une case entière transposé à la zone rognée.
    fn sprite(&self, dir: &str, name: &str, index: usize, size: Vec2, base: Anchor) -> (Sprite, Anchor) {
        let (layout, crop) = &self.layouts[&format!("{dir}/{name}")];
        let cell = self.cell as f32;
        let (w, h) = (crop.w as f32, crop.h as f32);
        let anchor = crop_anchor(base, *crop, self.cell);
        let sprite = Sprite {
            image: self.image(dir, name),
            texture_atlas: Some(TextureAtlas { layout: layout.clone(), index }),
            custom_size: Some(Vec2::new(w * size.x / cell, h * size.y / cell)),
            ..default()
        };
        (sprite, anchor)
    }
}

/// L'ancrage, pour une zone rognée de la case, qui garde l'origine de l'entité au même point du dessin que `base` dans la
/// case entière.
fn crop_anchor(base: Anchor, crop: SheetCrop, cell: u32) -> Anchor {
    let cell = cell as f32;
    let (w, h) = (crop.w as f32, crop.h as f32);
    // Le point d'origine de la case entière (en pixels, y vers le bas), puis par rapport au centre de la zone rognée.
    let origin = Vec2::new(cell / 2.0 + base.0.x * cell, cell / 2.0 - base.0.y * cell);
    let center = Vec2::new(crop.x as f32 + w / 2.0, crop.y as f32 + h / 2.0);
    Anchor(Vec2::new((origin.x - center.x) / w, -(origin.y - center.y) / h))
}

/// L'ancrage d'un objet: posé au sol, sauf les panneaux de mur, vus de face et centrés sur leur tuile.
fn prop_base_anchor(row: usize) -> Anchor {
    if (object::WALL_RUNE..=object::WALL_TARGET_HIT).contains(&row) { WALL_ANCHOR } else { PROP_ANCHOR }
}

impl Art {
    /// Sprite d'un héros (direction et image de sa planche), affiché à la taille donnée, et son ancrage.
    pub fn hero_sprite(&self, class: HeroClass, direction: usize, frame: usize, size: Vec2) -> (Sprite, Anchor) {
        self.sheets.sprite("heroes", &file_name(class), sheet_index(direction, frame), size, HERO_ANCHOR)
    }

    pub fn monster_sprite(&self, kind: MonsterKind, direction: usize, frame: usize, size: Vec2) -> (Sprite, Anchor) {
        self.sheets.sprite("monsters", &file_name(kind), sheet_index(direction, frame), size, PROP_ANCHOR)
    }

    /// Sprite d'un grand motif (arbre, colonne, fontaine...) et son ancrage: une image unique dont le pied est l'origine.
    pub fn landmark_sprite(&self, kind: LandmarkKind) -> (Sprite, Anchor) {
        let size = 64.0 * kind.display_scale();
        let sprite = Sprite { image: self.sheets.image("landmarks", kind.file()), custom_size: Some(Vec2::splat(size)), ..default() };
        (sprite, PROP_ANCHOR)
    }

    /// Sprite d'un objet (ligne `object::*`, image d'animation) et son ancrage.
    pub fn prop_sprite(&self, row: usize, frame: usize, size: Vec2) -> (Sprite, Anchor) {
        self.sheets.sprite("objects", object::FILES[row], frame, size, prop_base_anchor(row))
    }
}

/// Nom du fichier d'une planche: le nom du personnage ou du monstre en minuscules (comme `tools/sprite_forge`).
fn file_name(name: impl std::fmt::Debug) -> String {
    format!("{name:?}").to_lowercase()
}

/// Case d'une direction et d'une image dans la planche d'un personnage ou d'un monstre.
fn sheet_index(direction: usize, frame: usize) -> usize {
    direction * HERO_FRAMES + frame
}

/// Objets: leur numéro est leur place dans `PROPS` (`tools/sprite_forge`); chacun a sa propre planche.
pub mod object {
    pub const GENERATOR: usize = 0;
    pub const TURRET: usize = 1;
    pub const COIN: usize = 2;
    pub const FOOD: usize = 3;
    pub const KEY: usize = 4;
    // Les lignes 5 (levier) et 7 (mécanisme) de la planche sont des objets au sol que l'énigme n'utilise plus.
    pub const TRAP_ARMED: usize = 8;
    pub const TRAP_OFF: usize = 9;
    pub const DOOR_LOCKED: usize = 10;
    pub const DOOR_ROYAL: usize = 11;
    pub const DOOR_ROYAL_EMBLEM: usize = 12;
    pub const POTION: usize = 14;
    pub const WALL_RUNE: usize = 15;
    pub const WALL_GEARS_JAMMED: usize = 16;
    pub const WALL_GEARS_FIXED: usize = 17;
    pub const WALL_TARGET_EMPTY: usize = 18;
    pub const WALL_TARGET_MOUNTED: usize = 19;
    pub const WALL_TARGET_HIT: usize = 20;
    pub const PORTCULLIS: usize = 21;
    pub const INGREDIENT: usize = 22;
    pub const GEAR_PART: usize = 23;
    pub const TARGET_DISC: usize = 24;
    pub const CAPTIVE: usize = 25;

    /// Nom du fichier de planche de chaque objet (même ordre que `PROPS` dans `tools/sprite_forge`).
    pub const FILES: [&str; 26] = [
        "generator",
        "turret",
        "coin",
        "food",
        "key",
        "lever",
        "rune",
        "mechanism",
        "traparmed",
        "trapoff",
        "doorlocked",
        "doorroyal",
        "doorroyalemblem",
        "stairs",
        "potion",
        "wallrune",
        "wallgearsjammed",
        "wallgearsfixed",
        "walltargetempty",
        "walltargetmounted",
        "walltargethit",
        "portcullis",
        "ingredient",
        "gearpart",
        "targetdisc",
        "captivecage",
    ];
}

/// Les sorties se dessinent en grand sur leur propre planche: cases de deux fois la case d'un personnage, dix images d'animation,
/// une ligne par état.
pub const EXIT_FRAMES: usize = 10;
/// Taille d'affichage d'une sortie, en unités du monde (quatre tuiles).
pub const EXIT_SIZE: f32 = 64.0;

/// La ligne de la planche des sorties qui dessine une sortie, selon son genre et si elle est ouverte.
pub fn exit_row(kind: ExitKind, open: bool) -> usize {
    match (kind, open) {
        (ExitKind::Stairs, _) => 0,
        (ExitKind::Gate, false) => 1,
        (ExitKind::Gate, true) => 2,
        (ExitKind::Bridge, false) => 3,
        (ExitKind::Bridge, true) => 4,
        (ExitKind::Drawbridge, false) => 5,
        (ExitKind::Drawbridge, true) => 6,
    }
}

/// Case de la planche des sorties pour une ligne et une image d'animation.
pub fn exit_index(row: usize, frame: usize) -> usize {
    row * EXIT_FRAMES + frame
}

/// Les grosses bêtes sont dessinées un peu plus grandes que les autres (rapport à la taille d'un sprite ordinaire).
pub fn monster_scale(kind: MonsterKind) -> f32 {
    match kind {
        MonsterKind::Wolf => 1.1,
        MonsterKind::Orc => 1.15,
        // Les boss dominent l'écran.
        MonsterKind::Warg => 3.1,
        MonsterKind::Troll => 3.0,
        MonsterKind::Lich => 2.0,
        _ => 1.0,
    }
}

/// Cases de la planche de projectiles (dessinés vers la droite, orientés par le jeu).
pub mod projectile {
    pub const ORB_A: usize = 0;
    pub const ORB_B: usize = 1;
    pub const ARROW: usize = 2;
    pub const BOLT: usize = 3;
}

/// Classe -> numéro de classe dans la planche des héros.
pub fn class_row(class: HeroClass) -> usize {
    match class {
        HeroClass::Knight => 0,
        HeroClass::Sorcerer => 1,
        HeroClass::Ranger => 2,
        HeroClass::Forgebraise => 3,
    }
}

/// Les héros font 96 px pour une boîte de collision de 12 unités: le sprite est remonté pour que
/// les pieds, situés vers le bas de la case, restent au bas de la boîte.
pub const HERO_ANCHOR: Anchor = Anchor(Vec2::new(0.0, -0.228));

/// Direction parmi les huit de la planche (0 = est, puis sens trigonométrique, 6 = sud).
pub fn direction_index(facing: Vec2) -> usize {
    ((facing.to_angle() / FRAC_PI_4).round() as i32).rem_euclid(8) as usize
}



/// Les objets et les ennemis posent leur point d'origine au sol, vers le bas de la case.
pub const PROP_ANCHOR: Anchor = Anchor(Vec2::new(0.0, -0.417));

/// Les panneaux de mur sont dessinés centrés dans leur case (vus de face), pas posés au sol.
pub const WALL_ANCHOR: Anchor = Anchor(Vec2::ZERO);

/// Une entité dessinée devant ou derrière les autres selon sa position vers le haut ou le bas de l'écran (les héros, les monstres
/// et les grands motifs): celle qui est plus bas est devant. Le nombre est une petite avance pour départager deux entités au même endroit.
#[derive(Component)]
pub struct YSort(pub f32);

/// Profondeur de l'ensemble des entités triées (voir `YSort`) et pente: un monde de 2 000 unités de haut reste entre 0,8 et 1,2.
const SORT_DEPTH: f32 = 1.0;
const SORT_SLOPE: f32 = 0.0002;

pub fn y_sort(mut sorted: Query<(&mut Transform, &YSort)>) {
    for (mut transform, sort) in &mut sorted {
        let z = SORT_DEPTH - transform.translation.y * SORT_SLOPE + sort.0;
        if (transform.translation.z - z).abs() > 1e-6 {
            transform.translation.z = z;
        }
    }
}

/// Ligne d'objet affichée par une entité: l'animation (impulsions, rotation) choisit l'image.
#[derive(Component)]
pub struct Prop(pub usize);

/// Progression de la marche d'un héros: la phase avance avec la distance parcourue, pour que les pas
/// collent au déplacement.
#[derive(Component, Default)]
pub struct WalkCycle {
    phase: f32,
    /// Dernière direction de marche (pour les monstres, qui n'ont pas de regard répliqué).
    direction: usize,
    last: Option<Vec2>,
    /// Temps restant pendant lequel on considère le héros en marche (évite de clignoter entre deux images du réseau).
    moving_for: f32,
    /// Dernier compteur de coups vu, et temps écoulé depuis le début du geste en cours.
    last_strikes: Option<u8>,
    attack_elapsed: Option<f32>,
}

fn load_set(assets: &AssetServer, layouts: &mut Assets<TextureAtlasLayout>, theme: ArtTheme) -> Art {
    let folder = theme.folder();
    // Les appareils à peu de mémoire (Apple TV HD...) ont des planches moins grandes, dans `lite/`.
    let lite = crate::settings::low_memory();
    let (cell, metas, sprite_folder) = if lite {
        (crate::sprite_meta_lite::CELL, crate::sprite_meta_lite::SHEETS, format!("{folder}/lite"))
    } else {
        (crate::sprite_meta::CELL, crate::sprite_meta::SHEETS, folder.to_string())
    };
    info!("sprites: cases de {cell} px ({sprite_folder})");
    let mut sheet_layouts = std::collections::HashMap::new();
    for &(dir, name, x, y, w, h) in metas {
        // Personnages et monstres: huit directions de `HERO_FRAMES` images sur `SHEET_COLUMNS` colonnes; objets: une ligne.
        let (columns, rows) = if dir == "objects" { (PROP_FRAMES as u32, 1) } else { (SHEET_COLUMNS, SHEET_ROWS) };
        let layout = layouts.add(TextureAtlasLayout::from_grid(UVec2::new(w, h), columns, rows, None, None));
        sheet_layouts.insert(format!("{dir}/{name}"), (layout, SheetCrop { x, y, w, h }));
    }
    let sheets = Sheets {
        assets: assets.clone(),
        folder: sprite_folder.clone(),
        cell,
        layouts: sheet_layouts,
        loaded: Default::default(),
    };
    let mut atlas = |file: &str, columns: u32, rows: u32, size: u32, smooth: bool| Atlas {
        // Les tuiles restent nettes (filtrage au plus proche); les rendus détaillés sont réduits à l'écran et sont lissés.
        image: if smooth {
            assets
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.sampler = ImageSampler::linear();
                    settings.asset_usage = RenderAssetUsages::RENDER_WORLD;
                })
                .load(format!("art/{folder}/{file}"))
        } else {
            assets.load(format!("art/{folder}/{file}"))
        },
        layout: layouts.add(TextureAtlasLayout::from_grid(UVec2::splat(size), columns, rows, None, None)),
    };
    Art {
        tiles: atlas("tiles.png", TILE_COLUMNS as u32, TILE_ROWS, TILE_CELL, false),
        exits: atlas(&format!("{}exits.basis", if lite { "lite/" } else { "" }), EXIT_FRAMES as u32, 7, cell * 2, true),
        projectiles: atlas("projectiles.png", 4, 1, 64, true),
        fx: atlas("fx.png", 2, 1, 96, true),
        decals: atlas("decals.png", 13, 1, 128, true),
        portraits: atlas("portraits.png", 4, 1, 128, true),
        decor: assets.load(format!("art/{folder}/decor.png")),
        sheets,
    }
}

/// Journal des grandes textures chargées (format et taille): utile pour comprendre la mémoire sur les petits appareils.
fn log_big_images(mut events: MessageReader<AssetEvent<Image>>, images: Res<Assets<Image>>) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } | AssetEvent::Added { id } = event {
            if let Some(image) = images.get(*id) {
                let size = image.texture_descriptor.size;
                if size.width * size.height > 4_000_000 {
                    info!("texture {}x{} {:?}", size.width, size.height, image.texture_descriptor.format);
                }
            }
        }
    }
}

pub(crate) fn load_art(mut commands: Commands, assets: Res<AssetServer>, mut layouts: ResMut<Assets<TextureAtlasLayout>>) {
    let theme = ArtTheme::initial();
    let library: Vec<_> = [ArtTheme::Medieval]
        .into_iter()
        .map(|t| (t, load_set(&assets, &mut layouts, t)))
        .collect();
    let current = library.iter().find(|(t, _)| *t == theme).map(|(_, art)| art.clone()).expect("thème chargé");
    commands.insert_resource(current);
    commands.insert_resource(ArtLibrary(library));
    commands.insert_resource(theme);
}

/// Animation de passage d'un mode à l'autre: l'écran se couvre (de sang qui coule vers le mode sanglant, d'un
/// éclat clair vers le mode manga), les sprites sont échangés au plus fort, puis l'écran se découvre.
#[derive(Resource, Default)]
struct ThemeTransition {
    elapsed: Option<f32>,
    swapped: bool,
}

/// Durées en secondes: l'écran se couvre, puis la transition complète.
const TRANSITION_COVER: f32 = 0.5;
const TRANSITION_TOTAL: f32 = 1.2;
/// Bandes verticales qui dessinent les coulures.
const DRIP_BARS: usize = 16;

#[derive(Component)]
struct TransitionOverlay;

#[derive(Component)]
struct TransitionDrip(usize);

fn spawn_transition_overlay(mut commands: Commands) {
    commands
        .spawn((
            TransitionOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(1000),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            for i in 0..DRIP_BARS {
                root.spawn((
                    TransitionDrip(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(i as f32 * 100.0 / DRIP_BARS as f32),
                        width: Val::Percent(100.0 / DRIP_BARS as f32 + 0.6),
                        top: Val::Px(0.0),
                        height: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ));
            }
        });
}

/// F2: lance la transition entre le mode manga et le mode sanglant.
fn start_transition(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<ThemeTransition>) {
    // F2 est désactivé: un seul style (médiéval).
    if false && keys.just_pressed(KeyCode::F2) && state.elapsed.is_none() {
        state.elapsed = Some(0.0);
        state.swapped = false;
    }
}

/// Fait avancer la transition; à mi-parcours, échange les deux jeux de planches. Les deux jeux ont le même
/// découpage: on remplace seulement l'image de chaque sprite existant.
fn run_transition(
    time: Res<Time>,
    mut commands: Commands,
    mut state: ResMut<ThemeTransition>,
    mut art: ResMut<Art>,
    library: Res<ArtLibrary>,
    mut theme: ResMut<ArtTheme>,
    mut sprites: Query<&mut Sprite>,
    stains: Query<Entity, With<Stain>>,
    mut overlay: Query<(&mut BackgroundColor, &mut Visibility), (With<TransitionOverlay>, Without<TransitionDrip>)>,
    mut drips: Query<(&TransitionDrip, &mut Node, &mut BackgroundColor), Without<TransitionOverlay>>,
) {
    let Ok((mut background, mut visibility)) = overlay.single_mut() else {
        return;
    };
    let Some(elapsed) = state.elapsed else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let elapsed = elapsed + time.delta_secs();

    if elapsed >= TRANSITION_COVER && !state.swapped {
        state.swapped = true;
        let target = theme.next();
        let new = library.0.iter().find(|(t, _)| *t == target).map(|(_, art)| art.clone()).expect("thème chargé");
        let old = std::mem::replace(&mut *art, new);
        let new = &*art;
        let pairs: Vec<(&Atlas, &Atlas)> = vec![
            (&old.tiles, &new.tiles),
            (&old.exits, &new.exits),
            (&old.projectiles, &new.projectiles),
            (&old.fx, &new.fx),
            (&old.decals, &new.decals),
        ];
        for mut sprite in &mut sprites {
            if let Some((_, to)) = pairs.iter().find(|(from, _)| from.image == sprite.image) {
                sprite.image = to.image.clone();
            }
        }
        *theme = target;
        info!("theme: {:?}", *theme);
        if *theme != ArtTheme::Gore {
            for stain in &stains {
                commands.entity(stain).despawn();
            }
        }
        // Un à-coup au moment du changement.
        commands.trigger(castle_shared::ScreenShake(0.45));
    }
    if elapsed >= TRANSITION_TOTAL {
        state.elapsed = None;
        return;
    }
    state.elapsed = Some(elapsed);

    // Couleurs selon le mode vers lequel on va: du sang qui coule, un éclat clair, ou une lueur d'ambre pour le médiéval.
    let target_theme = if state.swapped { *theme } else { theme.next() };
    let (bar, fill) = match target_theme {
        ArtTheme::Gore => (Color::srgb(0.5, 0.02, 0.04), Color::srgb(0.2, 0.0, 0.02)),
        ArtTheme::Manga => (Color::srgb(0.92, 0.96, 1.0), Color::srgb(0.85, 0.9, 1.0)),
        ArtTheme::Medieval => (Color::srgb(0.85, 0.62, 0.28), Color::srgb(0.35, 0.22, 0.08)),
    };
    let cover = (elapsed / TRANSITION_COVER).min(1.0);
    let alpha = if elapsed < TRANSITION_COVER {
        1.0
    } else {
        1.0 - (elapsed - TRANSITION_COVER) / (TRANSITION_TOTAL - TRANSITION_COVER)
    };
    *visibility = Visibility::Inherited;
    background.0 = fill.with_alpha(0.75 * cover * cover * alpha);
    for (drip, mut node, mut color) in &mut drips {
        // Chaque bande descend à son rythme (retard selon son rang), comme du sang qui coule.
        let delay = ((drip.0 * 7) % DRIP_BARS) as f32 / DRIP_BARS as f32 * 0.55;
        let reach = ((cover * 1.55 - delay) / 1.0).clamp(0.0, 1.0);
        node.height = Val::Percent(110.0 * reach * reach.sqrt());
        color.0 = bar.with_alpha(alpha);
    }
}

/// Débordement maximal, en pixels, de la lisière irrégulière entre une clairière et sa forêt.
const RAGGED_REACH: f32 = 26.0;
/// Amplitude et longueur d'onde, en pixels, de la limite ondulée entre la bande des troncs et l'herbe.
const BOUNDARY_REACH: f32 = 13.0;
const BOUNDARY_WAVE: f32 = 13.0;
/// Longueur d'onde, en pixels, des ondulations de la lisière.
const RAGGED_WAVE: f32 = 17.0;
/// Largeur, en pixels, sur laquelle une tuile se fond dans celle d'un voisin d'une autre région.
const TRANSITION_REACH: f32 = 14.0;
/// Côté d'une case de la planche de décor (deux tuiles).
const DECOR_CELL: usize = 96;

/// Bruit lissé entre 0 et 1 le long d'une ligne (`t` en longueurs d'onde), distinct pour chaque `seed`.
fn smooth_noise(t: f32, seed: u32) -> f32 {
    let (i, f) = (t.floor(), t - t.floor());
    let value = |k: f32| edge_noise(k as i32 as u32, seed, 3, 7);
    let blend = f * f * (3.0 - 2.0 * f);
    value(i) * (1.0 - blend) + value(i + 1.0) * blend
}

/// Deux octaves de `smooth_noise`: de grandes ondulations avec des irrégularités plus fines par-dessus.
fn fractal_noise(t: f32, seed: u32) -> f32 {
    // Un mélange d'ondulations de plusieurs tailles tend vers 0,5: on le ré-étale sur tout l'intervalle de 0 à 1.
    let mixed = 0.45 * smooth_noise(t * 0.4, seed + 23) + 0.35 * smooth_noise(t, seed) + 0.2 * smooth_noise(t * 2.7 + 5.0, seed + 11);
    ((mixed - 0.5) * 2.8 + 0.5).clamp(0.0, 1.0)
}

/// Valeur pseudo-aléatoire stable entre 0 et 1 pour un bloc de pixels d'une tuile.
fn edge_noise(x: u32, y: u32, bx: usize, by: usize) -> f32 {
    let mut h = x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ (bx as u32).wrapping_mul(83492791) ^ (by as u32).wrapping_mul(2654435761);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65536.0
}

/// Côté d'un bloc de décor, en tuiles, et débordement de chaque bloc sur ses voisins, en unités du monde.
const CHUNK: u32 = 16;
const TILE_OVERLAP: f32 = 0.35;

/// Assombrit un rectangle (haut, bas, gauche, droite, en pixels) d'une tuile de l'image d'un bloc, avec la teinte des ombres du
/// décor, comme le ferait un voile translucide posé dessus (le mélange se fait en lumière linéaire, comme à l'écran).
fn shade_pixels(pixels: &mut [u8], origin: usize, width: usize, (top, bottom, left, right): (usize, usize, usize, usize), alpha: f32) {
    for line in top..bottom {
        for column in left..right {
            shade_pixel(pixels, origin + (line * width + column) * 4, alpha);
        }
    }
}

/// Assombrit un pixel (RVBA, à l'indice `at`) avec la teinte des ombres du décor, comme un voile translucide d'opacité `alpha`.
fn shade_pixel(pixels: &mut [u8], at: usize, alpha: f32) {
    // Tables de conversion sRGB <-> lumière linéaire (calculées une fois): il y a des millions de pixels à mélanger.
    static TABLES: std::sync::OnceLock<(Vec<f32>, Vec<u8>, [f32; 3])> = std::sync::OnceLock::new();
    const STEPS: usize = 4096;
    let (to_linear, to_byte, shade) = TABLES.get_or_init(|| {
        let forward = (0..=255u8).map(|byte| Color::srgb_u8(byte, byte, byte).to_linear().red).collect();
        let backward = (0..STEPS)
            .map(|i| {
                let linear = i as f32 / (STEPS - 1) as f32;
                (Srgba::from(LinearRgba::new(linear, linear, linear, 1.0)).red * 255.0 + 0.5) as u8
            })
            .collect();
        let shade = Color::srgb(0.02, 0.0, 0.06).to_linear();
        (forward, backward, [shade.red, shade.green, shade.blue])
    });
    for channel in 0..3 {
        let mixed = to_linear[pixels[at + channel] as usize] * (1.0 - alpha) + shade[channel] * alpha;
        pixels[at + channel] = to_byte[(mixed.clamp(0.0, 1.0) * (STEPS - 1) as f32 + 0.5) as usize];
    }
}

/// Décalage, en pixels, de la limite entre la bande des troncs et l'herbe, à la position `gx` (pixels, en coordonnées du niveau):
/// positif, la bande descend dans l'herbe; négatif, l'herbe monte dans la bande.
fn boundary_contour(gx: usize) -> f32 {
    BOUNDARY_REACH * (2.0 * fractal_noise(gx as f32 / BOUNDARY_WAVE, 7) - 1.0) - 3.0 + 2.0 * (edge_noise(gx as u32, 5, 1, 1) - 0.5)
}

/// Tuile du décor, effacée quand le château est regénéré.
#[derive(Component)]
struct WorldTile;

/// Marque une tuile du décor (pour la teinte du héros dirigé).
#[derive(Component)]
struct TileMark;

/// Teint les tuiles de la couleur du héros dirigé (multiplication de la couleur): seul le décor change, les
/// personnages gardent leurs couleurs. Appliqué quand la teinte bouge, et à toute nouvelle tuile.
fn tint_tiles(
    grade: Res<crate::lighting::Grade>,
    fresh: Query<(), Added<TileMark>>,
    mut tiles: Query<&mut Sprite, With<TileMark>>,
    mut last: Local<Vec3>,
) {
    if last.distance(grade.0) < 0.002 && fresh.is_empty() {
        return;
    }
    *last = grade.0;
    let color = Color::srgb(grade.0.x, grade.0.y, grade.0.z);
    for mut sprite in &mut tiles {
        sprite.color = color;
    }
}

/// Les pixels qui servent à assembler le sol: la planche des tuiles et celle du décor (RVBA, lignes à la suite).
struct BakeSources<'a> {
    tiles: &'a [u8],
    tiles_width: usize,
    decor: &'a [u8],
    decor_width: usize,
}

fn rank(region: Region) -> usize {
    Region::ALL.iter().position(|r| *r == region).unwrap_or(0)
}

/// Assemble en une image (RVBA, `TILE_CELL` pixels par tuile, première ligne en haut) le bloc de tuiles dont le coin bas-gauche est
/// (`chunk_x`, `chunk_y`): les tuiles de chaque région, les fondus entre régions, les ombres le long des murs, puis le décor.
fn bake_chunk(def: &LevelDef, room: &Room, sources: &BakeSources, chunk_x: u32, chunk_y: u32) -> Vec<u8> {
    let size = room.size();
    let (cols, rows) = ((size.x - chunk_x).min(CHUNK), (size.y - chunk_y).min(CHUNK));
    let (source, source_width) = (sources.tiles, sources.tiles_width);
    let (decor_source, decor_width) = (sources.decor, sources.decor_width);
    let tile_px = TILE_CELL as usize;
    let width = cols as usize * tile_px;
    let mut pixels = vec![0u8; width * rows as usize * tile_px * 4];
    for row in 0..rows {
        // La première ligne de l'image est le haut du bloc.
        let y = chunk_y + rows - 1 - row;
        for col in 0..cols {
            let x = chunk_x + col;
            let (xi, yi) = (x as i32, y as i32);
            let tile_row = rank(def.region_at(xi, yi));
            let column = tile_column(x, y, tile_row, room.tile(xi, yi) == Tile::Wall, room.tile(xi, yi - 1) == Tile::Floor);
            let origin = (row as usize * tile_px * width + col as usize * tile_px) * 4;
            for line in 0..tile_px {
                let from = ((tile_row * TILE_CELL as usize + line) * source_width + column * tile_px) * 4;
                let to = origin + line * width * 4;
                pixels[to..to + tile_px * 4].copy_from_slice(&source[from..from + tile_px * 4]);
            }
            // Fondu avec les voisins qui empruntent le décor d'une autre région: au bord, la tuile prend peu à peu l'aspect
            // du voisin (le bord est irrégulier), au lieu d'une coupure nette.
            let wall = room.tile(xi, yi) == Tile::Wall;
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (xi + dx, yi + dy);
                if nx < 0 || ny < 0 || nx >= size.x as i32 || ny >= size.y as i32 || (room.tile(nx, ny) == Tile::Wall) != wall {
                    continue;
                }
                let neighbor_row = rank(def.region_at(nx, ny));
                if neighbor_row == tile_row {
                    continue;
                }
                let neighbor_column = tile_column(x, y, neighbor_row, wall, room.tile(xi, yi - 1) == Tile::Floor);
                for py in 0..tile_px {
                    for px in 0..tile_px {
                        // Distance au bord partagé, en pixels, et irrégularité du fondu (par blocs de 2 pixels).
                        let edge = match (dx, dy) {
                            (-1, 0) => px,
                            (1, 0) => tile_px - 1 - px,
                            (0, 1) => py,
                            _ => tile_px - 1 - py,
                        } as f32;
                        let noise = edge_noise(x, y, px / 2, py / 2);
                        let reach = TRANSITION_REACH * (0.45 + 0.9 * noise);
                        if edge >= reach {
                            continue;
                        }
                        let weight = 0.5 * (1.0 - edge / reach);
                        let from = ((neighbor_row * TILE_CELL as usize + py) * source_width + neighbor_column * tile_px + px) * 4;
                        let to = origin + (py * width + px) * 4;
                        for channel in 0..3 {
                            let mixed = pixels[to + channel] as f32 * (1.0 - weight) + source[from + channel] as f32 * weight;
                            pixels[to + channel] = (mixed + 0.5) as u8;
                        }
                    }
                }
            }
            // Dehors, la lisière n'est pas un trait droit: les arbres débordent de façon irrégulière sur la clairière (et la
            // clairière mord un peu sur la forêt), ce qui efface l'effet d'escalier des bords de tuiles.
            if Region::ALL[tile_row].is_outdoor() {
                // Seule la forêt déborde sur la clairière: si la clairière mordait aussi sur la forêt, la limite des deux tuiles resterait droite.
                let face = wall;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1)] {
                    let (nx, ny) = (xi + dx, yi + dy);
                    if face || nx < 0 || ny < 0 || nx >= size.x as i32 || ny >= size.y as i32 {
                        continue;
                    }
                    let neighbor_wall = room.tile(nx, ny) == Tile::Wall;
                    if neighbor_wall == wall {
                        // Même nature des deux côtés: pas de lisière à casser.
                        continue;
                    }
                    let neighbor_row = rank(def.region_at(nx, ny));
                    let neighbor_column = tile_column(nx as u32, ny as u32, neighbor_row, neighbor_wall, room.tile(nx, ny - 1) == Tile::Floor);
                    let reach = RAGGED_REACH;
                    for py in 0..tile_px {
                        for px in 0..tile_px {
                            let edge = match (dx, dy) {
                                (-1, 0) => px,
                                (1, 0) => tile_px - 1 - px,
                                _ => tile_px - 1 - py,
                            };
                            if edge as f32 >= reach {
                                continue;
                            }
                            // Le contour de la lisière ondule le long du bord (bruit lissé en coordonnées du niveau, donc continu d'une
                            // tuile à l'autre), avec un peu de grain feuillu par-dessus.
                            let along = if dx != 0 { y as usize * tile_px + py } else { x as usize * tile_px + px };
                            let side = if dx != 0 { 1 } else { 2 };
                            let wave = fractal_noise(along as f32 / RAGGED_WAVE, side);
                            let grain = edge_noise(x, y, px / 2, py / 2);
                            let contour = reach * (0.08 + 0.92 * wave) + 3.0 * (grain - 0.5);
                            if (edge as f32) >= contour {
                                continue;
                            }
                            // Le pixel du voisin à la même distance du bord partagé.
                            let (sx, sy) = match (dx, dy) {
                                (-1, 0) => (tile_px - 1 - edge, py),
                                (1, 0) => (edge, py),
                                _ => (px, edge),
                            };
                            let from = ((neighbor_row * TILE_CELL as usize + sy) * source_width + neighbor_column * tile_px + sx) * 4;
                            let to = origin + (py * width + px) * 4;
                            pixels[to..to + 3].copy_from_slice(&source[from..from + 3]);
                        }
                    }
                }
            }
            // Entre la bande des troncs (face d'un mur) et l'herbe qui la longe, la limite ondule le long du bord au lieu de suivre la
            // ligne droite de la tuile: la face avance dans l'herbe par endroits, et l'herbe remonte dans la face ailleurs. Les deux
            // tuiles lisent le même contour (en coordonnées du niveau), donc il est continu de l'une à l'autre.
            let outdoor = Region::ALL[tile_row].is_outdoor();
            if outdoor && !wall && room.tile(xi, yi + 1) == Tile::Wall {
                let above_row = rank(def.region_at(xi, yi + 1));
                let above_column = tile_column(x, y + 1, above_row, true, true);
                for px in 0..tile_px {
                    let depth = boundary_contour(x as usize * tile_px + px).max(0.0) as usize;
                    for py in 0..depth.min(tile_px) {
                        // La ligne du bas de la face est contre la ligne du haut de cette tuile.
                        let from = ((above_row * TILE_CELL as usize + (tile_px - 1 - py)) * source_width + above_column * tile_px + px) * 4;
                        let to = origin + (py * width + px) * 4;
                        pixels[to..to + 3].copy_from_slice(&source[from..from + 3]);
                    }
                }
            }
            if outdoor && wall && room.tile(xi, yi - 1) == Tile::Floor {
                let below_row = rank(def.region_at(xi, yi - 1));
                let below_column = tile_column(x, y - 1, below_row, false, false);
                for px in 0..tile_px {
                    let depth = (-boundary_contour(x as usize * tile_px + px)).max(0.0) as usize;
                    for d in 0..depth.min(tile_px) {
                        let from = ((below_row * TILE_CELL as usize + d) * source_width + below_column * tile_px + px) * 4;
                        let to = origin + ((tile_px - 1 - d) * width + px) * 4;
                        pixels[to..to + 3].copy_from_slice(&source[from..from + 3]);
                    }
                }
            }
            // Ombre portée sur le sol le long d'un mur: au-dessus (face du mur) et sur les côtés.
            if room.tile(xi, yi) == Tile::Floor {
                if room.tile(xi, yi + 1) == Tile::Wall {
                    // Sous le mur, l'ombre s'estompe progressivement (et suit le contour de la lisière, dehors).
                    for px in 0..tile_px {
                        let shift = if outdoor { 0.5 * boundary_contour(x as usize * tile_px + px).max(0.0) } else { 0.0 };
                        for py in 0..(2 * tile_px / 3).min(tile_px) {
                            let fade = 1.0 - (py as f32 - shift).max(0.0) / (tile_px as f32 * 0.5);
                            if fade > 0.0 {
                                shade_pixel(&mut pixels, origin + (py * width + px) * 4, 0.42 * fade.powf(1.6));
                            }
                        }
                    }
                }
                let mut shade = |top: usize, bottom: usize, left: usize, right: usize, alpha: f32| {
                    shade_pixels(&mut pixels, origin, width, (top, bottom, left, right), alpha);
                };
                let unit = tile_px / 16;
                if room.tile(xi - 1, yi) == Tile::Wall {
                    shade(0, tile_px, 0, 3 * unit, 0.3);
                }
                if room.tile(xi + 1, yi) == Tile::Wall {
                    shade(0, tile_px, tile_px - 3 * unit, tile_px, 0.3);
                }
            }
        }
    }
    // Le décor posé au sol (herbes, cailloux, flaques...) se mêle aux tuiles du bloc.
    let chunk_origin = Vec2::new(chunk_x as f32, (chunk_y + rows) as f32);
    for decor in def.decor.iter().filter(|d| {
        d.at.x >= chunk_x as f32 - 1.0 && d.at.x < (chunk_x + cols) as f32 + 1.0 && d.at.y >= chunk_y as f32 - 1.0 && d.at.y < (chunk_y + rows) as f32 + 1.0
    }) {
        // Coin haut-gauche de la case de 96 px (deux tuiles), en pixels de l'image du bloc (y vers le bas).
        let left = ((decor.at.x - chunk_origin.x) * tile_px as f32).round() as i32 - DECOR_CELL as i32 / 2;
        let top = ((chunk_origin.y - decor.at.y) * tile_px as f32).round() as i32 - DECOR_CELL as i32 / 2;
        let (column, row) = (decor.kind.column(), (decor.variant % DECOR_VARIANTS) as usize);
        for py in 0..DECOR_CELL {
            let y = top + py as i32;
            if y < 0 || y >= (rows as usize * tile_px) as i32 {
                continue;
            }
            for px in 0..DECOR_CELL {
                let x = left + px as i32;
                if x < 0 || x >= width as i32 {
                    continue;
                }
                let from = ((row * DECOR_CELL + py) * decor_width + column * DECOR_CELL + px) * 4;
                let alpha = decor_source[from + 3] as f32 / 255.0;
                if alpha <= 0.0 {
                    continue;
                }
                let to = (y as usize * width + x as usize) * 4;
                for channel in 0..3 {
                    let mixed = pixels[to + channel] as f32 * (1.0 - alpha) + decor_source[from + channel] as f32 * alpha;
                    pixels[to + channel] = (mixed + 0.5) as u8;
                }
            }
        }
    }
    pixels
}

/// Reconstruit la carte quand une expédition arrive du serveur: la graine suffit à la regénérer.
/// Un mur dont la tuile du dessous est du sol montre sa face de briques.
fn rebuild_world(
    mut commands: Commands,
    art: Res<Art>,
    theme: Res<ArtTheme>,
    expeditions: Query<&Expedition>,
    fresh: Query<(), Changed<Expedition>>,
    old_tiles: Query<Entity, With<WorldTile>>,
    mut images: ResMut<Assets<Image>>,
    mut waiting: Local<bool>,
) {
    // Le décor se reconstruit quand un nouveau château arrive, ou quand on change de mode.
    if fresh.is_empty() && !theme.is_changed() && !*waiting {
        return;
    }
    let Some(expedition) = expeditions.iter().last() else {
        return;
    };
    // Les tuiles sont assemblées à partir de leur planche: on attend qu'elle soit chargée.
    let Some(sheet) = images.get(&art.tiles.image).filter(|image| image.data.is_some()) else {
        *waiting = true;
        return;
    };
    *waiting = false;
    let (source, source_width, format) = (sheet.data.clone().unwrap_or_default(), sheet.width() as usize, sheet.texture_descriptor.format);
    let Some(decor_sheet) = images.get(&art.decor).filter(|image| image.data.is_some()) else {
        *waiting = true;
        return;
    };
    let (decor_source, decor_width) = (decor_sheet.data.clone().unwrap_or_default(), decor_sheet.width() as usize);
    info!("building level {} ({:?})", expedition.level, expedition.region);
    for entity in &old_tiles {
        commands.entity(entity).despawn();
    }
    let def = castle_shared::level_def(expedition.level);
    let dungeon = def.to_dungeon().unwrap_or_else(|e| panic!("niveau {}: {e}", expedition.level));
    let room = &dungeon.room;
    // Le décor d'ensemble (décalques...) suit la région du niveau; chaque tuile peut emprunter les briques d'une autre.
    let region_row = rank(expedition.region);
    let size = room.size();
    // Le sol et les murs sont assemblés en grands blocs de `CHUNK` x `CHUNK` tuiles (une image et une entité par bloc, au lieu
    // d'une entité par tuile): c'est ce qui rend le jeu fluide sur les petits appareils.
    let sources = BakeSources { tiles: &source, tiles_width: source_width, decor: &decor_source, decor_width };
    for chunk_y in (0..size.y).step_by(CHUNK as usize) {
        for chunk_x in (0..size.x).step_by(CHUNK as usize) {
            let (cols, rows) = ((size.x - chunk_x).min(CHUNK), (size.y - chunk_y).min(CHUNK));
            let pixels = bake_chunk(def, room, &sources, chunk_x, chunk_y);
            let image = Image::new(
                bevy::render::render_resource::Extent3d {
                    width: cols * TILE_CELL,
                    height: rows * TILE_CELL,
                    depth_or_array_layers: 1,
                },
                bevy::render::render_resource::TextureDimension::D2,
                pixels,
                format,
                RenderAssetUsages::RENDER_WORLD,
            );
            let center = (room.tile_center(chunk_x, chunk_y) + room.tile_center(chunk_x + cols - 1, chunk_y + rows - 1)) / 2.0;
            commands.spawn((
                WorldTile,
                TileMark,
                Sprite {
                    image: images.add(image),
                    // Les blocs se recouvrent un peu: sans cela, des fentes d'un pixel apparaissent entre eux quand la caméra est
                    // à une échelle non entière ou à une position fractionnaire.
                    custom_size: Some(Vec2::new(cols as f32, rows as f32) * TILE_SIZE + Vec2::splat(TILE_OVERLAP)),
                    ..default()
                },
                Transform::from_translation(center.extend(0.0)),
            ));
        }
    }
    spawn_decals(&mut commands, &art, &dungeon, region_row, expedition.seed, *theme == ArtTheme::Gore);
    commands.insert_resource(dungeon.room);
    commands.insert_resource(dungeon.layout);
}

/// Animation des héros: la ligne de la planche suit la direction du regard (8 directions), et le cycle de
/// marche avance avec la distance parcourue; à l'arrêt, le héros respire.
fn animate_heroes(
    time: Res<Time>,
    mut heroes: Query<(&mut Sprite, &Transform, &Facing, &Strikes, &mut WalkCycle), With<HeroClass>>,
) {
    let dt = time.delta_secs();
    for (mut sprite, transform, facing, strikes, mut cycle) in &mut heroes {
        let here = transform.translation.truncate();
        let moved = cycle.last.map_or(0.0, |last| last.distance(here));
        cycle.last = Some(here);
        if moved > 0.03 {
            cycle.phase = (cycle.phase + moved / HERO_STRIDE).fract();
            cycle.moving_for = 0.12;
        } else {
            cycle.moving_for = (cycle.moving_for - dt).max(0.0);
        }
        // Un nouveau coup (le compteur du serveur a changé) lance le geste de l'arme.
        if cycle.last_strikes.is_some_and(|last| last != strikes.0) {
            cycle.attack_elapsed = Some(0.0);
        }
        cycle.last_strikes = Some(strikes.0);
        let attack_frame = cycle.attack_elapsed.map(|elapsed| elapsed / ATTACK_DURATION).filter(|p| *p < 1.0);
        cycle.attack_elapsed = attack_frame.map(|p| p * ATTACK_DURATION + dt);
        let frame = if let Some(progress) = attack_frame {
            HERO_IDLE_FRAMES + HERO_WALK_FRAMES + ((progress * HERO_ATTACK_FRAMES as f32) as usize).min(HERO_ATTACK_FRAMES - 1)
        } else if cycle.moving_for > 0.0 {
            HERO_IDLE_FRAMES + (cycle.phase * HERO_WALK_FRAMES as f32) as usize % HERO_WALK_FRAMES
        } else {
            (time.elapsed_secs() * ANIMATION_FPS) as usize % HERO_IDLE_FRAMES
        };
        let index = sheet_index(direction_index(facing.0), frame);
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            if atlas.index != index {
                atlas.index = index;
            }
        }
    }
}

/// Animation des monstres: la direction suit leur déplacement (ils n'ont pas de regard répliqué), le squelette
/// marche au rythme de la distance parcourue et la chauve-souris bat des ailes en permanence.
fn animate_monsters(
    time: Res<Time>,
    mut monsters: Query<(&mut Sprite, &Position, &Transform, &Monster, &Strikes, &mut WalkCycle)>,
) {
    let dt = time.delta_secs();
    for (mut sprite, pos, transform, monster, strikes, mut cycle) in &mut monsters {
        let delta = pos.0 - transform.translation.truncate();
        let here = transform.translation.truncate();
        let moved = cycle.last.map_or(0.0, |last| last.distance(here));
        cycle.last = Some(here);
        if delta.length() > 0.3 {
            cycle.direction = direction_index(delta);
        }
        if moved > 0.03 {
            cycle.phase = (cycle.phase + moved / HERO_STRIDE).fract();
            cycle.moving_for = 0.12;
        } else {
            cycle.moving_for = (cycle.moving_for - dt).max(0.0);
        }
        // Chaque coup porté (le compteur du serveur change) lance le geste d'attaque.
        if cycle.last_strikes.is_some_and(|last| last != strikes.0) {
            cycle.attack_elapsed = Some(0.0);
        }
        cycle.last_strikes = Some(strikes.0);
        let attack_progress = cycle.attack_elapsed.map(|elapsed| elapsed / MONSTER_ATTACK_DURATION).filter(|p| *p < 1.0);
        cycle.attack_elapsed = attack_progress.map(|p| p * MONSTER_ATTACK_DURATION + dt);
        let frame = match monster.0 {
            _ if attack_progress.is_some() => {
                let progress = attack_progress.unwrap_or(0.0);
                HERO_IDLE_FRAMES + HERO_WALK_FRAMES + ((progress * HERO_ATTACK_FRAMES as f32) as usize).min(HERO_ATTACK_FRAMES - 1)
            }
            MonsterKind::Bat => HERO_IDLE_FRAMES + (time.elapsed_secs() * ANIMATION_FPS) as usize % HERO_WALK_FRAMES,
            _ if cycle.moving_for > 0.0 => HERO_IDLE_FRAMES + (cycle.phase * HERO_WALK_FRAMES as f32) as usize % HERO_WALK_FRAMES,
            _ => (time.elapsed_secs() * ANIMATION_FPS) as usize % HERO_IDLE_FRAMES,
        };
        let index = sheet_index(cycle.direction, frame);
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            if atlas.index != index {
                atlas.index = index;
            }
        }
    }
}

/// Animation des objets: vingt images (impulsion de lumière, pièce qui tourne, engrenages...).
fn animate_props(time: Res<Time>, art: Res<Art>, mut props: Query<(&mut Sprite, &Prop, &mut Anchor)>) {
    let frame = (time.elapsed_secs() * ANIMATION_FPS) as usize % PROP_FRAMES;
    for (mut sprite, prop, mut anchor) in &mut props {
        // Chaque objet a sa planche (et sa zone rognée): un piège qui se désarme, un panneau qui s'active en change.
        let current = art.sheets.image("objects", object::FILES[prop.0]);
        if sprite.image != current {
            let (fresh, fresh_anchor) = art.prop_sprite(prop.0, frame, Vec2::splat(SPRITE_SIZE));
            let color = sprite.color;
            *sprite = Sprite { color, ..fresh };
            *anchor = fresh_anchor;
        }
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            if atlas.index != frame {
                atlas.index = frame;
            }
        }
    }
}

/// Teintes d'usure par région: (taches sombres, marque propre à la région), en RVBA. Tout reste sombre.
fn decal_palette(region_row: usize) -> [Color; 2] {
    let c = |r: f32, g: f32, b: f32, a: f32| Color::srgba(r, g, b, a);
    match region_row {
        0 => [c(0.03, 0.03, 0.08, 0.24), c(0.02, 0.05, 0.1, 0.24)],  // humidité
        1 => [c(0.04, 0.1, 0.04, 0.24), c(0.05, 0.22, 0.08, 0.28)],  // mousse sombre
        2 => [c(0.1, 0.04, 0.02, 0.24), c(0.08, 0.04, 0.16, 0.24)],  // encre
        3 => [c(0.0, 0.0, 0.0, 0.32), c(0.1, 0.03, 0.0, 0.34)],      // suie, brûlures
        4 => [c(0.02, 0.03, 0.12, 0.26), c(0.06, 0.12, 0.3, 0.22)],  // humidité froide
        5 => [c(0.28, 0.04, 0.08, 0.22), c(0.1, 0.05, 0.1, 0.24)],   // vin renversé, cire
        6 => [c(0.05, 0.1, 0.03, 0.22), c(0.2, 0.15, 0.05, 0.2)],    // feuilles mortes, terre
        7 => [c(0.03, 0.08, 0.06, 0.28), c(0.05, 0.14, 0.1, 0.26)],  // vase, algues
        _ => [c(0.12, 0.08, 0.04, 0.24), c(0.06, 0.05, 0.04, 0.22)], // poussière, ornières
    }
}

/// Pose l'usure du sol: de grandes marques douces qui s'étendent sur plusieurs tuiles (taches, traînées de passage
/// le long des couloirs, fissures), plus rares et plus discrètes que les défauts d'une tuile isolée.
fn spawn_decals(commands: &mut Commands, art: &Art, dungeon: &Dungeon, region_row: usize, seed: u64, gore: bool) {
    let (room, layout) = (&dungeon.room, &dungeon.layout);
    let mut rng = castle_shared::Rng::new(seed ^ 0xDECA15);
    // Mode sanglant: du sang séché, deux fois plus de marques, avec des éclaboussures.
    let [dark, special] = if gore {
        [Color::srgba(0.32, 0.02, 0.04, 0.4), Color::srgba(0.1, 0.01, 0.02, 0.45)]
    } else {
        decal_palette(region_row)
    };
    // Cases de la planche des marques: 0..4 taches, 4..7 traînées, 7..10 fissures.
    let place = |commands: &mut Commands, index: usize, center: Vec2, size: Vec2, angle: f32, color: Color| {
        let mut sprite = art.decals.sprite_sized(index, size);
        sprite.color = color;
        commands.spawn((
            WorldTile,
            sprite,
            Transform {
                translation: center.extend(0.02),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
        ));
    };

    // Taches et fissures dans les salles, en vérifiant qu'aucun mur (pilier) ne se trouve dessous.
    for info in &layout.rooms {
        let dims = info.size();
        let count = (rng.range(0, 1) + (dims.x * dims.y) / 110) * if gore { 2 } else { 1 };
        for _ in 0..count {
            for _attempt in 0..8 {
                let kind = rng.range(0, 10);
                // Des taches sombres de deux à trois tuiles, et de fines fissures: rien de large ni de clair.
                let (index, color, tiles) = match kind {
                    0..=4 if gore => (10 + rng.range(0, 2) as usize, dark, 2.5 + rng.range(0, 1) as f32 * 1.5),
                    0..=4 => (rng.range(0, 3) as usize, dark, 2.0 + rng.range(0, 1) as f32 * 1.5),
                    5..=7 => (rng.range(0, 3) as usize, special, 2.5 + rng.range(0, 1) as f32),
                    _ => (7 + rng.range(0, 2) as usize, Color::srgba(0.02, 0.02, 0.05, 0.3), 3.0),
                };
                let half = (tiles / 2.0).min(dims.x.min(dims.y) as f32 / 2.0 - 0.6);
                if half < 1.0 {
                    continue;
                }
                let mut span = |lo: u32, hi: u32| lo as f32 + half + rng.next_f32() * ((hi + 1 - lo) as f32 - 2.0 * half).max(0.0);
                let (cx, cy) = (span(info.min.x, info.max.x), span(info.min.y, info.max.y));
                // Chaque point d'un maillage d'une tuile doit être du sol.
                let samples = (half * 2.0).ceil() as i32;
                let clear = (-samples..=samples).all(|dy| {
                    (-samples..=samples).all(|dx| {
                        let (fx, fy) = (dx as f32 * 0.5, dy as f32 * 0.5);
                        fx.abs() > half || fy.abs() > half || room.tile((cx + fx) as i32, (cy + fy) as i32) == Tile::Floor
                    })
                });
                if !clear {
                    continue;
                }
                let center = room.tile_center(0, 0) + Vec2::new(cx - 0.5, cy - 0.5) * TILE_SIZE;
                place(commands, index, center, Vec2::splat(tiles * TILE_SIZE), rng.next_f32() * std::f32::consts::TAU, color);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_image_of_a_character_fits_in_its_sheet() {
        let cells = (SHEET_COLUMNS * SHEET_ROWS) as usize;
        assert_eq!(cells, 8 * HERO_FRAMES, "la planche contient huit directions de HERO_FRAMES images");
        assert_eq!(sheet_index(7, HERO_FRAMES - 1), cells - 1);
        assert_eq!(object::FILES.len(), 26);
    }

    #[test]
    fn a_full_cell_keeps_its_anchor_and_a_crop_keeps_the_origin_in_place() {
        let cell = crate::sprite_meta::CELL;
        let full = crop_anchor(PROP_ANCHOR, SheetCrop { x: 0, y: 0, w: cell, h: cell }, cell);
        assert!((full.0 - PROP_ANCHOR.0).length() < 1e-5);
        // La moitié basse de la case: l'origine (aux pieds) reste au même endroit du dessin.
        let half = crop_anchor(PROP_ANCHOR, SheetCrop { x: 0, y: cell / 2, w: cell, h: cell / 2 }, cell);
        let origin_in_full = Vec2::new(0.5, 0.5 - PROP_ANCHOR.0.y);
        let origin_in_crop = Vec2::new(0.5 - half.0.x, 0.5 - half.0.y);
        assert!((origin_in_crop.x - origin_in_full.x).abs() < 1e-5);
        // Dans la zone rognée (hauteur moitié), la position verticale vaut (y_complet - 0,5) * 2.
        assert!((origin_in_crop.y - (origin_in_full.y - 0.5) * 2.0).abs() < 1e-4);
    }

    /// Chaque planche livrée (grande et légère) se lit, et sa taille est celle que le jeu attend (cases rognées x colonnes et lignes).
    #[test]
    fn every_shipped_sheet_decodes_to_the_size_the_layout_expects() {
        use basis_universal::Transcoder;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/art/medieval");
        for (folder, metas) in [("", crate::sprite_meta::SHEETS), ("lite", crate::sprite_meta_lite::SHEETS)] {
            for &(dir, name, _, _, w, h) in metas {
                let path = root.join(folder).join(dir).join(format!("{name}.basis"));
                let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                let mut transcoder = Transcoder::new();
                let info = transcoder.image_info(&data, 0).unwrap_or_else(|| panic!("{}: en-tête illisible", path.display()));
                let (columns, rows) = if dir == "objects" { (PROP_FRAMES as u32, 1) } else { (SHEET_COLUMNS, SHEET_ROWS) };
                assert_eq!((info.m_orig_width, info.m_orig_height), (columns * w, rows * h), "{}", path.display());
            }
        }
    }

    fn read_png(path: &std::path::Path) -> (Vec<u8>, usize) {
        let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut data = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut data).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba, "{}", path.display());
        data.truncate(info.buffer_size());
        (data, info.width as usize)
    }

    /// Assemble tout un niveau comme le jeu le fait; avec `CASTLE_BAKE_PNG=<dossier>`, l'image est écrite pour être regardée.
    fn bake_level(def: &LevelDef, sources: &BakeSources) -> (Vec<u8>, usize, usize) {
        let room = def.to_dungeon().unwrap().room;
        let size = room.size();
        let (width, height) = (size.x as usize * TILE_CELL as usize, size.y as usize * TILE_CELL as usize);
        let mut whole = vec![0u8; width * height * 4];
        for chunk_y in (0..size.y).step_by(CHUNK as usize) {
            for chunk_x in (0..size.x).step_by(CHUNK as usize) {
                let (cols, rows) = ((size.x - chunk_x).min(CHUNK) as usize, (size.y - chunk_y).min(CHUNK) as usize);
                let pixels = bake_chunk(def, &room, sources, chunk_x, chunk_y);
                let tile = TILE_CELL as usize;
                for line in 0..rows * tile {
                    // Le bloc part du bas de la carte; sa première ligne est son haut.
                    let top = height - (chunk_y as usize + rows) * tile;
                    let to = ((top + line) * width + chunk_x as usize * tile) * 4;
                    whole[to..to + cols * tile * 4].copy_from_slice(&pixels[line * cols * tile * 4..(line + 1) * cols * tile * 4]);
                }
            }
        }
        (whole, width, height)
    }

    #[test]
    fn baking_a_level_adds_decor_and_blends_regions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/art/medieval");
        let (tiles, tiles_width) = read_png(&root.join("tiles.png"));
        let (decor, decor_width) = read_png(&root.join("decor.png"));
        let sources = BakeSources { tiles: &tiles, tiles_width, decor: &decor, decor_width };
        for level in [1, 2, 3, 4, 6] {
            let def = castle_shared::level_def(level);
            let (whole, width, height) = bake_level(def, &sources);
            assert!(whole.chunks(4).all(|p| p[3] == 255), "niveau {level}: le sol est opaque");
            if let Some(dir) = std::env::var_os("CASTLE_BAKE_PNG") {
                let path = std::path::Path::new(&dir).join(format!("niveau_{level}.png"));
                let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path).unwrap()), width as u32, height as u32);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&whole).unwrap();
            }
            // Sans décor, l'image est différente.
            let mut bare = def.clone();
            bare.decor.clear();
            let (without, ..) = bake_level(&bare, &sources);
            assert_ne!(whole, without, "niveau {level}: le décor se voit");
        }
    }
}
