//! Les héros et les monstres sont dessinés en 3D par la carte graphique, à partir des modèles de `sprite_forge`, au lieu d'être
//! lus dans des planches de sprites. Actif par défaut sur ordinateur (option de compilation `live3d`, activée par défaut);
//! `CASTLE_LIVE3D=0` revient aux sprites.
//!
//! Avec le décor en 3D (`CASTLE_DECOR3D=1`, voir `decor3d`), les modèles (personnages, et objets qui en ont un) ne passent plus par
//! l'atlas: ce sont des entités de la scène du décor, posées sur le sol à leur position réelle. Ils sont donc vus sous l'angle et
//! l'inclinaison choisis, nets à tout cadrage, et masqués par les murs par la profondeur.
//!
//! Chaque personnage a une case dans une grande image (`ATLAS`): une caméra 3D à part y dessine, chaque image, tous les modèles
//! posés sur une grille. Dans le monde 2D, le personnage reste un sprite qui montre sa case: il garde donc sa place dans le tri
//! par hauteur avec le décor et les grands motifs, ses ombres et sa teinte.

use bevy::{
    camera::{ClearColorConfig, RenderTarget, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType, TextureFormat},
    shader::ShaderRef,
};
use castle_shared::{ExitKind, HeroClass, Landmark, LandmarkKind, LevelExit, Monster};
use sprite_forge::{Anim, LivePart, LiveShape, Theme, live_exit, live_landmark, live_parts};

use crate::art::{EXIT_FRAMES, HERO_ANCHOR, PROP_ANCHOR, PoseKind, Prop, SPRITE_SIZE, WalkCycle, exit_row, monster_scale, object};

/// Cases de l'image: `GRID` x `GRID` de `SLOT` pixels, chacune montrant un carré de `CELL_M` mètres de côté.
const GRID: usize = 12;
const SLOT: usize = 240;
const CELL_M: f32 = 3.6;
/// Les planches de sprites montraient 2,4 mètres par case: la case de l'image, plus large (un loup de profil dépassait de l'ancienne
/// et son museau était coupé), s'affiche donc plus grande dans le même rapport.
const CELL_SCALE: f32 = CELL_M / 2.4;
/// Inclinaison de la vue (la même que celle des planches de sprites).
const ELEVATION: f32 = 0.42;
/// Où les pieds tombent dans la case, depuis le haut (comme dans les planches: 88 pixels sur 96).
const FEET: f32 = 88.0 / 96.0;

/// Les personnages sont-ils dessinés en 3D? `CASTLE_LIVE3D=1` ou `0` force le choix; sinon oui, sauf sur Apple TV et sur les appareils
/// à peu de mémoire, qui gardent les planches de sprites.
pub fn enabled() -> bool {
    match std::env::var("CASTLE_LIVE3D").as_deref() {
        Ok("0") => false,
        Ok(_) => true,
        Err(_) => !crate::settings::is_tv() && !crate::settings::low_memory(),
    }
}

/// Les modèles sont-ils dessinés directement dans la scène du décor en 3D (et non dans l'atlas)?
fn direct() -> bool {
    crate::decor3d::enabled()
}

/// Un personnage ou un objet dont le modèle est dans la scène du décor: son sprite n'est plus dessiné à plat.
#[derive(Component)]
pub struct Direct3d;

pub struct Live3dPlugin;

impl Plugin for Live3dPlugin {
    fn build(&self, app: &mut App) {
        if !enabled() && !direct() {
            return;
        }
        app.add_plugins(MaterialPlugin::<ToonMaterial>::default()).init_resource::<Caches>().init_resource::<Slots>().add_observer(release);
        if direct() {
            app.add_systems(Update, (attach_direct, attach_props, attach_landmarks, attach_exits, animate).chain().after(crate::art::AnimateCharacters));
        } else {
            app.add_systems(Startup, setup)
                .add_systems(Update, (attach, animate).chain().after(crate::art::AnimateCharacters))
                .add_systems(Update, shoot);
        }
    }
}

/// Le matériau « toon » de tous les personnages et objets: sans réglage, la couleur, l'éclat et le reflet viennent des sommets du
/// maillage fusionné (voir `merged`), donc un seul matériau sert tout le monde.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
struct ToonMaterial {}

