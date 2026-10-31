//! Essai (option de compilation `live3d`, puis `CASTLE_LIVE3D=1`): les héros et les monstres sont dessinés en 3D par la carte
//! graphique, à partir des modèles de `sprite_forge`, au lieu d'être lus dans des planches de sprites.
//!
//! Chaque personnage a une case dans une grande image (`ATLAS`): une caméra 3D à part y dessine, chaque image, tous les modèles
//! posés sur une grille. Dans le monde 2D, le personnage reste un sprite qui montre sa case: il garde donc sa place dans le tri
//! par hauteur avec le décor et les grands motifs, ses ombres et sa teinte.

use bevy::{
    camera::{ClearColorConfig, RenderTarget, ScalingMode},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType, TextureFormat},
    shader::ShaderRef,
};
use castle_shared::{HeroClass, Monster, Position};
use sprite_forge::{Anim, LivePart, LiveShape, Theme, live_parts};

use crate::art::{HERO_ANCHOR, PROP_ANCHOR, PoseKind, SPRITE_SIZE, WalkCycle, monster_scale};

/// Cases de l'image: `GRID` x `GRID` de `SLOT` pixels, chacune montrant un carré de `CELL_M` mètres de côté.
const GRID: usize = 16;
const SLOT: usize = 160;
const CELL_M: f32 = 2.4;
/// Inclinaison de la vue (la même que celle des planches de sprites).
const ELEVATION: f32 = 0.42;
/// Où les pieds tombent dans la case, depuis le haut (comme dans les planches: 88 pixels sur 96).
const FEET: f32 = 88.0 / 96.0;

/// L'essai est-il demandé (`CASTLE_LIVE3D=1`)?
pub fn enabled() -> bool {
    std::env::var_os("CASTLE_LIVE3D").is_some()
}

pub struct Live3dPlugin;

impl Plugin for Live3dPlugin {
    fn build(&self, app: &mut App) {
        if !enabled() {
            return;
        }
        app.add_plugins(MaterialPlugin::<ToonMaterial>::default())
            .init_resource::<Caches>()
            .init_resource::<Slots>()
            .add_systems(Startup, setup)
            .add_systems(Update, (attach, animate).chain().after(crate::art::AnimateCharacters))
            .add_systems(Update, shoot)
            .add_observer(release);
    }
}

#[derive(ShaderType, Clone, Debug)]
struct ToonData {
    color: Vec4,
    params: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct ToonMaterial {
    #[uniform(0)]
    data: ToonData,
    blend: bool,
}

impl Material for ToonMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        if self.blend { AlphaMode::Blend } else { AlphaMode::Opaque }
    }
}

#[derive(Resource, Default)]
struct Caches {
    meshes: std::collections::HashMap<(u8, i32, i32), Handle<Mesh>>,
    materials: std::collections::HashMap<[u8; 6], Handle<ToonMaterial>>,
}

/// L'image où se dessinent les personnages, et les cases libres.
#[derive(Resource, Default)]
struct Slots {
    atlas: Handle<Image>,
    free: Vec<usize>,
}

/// Un personnage dessiné en 3D: son modèle, sa case, et ses pièces.
#[derive(Component)]
pub struct Live3d {
    key: String,
    slot: usize,
    root: Entity,
    parts: Vec<Entity>,
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
            Sprite { image: slots.atlas.clone(), rect: Some(slot_rect(slot)), custom_size: Some(size), ..default() },
            anchor,
            Live3d { key, slot, root, parts: Vec::new() },
        ));
    }
}

/// Un personnage disparu rend sa case et retire son modèle.
fn release(removed: On<Remove, Live3d>, characters: Query<&Live3d>, mut slots: ResMut<Slots>, mut commands: Commands) {
    if let Ok(live) = characters.get(removed.entity) {
        commands.entity(live.root).despawn();
        slots.free.push(live.slot);
    }
}

