//! Les portes et les herses, posées dans la scène 3D comme le reste du décor: des volumes éclairés par les mêmes lampes et le même
//! soleil que les modèles, alignés sur le couloir qu'ils ferment (et non plus des sprites à plat, de travers dans la vue).
//!
//! Une porte ou une herse occupe deux tuiles en travers du couloir. Ses pièces sont construites le long de l'axe X local (la largeur du
//! couloir) et de l'axe Z local (l'épaisseur), sur une hauteur de 1; la racine les tourne dans le sens du couloir et les étire à la
//! hauteur d'un mur.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use castle_shared::{Door, DoorKind, Portcullis, Position, TILE_SIZE};

use super::{LAYER, to_3d, wall_height};

/// La racine des pièces d'une porte ou d'une herse: elle disparaît avec l'objet du monde qu'elle représente.
#[derive(Component)]
pub struct GateRoot(Entity);

/// Largeur d'une porte, en travers du couloir.
const SPAN: f32 = TILE_SIZE * 2.0;
/// Épaisseur du vantail ou des barreaux.
const THICKNESS: f32 = 5.0;
/// Épaisseur des montants de pierre qui encadrent la porte (un peu plus que le vantail).
const POST: f32 = 7.0;

struct Palette {
    stone: Handle<StandardMaterial>,
    leaf: Handle<StandardMaterial>,
    band: Handle<StandardMaterial>,
    boss: Handle<StandardMaterial>,
}

fn material(standard: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32, metallic: f32) -> Handle<StandardMaterial> {
    standard.add(StandardMaterial { base_color: Color::srgb(r, g, b), perceptual_roughness: 0.75, metallic, ..default() })
}

/// Pose les pièces d'une porte ou d'une herse sous une racine à la place de `owner`.
fn spawn_gate(commands: &mut Commands, owner: Entity, center: Vec2, vertical: bool, parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>) {
    // Le couloir « vertical » s'étend du nord au sud: la porte, en travers, est alors le long de Y.
    let rotation = if vertical { Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
    commands
        .spawn((
            GateRoot(owner),
            Transform { translation: to_3d(center, 0.0), rotation, scale: Vec3::new(1.0, wall_height(), 1.0) },
            Visibility::default(),
            RenderLayers::layer(LAYER),
        ))
        .with_children(|root| {
            for (mesh, material, transform) in parts {
                root.spawn((Mesh3d(mesh), MeshMaterial3d(material), transform, RenderLayers::layer(LAYER)));
            }
        });
}

/// Les deux montants de pierre et le linteau qui encadrent le passage.
fn frame(meshes: &mut Assets<Mesh>, stone: &Handle<StandardMaterial>) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> {
    let post = meshes.add(Cuboid::new(3.0, 1.0, POST));
    let lintel = meshes.add(Cuboid::new(SPAN, 0.08, POST));
    let side = SPAN / 2.0 - 1.5;
    vec![
        (post.clone(), stone.clone(), Transform::from_xyz(-side, 0.5, 0.0)),
        (post, stone.clone(), Transform::from_xyz(side, 0.5, 0.0)),
        (lintel, stone.clone(), Transform::from_xyz(0.0, 0.96, 0.0)),
    ]
}

/// Les portes qui viennent d'apparaître: la porte à clé est en bois cerclé de fer, la porte royale en pierre violette cerclée d'or avec
/// un emblème au centre.
pub fn spawn_doors(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    doors: Query<(Entity, &Position, &Door), Added<Door>>,
) {
    for (entity, position, door) in &doors {
        let palette = match door.kind {
            DoorKind::Locked => Palette {
                stone: material(&mut standard, 0.30, 0.29, 0.31, 0.0),
                leaf: material(&mut standard, 0.30, 0.18, 0.09, 0.0),
                band: material(&mut standard, 0.12, 0.12, 0.14, 0.8),
                boss: material(&mut standard, 0.75, 0.55, 0.15, 0.8),
            },
            DoorKind::Royal => Palette {
                stone: material(&mut standard, 0.30, 0.29, 0.31, 0.0),
                leaf: material(&mut standard, 0.30, 0.12, 0.42, 0.0),
                band: material(&mut standard, 0.80, 0.60, 0.18, 0.8),
                boss: material(&mut standard, 0.95, 0.78, 0.30, 0.8),
            },
        };
        let mut parts = frame(&mut meshes, &palette.stone);
        let leaf = meshes.add(Cuboid::new(SPAN - 6.0, 0.92, THICKNESS));
        parts.push((leaf, palette.leaf.clone(), Transform::from_xyz(0.0, 0.46, 0.0)));
        let band = meshes.add(Cuboid::new(SPAN - 6.0, 0.05, THICKNESS + 0.8));
        for height in [0.2, 0.5, 0.8] {
            parts.push((band.clone(), palette.band.clone(), Transform::from_xyz(0.0, height, 0.0)));
        }
        // Au centre: la serrure de la porte à clé, ou l'emblème (un losange) de la porte royale.
        match door.kind {
            DoorKind::Locked => {
                let lock = meshes.add(Cuboid::new(4.0, 0.09, THICKNESS + 1.6));
                parts.push((lock, palette.boss.clone(), Transform::from_xyz(0.0, 0.46, 0.0)));
            }
            DoorKind::Royal => {
                let emblem = meshes.add(Cuboid::new(7.0, 0.2, THICKNESS + 1.6));
                parts.push((emblem, palette.boss.clone(), Transform::from_xyz(0.0, 0.5, 0.0).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4))));
            }
        }
        spawn_gate(&mut commands, entity, position.0, door.vertical, parts);
    }
}

/// Les herses qui viennent d'apparaître: des barreaux de fer et trois traverses.
pub fn spawn_portcullises(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    gates: Query<(Entity, &Position, &Portcullis), Added<Portcullis>>,
) {
    for (entity, position, gate) in &gates {
        let stone = material(&mut standard, 0.30, 0.29, 0.31, 0.0);
        let iron = material(&mut standard, 0.14, 0.14, 0.16, 0.9);
        let mut parts = frame(&mut meshes, &stone);
        let bar = meshes.add(Cuboid::new(1.2, 0.92, 1.2));
        let count = 9;
        for i in 0..count {
            let x = (i as f32 / (count - 1) as f32 - 0.5) * (SPAN - 8.0);
            parts.push((bar.clone(), iron.clone(), Transform::from_xyz(x, 0.46, 0.0)));
        }
        let rail = meshes.add(Cuboid::new(SPAN - 6.0, 0.04, 1.8));
        for height in [0.25, 0.55, 0.85] {
            parts.push((rail.clone(), iron.clone(), Transform::from_xyz(0.0, height, 0.0)));
        }
        spawn_gate(&mut commands, entity, position.0, gate.vertical, parts);
    }
}

/// Une porte ouverte ou une herse levée (ou un niveau quitté) retire ses pièces; la hauteur des murs suit la vue.
pub fn update_gates(mut commands: Commands, mut roots: Query<(Entity, &GateRoot, &mut Transform)>, alive: Query<(), Or<(With<Door>, With<Portcullis>)>>) {
    let height = wall_height();
    for (entity, root, mut transform) in &mut roots {
        if !alive.contains(root.0) {
            commands.entity(entity).despawn();
        } else if transform.scale.y != height {
            transform.scale.y = height;
        }
    }
}