impl Material for ToonMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon.wesl".into()
    }
}

#[derive(Resource, Default)]
struct Caches {
    /// Le maillage fusionné de chaque pose déjà vue: (modèle, famille de pose, image de la boucle).
    poses: std::collections::HashMap<(ModelSource, u8, u16), Handle<Mesh>>,
    material: Option<Handle<ToonMaterial>>,
}

/// Au-delà de ce nombre de poses gardées, on repart de zéro: les maillages en cours d'usage restent tenus par leur personnage.
const MAX_CACHED_POSES: usize = 600;

/// L'image où se dessinent les personnages, et les cases libres.
#[derive(Resource, Default)]
struct Slots {
    atlas: Handle<Image>,
    free: Vec<usize>,
}

/// Un personnage dessiné en 3D: son modèle, sa case, et ses pièces.
#[derive(Component)]
pub struct Live3d {
    source: ModelSource,
    /// Case de l'atlas, ou `DIRECT` quand le modèle est dans la scène du décor.
    slot: usize,
    root: Entity,
    /// L'entité qui porte le maillage fusionné, et la pose qu'il montre.
    body: Option<Entity>,
    shown: Option<(ModelSource, u8, u16)>,
    /// Unités du monde par mètre du modèle (scène du décor seulement).
    scale: f32,
}

const DIRECT: usize = usize::MAX;

/// D'où vient le modèle d'un personnage ou d'un objet dans `sprite_forge`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum ModelSource {
    /// Un modèle réglable (héros, monstre, objet): sa clé, voir `sprite_forge::models`.
    Key(String),
    /// Un grand motif du décor: son indice dans `LandmarkKind::ALL`.
    Landmark(usize),
    /// Une sortie de niveau: la ligne de sa planche (voir `art::exit_row`).
    Exit(usize),
}

impl ModelSource {
    /// La pose du modèle la plus proche de `anim` parmi celles qu'il a vraiment (les images de ses planches): sa famille, son indice
    /// dans la boucle, et la pose exacte à construire.
    fn pose(&self, anim: Anim) -> (u8, u16, Anim) {
        let (Anim::Idle(t) | Anim::Walk(t) | Anim::Attack(t)) = anim;
        let (steps, family) = match (self, anim) {
            (Self::Landmark(_), _) => return (0, 0, Anim::Idle(0.0)),
            (Self::Exit(_), _) => (EXIT_FRAMES, 0u8),
            (Self::Key(key), Anim::Idle(_)) if key.starts_with("prop:") => (20, 0),
            (_, Anim::Idle(_)) => (25, 0),
            (_, Anim::Walk(_)) => (16, 1),
            (_, Anim::Attack(_)) => (10, 2),
        };
        let index = ((t.clamp(0.0, 0.9999) * steps as f32) as usize).min(steps - 1);
        let t = index as f32 / steps as f32;
        let exact = match (self, family) {
            (Self::Key(_), 1) => Anim::Walk(t),
            (Self::Key(_), 2) => Anim::Attack(t),
            _ => Anim::Idle(t),
        };
        (family, index as u16, exact)
    }