fn mesh_for(caches: &mut Caches, meshes: &mut Assets<Mesh>, shape: LiveShape) -> Handle<Mesh> {
    let key = match shape {
        LiveShape::Sphere => (0, 0, 0),
        LiveShape::Cube => (1, 0, 0),
        LiveShape::Capsule(_) => (2, 0, 0),
        LiveShape::Frustum(a, b) => (3, (a * 50.0).round() as i32, (b * 50.0).round() as i32),
    };
    caches
        .meshes
        .entry(key)
        .or_insert_with(|| {
            meshes.add(match shape {
                LiveShape::Sphere => Sphere::new(1.0).mesh().uv(20, 12),
                LiveShape::Cube => Cuboid::new(2.0, 2.0, 2.0).into(),
                LiveShape::Capsule(_) => Capsule3d::new(1.0, 2.0).mesh().rings(4).latitudes(8).longitudes(16).build(),
                LiveShape::Frustum(a, b) => ConicalFrustum { radius_top: b, radius_bottom: a, height: 2.0 }.mesh().resolution(20).build(),
            })
        })
        .clone()
}

fn material_for(caches: &mut Caches, materials: &mut Assets<ToonMaterial>, part: &LivePart) -> Handle<ToonMaterial> {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let key = [byte(part.color[0]), byte(part.color[1]), byte(part.color[2]), part.glow as u8, byte(part.shine), 0];
    caches
        .materials
        .entry(key)
        .or_insert_with(|| {
            materials.add(ToonMaterial {
                data: ToonData {
                    color: Vec4::new(part.color[0], part.color[1], part.color[2], 1.0),
                    params: Vec4::new(if part.glow { 1.0 } else { 0.0 }, part.shine, 1.0, 0.0),
                },
                blend: false,
            })
        })
        .clone()
}

fn part_transform(part: &LivePart) -> Transform {
    let mut scale = Vec3::from(part.scale);
    if let LiveShape::Capsule(h) = part.shape {
        // La capsule de référence a une demi-longueur de 2 pour un rayon de 1: on l'étire pour que le total fasse `rayon * (h + 1)`.
        scale.y = part.scale[0] * (h + 1.0) / 2.0;
    }
    Transform { translation: Vec3::from(part.center), rotation: Quat::from_mat3(&Mat3::from_cols_array_2d(&part.rot)), scale }
}

/// Met chaque modèle dans la pose du sprite (repos, marche, coup) et tourne son regard dans le sens où le personnage va.
fn animate(
    mut commands: Commands,
    mut characters: Query<(&mut Live3d, &WalkCycle, Option<&crate::Facing>)>,
    mut transforms: Query<&mut Transform>,
    mut caches: ResMut<Caches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ToonMaterial>>,
) {
    for (mut live, cycle, facing) in &mut characters {
        let (kind, t) = cycle.pose;
        let anim = match kind {
            PoseKind::Idle => Anim::Idle(t),
            PoseKind::Walk => Anim::Walk(t),
            PoseKind::Attack => Anim::Attack(t),
        };
        // Le regard: les héros ont le leur (répliqué); les monstres suivent leur dernier déplacement (une des huit directions).
        let look = match facing {
            Some(facing) if facing.0.length_squared() > 0.01 => facing.0.normalize(),
            _ => Vec2::from_angle(cycle.direction as f32 * std::f32::consts::FRAC_PI_4),
        };
        let yaw = look.x.atan2(-look.y);
        if let Ok(mut root) = transforms.get_mut(live.root) {
            root.rotation = Quat::from_rotation_y(yaw);
        }
        let list = live_parts(&live.key, Theme::Medieval, anim);
        if live.parts.len() != list.len() {
            for old in live.parts.drain(..) {
                commands.entity(old).despawn();
            }
            live.parts = list
                .iter()
                .map(|part| {
                    commands
                        .spawn((
                            Mesh3d(mesh_for(&mut caches, &mut meshes, part.shape)),
                            MeshMaterial3d(material_for(&mut caches, &mut materials, part)),
                            part_transform(part),
                            ChildOf(live.root),
                        ))
                        .id()
                })
                .collect();
            continue;
        }
        for (child, part) in live.parts.iter().zip(&list) {
            if let Ok(mut place) = transforms.get_mut(*child) {
                *place = part_transform(part);
            }
        }
    }
}

/// Outil d'essai: avec `CASTLE_SHOT=<fichier.png>`, enregistre une capture de la fenêtre au bout de vingt secondes.
fn shoot(mut commands: Commands, time: Res<Time>, mut done: Local<bool>) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let Some(path) = std::env::var_os("CASTLE_SHOT") else {
        return;
    };
    if !*done && time.elapsed_secs() > 20.0 {
        *done = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}
