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
/// 12 images par ligne: 4 de repos (respiration) puis 8 de marche.
pub const HERO_IDLE_FRAMES: usize = 4;
pub const HERO_WALK_FRAMES: usize = 8;
pub const HERO_ATTACK_FRAMES: usize = 6;
/// Images par ligne pour les héros (repos, marche, attaque) et pour les ennemis (repos, marche).
const HERO_FRAMES: usize = HERO_IDLE_FRAMES + HERO_WALK_FRAMES + HERO_ATTACK_FRAMES;
const MONSTER_FRAMES: usize = HERO_IDLE_FRAMES + HERO_WALK_FRAMES;
/// Une ligne par monstre et par direction (cinq monstres, huit directions).
const MONSTER_ROWS: u32 = 5 * 8;
/// Durée du geste d'une attaque, en secondes (la frappe tombe vers 60 % du geste).
const ATTACK_DURATION: f32 = 0.38;
const HERO_CELL: u32 = 96;
/// Taille d'affichage d'un héros, d'un ennemi ou d'un objet, en unités du monde (la case fait 96 px d'art).
pub const SPRITE_SIZE: f32 = 32.0;
/// Les ennemis et les objets partagent le découpage de la planche des héros pour les ennemis (8 directions,
/// 12 images), et 4 images d'animation par objet.
const PROP_FRAMES: usize = 4;
/// Colonnes d'une ligne de la planche des tuiles: 4 sols, 2 dessus de mur, 2 faces de mur (une ligne par région).
const TILE_COLUMNS: usize = 90;
/// Découpage d'une ligne de la planche des tuiles (identique à `tools/sprite_forge`): 3 motifs de sol de 4x4 tuiles,
/// 2 dessus de mur de 4x4 tuiles, 2 motifs de face de mur de 4 tuiles, 2 faces décorées.
const MACRO: u32 = 4;
const FLOOR_PATTERNS: u32 = 3;
const TOP_START: usize = 48;
const TOP_PATTERNS: u32 = 2;
const FACE_PATTERNS: u32 = 2;
const FACE_START: usize = 80;
const DECOR_START: usize = 88;
const TILE_CELL: u32 = 48;
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
                (start_transition, run_transition, rebuild_world, update_tile_blur, tint_tiles, animate_heroes, animate_monsters, animate_props).chain(),
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
    /// Tuiles légèrement floues, puis plus floues, pour les zones éloignées du héros.
    pub tiles_blur: [Atlas; 3],
    pub heroes: Atlas,
    pub monsters: Atlas,
    pub objects: Atlas,
    pub projectiles: Atlas,
    pub fx: Atlas,
    pub decals: Atlas,
    /// Vignettes des quatre héros (une case de 128 px par classe).
    pub portraits: Atlas,
}

/// Lignes de la planche d'objets (même ordre que dans `tools/sprite_forge`).
pub mod object {
    pub const GENERATOR: usize = 0;
    pub const TURRET: usize = 1;
    pub const COIN: usize = 2;
    pub const FOOD: usize = 3;
    pub const KEY: usize = 4;
    // Les lignes 5 (levier) et 7 (mécanisme) de la planche sont des objets au sol que l'énigme n'utilise plus.
    pub const RUNE: usize = 6;
    pub const TRAP_ARMED: usize = 8;
    pub const TRAP_OFF: usize = 9;
    pub const DOOR_LOCKED: usize = 10;
    pub const DOOR_ROYAL: usize = 11;
    pub const DOOR_ROYAL_EMBLEM: usize = 12;
    pub const PORTAL: usize = 13;
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
}

/// Cases de la planche de projectiles (dessinés vers la droite, orientés par le jeu).
pub mod projectile {
    pub const ORB_A: usize = 0;
    pub const ORB_B: usize = 1;
    pub const ARROW: usize = 2;
    pub const BOLT: usize = 3;
}