    /// Les pièces du modèle dans la pose donnée (pour une sortie, `Idle(t)` donne le point de sa boucle d'animation).
    fn parts(&self, anim: Anim) -> Vec<LivePart> {
        match self {
            Self::Key(key) => live_parts(key, Theme::Medieval, anim),
            Self::Landmark(index) => live_landmark(*index),
            Self::Exit(row) => {
                let (Anim::Idle(t) | Anim::Walk(t) | Anim::Attack(t)) = anim;
                live_exit(*row, t, Theme::Medieval)
            }
        }
    }
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut slots: ResMut<Slots>) {
    let side = (GRID * SLOT) as u32;
    let atlas = images.add(Image::new_target_texture(side, side, TextureFormat::Rgba8UnormSrgb, None));
    slots.atlas = atlas.clone();
    slots.free = (0..GRID * GRID).rev().collect();
    // La caméra voit toute la grille (un carré de GRID x GRID cases) de la même façon que les planches: vue orthographique inclinée.
    commands.spawn((
        Camera3d::default(),
        Camera { order: -10, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
        RenderTarget::Image(atlas.into()),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: GRID as f32 * CELL_M },
            ..OrthographicProjection::default_3d()
        }),
        Tonemapping::None,
        Msaa::Sample4,
        Transform::from_translation(Vec3::new(0.0, ELEVATION.sin(), ELEVATION.cos()) * 500.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Où poser le modèle de la case `slot` pour que ses pieds tombent à leur place dans la case.
fn slot_origin(slot: usize) -> Vec3 {
    let (column, row) = ((slot % GRID) as f32, (slot / GRID) as f32);
    let half = GRID as f32 * CELL_M / 2.0;
    let x = -half + (column + 0.5) * CELL_M;
    // Hauteur à l'écran des pieds, puis la profondeur qui y mène (le sol incliné: monter d'un mètre à l'écran recule de 1/sin).
    let screen_y = half - (row + FEET) * CELL_M;
    Vec3::new(x, 0.0, -screen_y / ELEVATION.sin())
}

fn slot_rect(slot: usize) -> Rect {
    let (column, row) = ((slot % GRID) as f32, (slot / GRID) as f32);
    Rect::new(column * SLOT as f32, row * SLOT as f32, (column + 1.0) * SLOT as f32, (row + 1.0) * SLOT as f32)
}

/// Donne une case et un modèle à chaque héros et monstre qui vient d'apparaître, et remplace son sprite par celui de la case.
fn attach(
    mut commands: Commands,
    mut slots: ResMut<Slots>,
    fresh: Query<(Entity, Option<&HeroClass>, Option<&Monster>), Added<WalkCycle>>,
) {
    for (entity, class, monster) in &fresh {
        let (key, anchor, size) = match (class, monster) {
            (Some(class), _) => (format!("hero:{class:?}"), HERO_ANCHOR, Vec2::splat(SPRITE_SIZE)),
            (None, Some(monster)) => (format!("monster:{:?}", monster.0), PROP_ANCHOR, Vec2::splat(SPRITE_SIZE * monster_scale(monster.0))),
            _ => continue,
        };
        let Some(slot) = slots.free.pop() else {
            continue;
        };
        let root = commands.spawn((Transform::from_translation(slot_origin(slot)), Visibility::default())).id();
        commands.entity(entity).insert((
            Sprite { image: slots.atlas.clone(), rect: Some(slot_rect(slot)), custom_size: Some(size * CELL_SCALE), ..default() },
            anchor,
            Live3d { source: ModelSource::Key(key), slot, root, body: None, shown: None, scale: 1.0 },
        ));
    }
}

/// Un héros ou un monstre qui vient d'apparaître reçoit son modèle dans la scène du décor.
fn attach_direct(mut commands: Commands, fresh: Query<(Entity, Option<&HeroClass>, Option<&Monster>), Added<WalkCycle>>) {
    for (entity, class, monster) in &fresh {
        let (key, size) = match (class, monster) {
            (Some(class), _) => (format!("hero:{class:?}"), SPRITE_SIZE),
            (None, Some(monster)) => (format!("monster:{:?}", monster.0), SPRITE_SIZE * monster_scale(monster.0)),
            _ => continue,
        };
        let root = spawn_root(&mut commands);
        commands.entity(entity).insert((Direct3d, Live3d { source: ModelSource::Key(key), slot: DIRECT, root, body: None, shown: None, scale: size * CELL_SCALE / CELL_M }));
    }
}

/// Le modèle de `sprite_forge` d'un objet, quand il en a un et qu'il se pose au sol (pas les panneaux de mur, ni les portes et
/// herses, dont l'orientation dépend de leur place).
fn prop_model(row: usize) -> Option<String> {
    const GROUND: [usize; 14] = [
        object::GENERATOR,
        object::TURRET,
        object::COIN,
        object::FOOD,
        object::KEY,
        object::TRAP_ARMED,
        object::TRAP_OFF,
        object::POTION,
        object::INGREDIENT,
        object::GEAR_PART,
        object::TARGET_DISC,
        object::CAPTIVE,
        object::SAMELIA,
        object::SAMELIA_CAGED,
    ];
    if !GROUND.contains(&row) {
        return None;
    }
    // Les modèles d'objets sont listés dans l'ordre des planches (`object::FILES`).
    sprite_forge::models().into_iter().filter(|model| model.group == "Objets").nth(row).map(|model| model.key)
}

/// Un objet qui vient d'apparaître reçoit son modèle dans la scène du décor, quand il en a un.
fn attach_props(mut commands: Commands, fresh: Query<(Entity, &Prop), (Added<Prop>, Without<Live3d>)>) {
    for (entity, prop) in &fresh {
        let Some(key) = prop_model(prop.0) else {
            continue;
        };
        let root = spawn_root(&mut commands);
        // Un objet occupe une case de 32 unités qui montre 2,4 mètres.
        commands.entity(entity).insert((Direct3d, Live3d { source: ModelSource::Key(key), slot: DIRECT, root, body: None, shown: None, scale: SPRITE_SIZE / 2.4 }));
    }
}

/// Un grand motif du décor (rocher, colonne, statue, fontaine) qui vient d'apparaître reçoit son modèle dans la scène.
fn attach_landmarks(mut commands: Commands, fresh: Query<(Entity, &Landmark), (Added<Landmark>, Without<Live3d>)>) {
    for (entity, landmark) in &fresh {
        let Some(index) = LandmarkKind::ALL.iter().position(|kind| *kind == landmark.0) else {
            continue;
        };
        let root = spawn_root(&mut commands);
        // Sa planche montre 4,8 mètres sur 64 unités du monde (la fontaine est dessinée plus petite dans sa case).
        let scale = 64.0 * landmark.0.display_scale() / 4.8;
        commands.entity(entity).insert((Direct3d, Live3d { source: ModelSource::Landmark(index), slot: DIRECT, root, body: None, shown: None, scale }));
    }
}

/// Une sortie de niveau qui vient d'apparaître reçoit son modèle dans la scène (pas celles que le jeu ne dessine pas).
fn attach_exits(mut commands: Commands, fresh: Query<(Entity, &LevelExit), (Added<LevelExit>, Without<Live3d>)>) {
    for (entity, exit) in &fresh {
        if exit.kind == ExitKind::Hidden {
            continue;
        }
        let root = spawn_root(&mut commands);
        // Sa planche montre 4,8 mètres sur 64 unités du monde.
        commands.entity(entity).insert((
            Direct3d,
            Live3d { source: ModelSource::Exit(exit_row(exit.kind, exit.open)), slot: DIRECT, root, body: None, shown: None, scale: 64.0 / 4.8 },
        ));
    }
}

fn spawn_root(commands: &mut Commands) -> Entity {
    commands.spawn((Transform::default(), Visibility::default(), RenderLayers::layer(crate::decor3d::LAYER))).id()
}

/// Un personnage disparu rend sa case et retire son modèle.
fn release(removed: On<Remove<Live3d>>, characters: Query<&Live3d>, mut slots: ResMut<Slots>, mut commands: Commands) {
    if let Ok(live) = characters.get(removed.entity) {
        commands.entity(live.root).despawn();
        if live.slot != DIRECT {
            slots.free.push(live.slot);
        }
    }
}

/// Met chaque modèle dans la pose du sprite (repos, marche, coup) et tourne son regard dans le sens où le personnage va.
fn animate(
    mut commands: Commands,
    time: Res<Time>,
    mut characters: Query<(Entity, &mut Live3d, Option<&WalkCycle>, Option<&crate::Facing>, Option<&Prop>, Option<&LevelExit>, &InheritedVisibility)>,
    mut transforms: Query<&mut Transform>,
    mut shown: Query<&mut Visibility>,
    mut caches: ResMut<Caches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ToonMaterial>>,
) {
    let material = caches.material.get_or_insert_with(|| materials.add(ToonMaterial::default())).clone();
    if caches.poses.len() > MAX_CACHED_POSES {
        caches.poses.clear();
    }
    for (entity, mut live, cycle, facing, prop, exit, visible) in &mut characters {
        let anim = match cycle {
            Some(cycle) => {
                let (kind, t) = cycle.pose;
                match kind {
                    PoseKind::Idle => Anim::Idle(t),
                    PoseKind::Walk => Anim::Walk(t),
                    PoseKind::Attack => Anim::Attack(t),
                }
            }
            // Une sortie: sa boucle de dix images (torches et bannières qui vacillent), comme sa planche.
            None if exit.is_some() => Anim::Idle(((time.elapsed_secs() * 12.0) as usize % EXIT_FRAMES) as f32 / EXIT_FRAMES as f32),
            // Un objet: vingt images en boucle, comme sa planche.
            None => Anim::Idle((time.elapsed_secs() * 1.25).fract()),
        };
        // Un objet qui change d'état (un piège qui se désarme...) change de modèle.
        let changed = match (prop, exit) {
            (Some(prop), _) => prop_model(prop.0).map(ModelSource::Key),
            // Une sortie qui s'ouvre (pont réparé, barrière ouverte...) change de modèle.
            (_, Some(exit)) => Some(ModelSource::Exit(exit_row(exit.kind, exit.open))),
            _ => None,
        };
        if let Some(source) = changed.filter(|source| *source != live.source) {
            live.source = source;
        }
        // Le regard: les héros ont le leur (répliqué); les monstres suivent leur dernier déplacement (une des huit directions).
        let look = match (facing, cycle) {
            (Some(facing), _) if facing.0.length_squared() > 0.01 => facing.0.normalize(),
            (_, Some(cycle)) => Vec2::from_angle(cycle.direction as f32 * std::f32::consts::FRAC_PI_4),
            _ => Vec2::NEG_Y,
        };
        let yaw = look.x.atan2(-look.y);
        let at = transforms.get(entity).map(|at| at.translation.truncate()).unwrap_or_default();
        if let Ok(mut root) = transforms.get_mut(live.root) {
            root.rotation = if cycle.is_some() { Quat::from_rotation_y(yaw) } else { Quat::IDENTITY };
            if live.slot == DIRECT {
                // Dans la scène du décor: sur le sol à la position du personnage (le nord est -z), à l'échelle du monde.
                root.translation = Vec3::new(at.x, 0.0, -at.y);
                root.scale = Vec3::splat(live.scale);
            }
        }
        if live.slot == DIRECT {
            if let Ok(mut visibility) = shown.get_mut(live.root) {
                let wanted = if visible.get() { Visibility::Inherited } else { Visibility::Hidden };
                if *visibility != wanted {
                    *visibility = wanted;
                }
            }
        }
        // Le maillage de la pose: construit une fois par pose, puis repris du cache.
        let (family, index, exact) = live.source.pose(anim);
        let wanted = (live.source.clone(), family, index);
        if live.shown.as_ref() == Some(&wanted) {
            continue;
        }
        let handle = caches
            .poses
            .entry(wanted.clone())
            .or_insert_with(|| meshes.add(crate::merged::merge(&live.source.parts(exact))))
            .clone();
        match live.body {
            Some(body) => {
                commands.entity(body).insert(Mesh3d(handle));
            }
            None => {
                let mut spawned = commands.spawn((Mesh3d(handle), MeshMaterial3d(material.clone()), ChildOf(live.root)));
                if live.slot == DIRECT {
                    spawned.insert(RenderLayers::layer(crate::decor3d::LAYER));
                }
                live.body = Some(spawned.id());
            }
        }
        live.shown = Some(wanted);
    }
}

/// Outil d'essai: avec `CASTLE_SHOT=<fichier.png>`, enregistre une capture de la fenêtre au bout de vingt secondes, puis toutes les six
/// secondes (le nom reçoit le numéro de la capture avant `.png`).
fn shoot(mut commands: Commands, time: Res<Time>, mut taken: Local<u32>) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let Some(path) = std::env::var_os("CASTLE_SHOT") else {
        return;
    };
    let due = 20.0 + *taken as f32 * 6.0;
    if time.elapsed_secs() > due && *taken < 8 {
        let name = path.to_string_lossy().replace(".png", &format!("_{}.png", *taken));
        *taken += 1;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(name));
    }
}