/// Cases de la planche d'effets de coups.
pub mod fx {
    pub const SLASH: usize = 0;
    pub const SMASH: usize = 1;
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

/// Case de la planche des héros: ligne = classe * 8 + direction.
pub fn hero_index(class: HeroClass, direction: usize, frame: usize) -> usize {
    (class_row(class) * 8 + direction) * HERO_FRAMES + frame
}

/// Direction parmi les huit de la planche (0 = est, puis sens trigonométrique, 6 = sud).
pub fn direction_index(facing: Vec2) -> usize {
    ((facing.to_angle() / FRAC_PI_4).round() as i32).rem_euclid(8) as usize
}

/// Case de la planche des ennemis: ligne = monstre * 8 + direction, comme pour les héros.
pub fn monster_index(kind: MonsterKind, direction: usize, frame: usize) -> usize {
    let row = match kind {
        MonsterKind::Skeleton => 0,
        MonsterKind::Bat => 1,
        MonsterKind::Goblin => 2,
        MonsterKind::Orc => 3,
        MonsterKind::Ghoul => 4,
    };
    (row * 8 + direction) * MONSTER_FRAMES + frame
}

/// Case de la planche d'objets pour une ligne (`object::*`) et une image d'animation.
pub fn object_index(row: usize, frame: usize) -> usize {
    row * PROP_FRAMES + frame
}

/// Les objets et les ennemis posent leur point d'origine au sol, vers le bas de la case.
pub const PROP_ANCHOR: Anchor = Anchor(Vec2::new(0.0, -0.417));

/// Les panneaux de mur sont dessinés centrés dans leur case (vus de face), pas posés au sol.
pub const WALL_ANCHOR: Anchor = Anchor(Vec2::ZERO);

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
    let mut atlas = |file: &str, columns: u32, rows: u32, cell: u32, smooth: bool| Atlas {
        // Les tuiles et le pixel-art restent nets (filtrage au plus proche); les rendus détaillés (héros,
        // ennemis, objets) sont réduits à l'écran et sont donc lissés.
        image: if smooth {
            assets
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.sampler = ImageSampler::linear();
                })
                .load(format!("art/{folder}/{file}"))
        } else {
            assets.load(format!("art/{folder}/{file}"))
        },
        layout: layouts.add(TextureAtlasLayout::from_grid(UVec2::splat(cell), columns, rows, None, None)),
    };
    Art {
        tiles: atlas("tiles.png", TILE_COLUMNS as u32, 6, TILE_CELL, false),
        tiles_blur: [
            atlas("tiles_blur1.png", TILE_COLUMNS as u32, 6, TILE_CELL, false),
            atlas("tiles_blur2.png", TILE_COLUMNS as u32, 6, TILE_CELL, false),
            atlas("tiles_blur3.png", TILE_COLUMNS as u32, 6, TILE_CELL, false),
        ],
        heroes: atlas("heroes.png", HERO_FRAMES as u32, 32, HERO_CELL, true),
        monsters: atlas("monsters.png", MONSTER_FRAMES as u32, MONSTER_ROWS, HERO_CELL, true),
        objects: atlas("objects.png", PROP_FRAMES as u32, 25, HERO_CELL, true),
        projectiles: atlas("projectiles.png", 4, 1, 64, true),
        fx: atlas("fx.png", 2, 1, 96, true),
        decals: atlas("decals.png", 13, 1, 128, true),
        portraits: atlas("portraits.png", 4, 1, 128, true),
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
        let mut pairs: Vec<(&Atlas, &Atlas)> = vec![
            (&old.tiles, &new.tiles),
            (&old.heroes, &new.heroes),
            (&old.monsters, &new.monsters),
            (&old.objects, &new.objects),
            (&old.projectiles, &new.projectiles),
            (&old.fx, &new.fx),
            (&old.decals, &new.decals),
        ];
        pairs.extend(old.tiles_blur.iter().zip(new.tiles_blur.iter()));
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

/// Tuile du décor, effacée quand le château est regénéré.
#[derive(Component)]
struct WorldTile;

/// Niveau de flou d'une tuile: 0 nette, puis 1 à 3 de plus en plus floue.
#[derive(Component)]
struct TileBlur(u8);

/// Distances (en unités du monde) à partir desquelles le décor passe au flou 1, 2 puis 3.
const BLUR_RINGS: [f32; 3] = [110.0, 150.0, 190.0];

/// Teint les tuiles de la couleur du héros dirigé (multiplication de la couleur): seul le décor change, les
/// personnages gardent leurs couleurs. Appliqué quand la teinte bouge, et à toute nouvelle tuile.
fn tint_tiles(
    grade: Res<crate::lighting::Grade>,
    fresh: Query<(), Added<TileBlur>>,
    mut tiles: Query<&mut Sprite, With<TileBlur>>,
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

/// Floute le décor loin du centre de la vue. Les anneaux suivent la caméra (lissée) et non le héros: en changeant
/// de héros, ils glissent avec l'image au lieu de sauter, et le décor ne change pas d'un coup par plaques.
fn update_tile_blur(
    art: Res<Art>,
    camera: Single<&Transform, With<Camera2d>>,
    mut tiles: Query<(&Transform, &mut Sprite, &mut TileBlur), Without<Camera2d>>,
) {
    let at = camera.translation.truncate();
    for (transform, mut sprite, mut blur) in &mut tiles {
        let distance = transform.translation.truncate().distance(at);
        let level = BLUR_RINGS.iter().filter(|ring| distance > **ring).count() as u8;
        if blur.0 != level {
            blur.0 = level;
            sprite.image = match level {
                0 => art.tiles.image.clone(),
                n => art.tiles_blur[n as usize - 1].image.clone(),
            };
        }
    }
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
) {
    // Le décor se reconstruit quand un nouveau château arrive, ou quand on change de mode.
    if fresh.is_empty() && !theme.is_changed() {
        return;
    }
    let Some(expedition) = expeditions.iter().last() else {
        return;
    };
    info!("building castle from seed {} ({:?})", expedition.seed, expedition.region);
    for entity in &old_tiles {
        commands.entity(entity).despawn();
    }
    let dungeon = Dungeon::generate(expedition.seed);
    let room = &dungeon.room;
    let region_row = Region::ALL.iter().position(|r| *r == expedition.region).unwrap_or(0);
    let size = room.size();
    for y in 0..size.y {
        for x in 0..size.x {
            let (xi, yi) = (x as i32, y as i32);
            // Le sol et les faces de mur sont de grands motifs (4x4 et 4x1 tuiles): on choisit un motif par bloc,
            // et la tuile à sa place dans le bloc, pour que les dalles et les joints se prolongent d'une tuile à l'autre.
            let block = (x / MACRO).wrapping_mul(73856093) ^ (y / MACRO).wrapping_mul(19349663) ^ (region_row as u32).wrapping_mul(83492791);
            let face_block = (x / MACRO).wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ (region_row as u32).wrapping_mul(83492791);
            let noise = (x * 31 + y * 17 + x * y * 3) % 23;
            let column = match room.tile(xi, yi) {
                Tile::Floor => {
                    let pattern = (block >> 5) % FLOOR_PATTERNS;
                    // La ligne 0 du motif est en haut, alors que `y` croît vers le haut.
                    (pattern * MACRO * MACRO + (MACRO - 1 - y % MACRO) * MACRO + x % MACRO) as usize
                }
                Tile::Wall if room.tile(xi, yi - 1) == Tile::Floor => {
                    if noise % 9 == 0 {
                        DECOR_START + (noise as usize / 9) % 2
                    } else {
                        FACE_START + (((face_block >> 5) % FACE_PATTERNS) * MACRO + x % MACRO) as usize
                    }
                }
                Tile::Wall => {
                    let pattern = (block >> 7) % TOP_PATTERNS;
                    (TOP_START as u32 + pattern * MACRO * MACRO + (MACRO - 1 - y % MACRO) * MACRO + x % MACRO) as usize
                }
            };
            let center = room.tile_center(x, y);
            commands.spawn((
                WorldTile,
                TileBlur(0),
                art.tiles.sprite_sized(region_row * TILE_COLUMNS + column, Vec2::splat(TILE_SIZE)),
                Transform::from_translation(center.extend(0.0)),
            ));
            // Ombre portée sur le sol le long d'un mur: au-dessus (face du mur) et sur les côtés.
            if room.tile(xi, yi) == Tile::Floor {
                let mut shade = |offset: Vec2, size: Vec2, alpha: f32| {
                    commands.spawn((
                        WorldTile,
                        Sprite::from_color(Color::srgba(0.02, 0.0, 0.06, alpha), size),
                        Transform::from_translation((center + offset).extend(0.05)),
                    ));
                };
                let half = TILE_SIZE / 2.0;
                if room.tile(xi, yi + 1) == Tile::Wall {
                    shade(Vec2::new(0.0, half - 3.0), Vec2::new(TILE_SIZE, 6.0), 0.4);
                    shade(Vec2::new(0.0, half - 7.0), Vec2::new(TILE_SIZE, 2.0), 0.2);
                }
                if room.tile(xi - 1, yi) == Tile::Wall {
                    shade(Vec2::new(-half + 1.5, 0.0), Vec2::new(3.0, TILE_SIZE), 0.3);
                }
                if room.tile(xi + 1, yi) == Tile::Wall {
                    shade(Vec2::new(half - 1.5, 0.0), Vec2::new(3.0, TILE_SIZE), 0.3);
                }
            }
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
    mut heroes: Query<(&mut Sprite, &Transform, &HeroClass, &Facing, &Strikes, &mut WalkCycle)>,
) {
    let dt = time.delta_secs();
    for (mut sprite, transform, class, facing, strikes, mut cycle) in &mut heroes {
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
            (time.elapsed_secs() * 3.0) as usize % HERO_IDLE_FRAMES
        };
        let index = hero_index(*class, direction_index(facing.0), frame);
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
    mut monsters: Query<(&mut Sprite, &Position, &Transform, &Monster, &mut WalkCycle)>,
) {
    let dt = time.delta_secs();
    for (mut sprite, pos, transform, monster, mut cycle) in &mut monsters {
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
        let frame = match monster.0 {
            MonsterKind::Bat => HERO_IDLE_FRAMES + (time.elapsed_secs() * 12.0) as usize % HERO_WALK_FRAMES,
            _ if cycle.moving_for > 0.0 => HERO_IDLE_FRAMES + (cycle.phase * HERO_WALK_FRAMES as f32) as usize % HERO_WALK_FRAMES,
            _ => (time.elapsed_secs() * 3.0) as usize % HERO_IDLE_FRAMES,
        };
        let index = monster_index(monster.0, cycle.direction, frame);
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            if atlas.index != index {
                atlas.index = index;
            }
        }
    }
}

/// Animation des objets: quatre images (impulsion de lumière, pièce qui tourne, engrenages...).
fn animate_props(time: Res<Time>, mut props: Query<(&mut Sprite, &Prop)>) {
    let frame = (time.elapsed_secs() * 5.0) as usize % PROP_FRAMES;
    for (mut sprite, prop) in &mut props {
        let index = object_index(prop.0, frame);
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            if atlas.index != index {
                atlas.index = index;
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
        _ => [c(0.28, 0.04, 0.08, 0.22), c(0.1, 0.05, 0.1, 0.24)],   // vin renversé, cire
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
    fn every_monster_has_its_own_rows_inside_the_sheet() {
        let kinds = [MonsterKind::Skeleton, MonsterKind::Bat, MonsterKind::Goblin, MonsterKind::Orc, MonsterKind::Ghoul];
        let mut seen = std::collections::HashSet::new();
        for kind in kinds {
            for direction in 0..8 {
                for frame in 0..MONSTER_FRAMES {
                    let index = monster_index(kind, direction, frame);
                    assert!(index < MONSTER_ROWS as usize * MONSTER_FRAMES, "{kind:?} hors de la planche");
                    assert!(seen.insert(index), "case en double pour {kind:?}");
                }
            }
        }
        // La planche est entièrement utilisée: autant de lignes que de monstres et de directions.
        assert_eq!(seen.len(), MONSTER_ROWS as usize * MONSTER_FRAMES);
    }
}
