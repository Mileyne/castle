//! Les torches: un support, et une flamme dessinée par un shader (`assets/shaders/flame.wesl`) sur un panneau tourné vers la caméra.

use bevy::{
    camera::visibility::RenderLayers,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

use super::look::{TORCH_HEIGHT, Torch};
use super::{LAYER, to_3d};

/// Une flamme: la matière du shader, avec un décalage propre à chaque torche.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct FlameMaterial {
    /// (décalage de temps, -, -, -).
    #[uniform(0)]
    params: Vec4,
}

impl Material for FlameMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/flame.wesl".into()
    }

    // La flamme s'ajoute à l'image: elle éclaire sans cacher.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

/// Une pièce d'une torche (support ou flamme): elle disparaît avec le niveau.
#[derive(Component)]
pub struct TorchMark;

/// Le panneau de la flamme d'une torche, qui reste tourné vers la caméra.
#[derive(Component)]
pub struct Flame;

/// Taille du panneau de la flamme, en unités du monde.
const FLAME_SIZE: Vec2 = Vec2::new(13.0, 22.0);
/// De combien la torche avance devant le mur: la longueur du bras qui la porte.
const REACH: f32 = 5.0;

/// Pose les torches, fixées sur le flanc du mur: une plaque de fer plaquée contre la pierre, un bras qui avance, le manche de bois et sa
/// coupelle au bout, et la flamme dessus. `yaw` est l'angle de la vue.
pub fn spawn_torches(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    standard: &mut Assets<StandardMaterial>,
    flames: &mut Assets<FlameMaterial>,
    torches: &[Torch],
    yaw: f32,
) {
    if torches.is_empty() {
        return;
    }
    let iron = standard.add(StandardMaterial { base_color: Color::srgb(0.07, 0.06, 0.06), unlit: true, ..default() });
    let wood = standard.add(StandardMaterial { base_color: Color::srgb(0.20, 0.12, 0.06), unlit: true, ..default() });
    let panel = meshes.add(Rectangle::new(FLAME_SIZE.x, FLAME_SIZE.y));
    let stick = meshes.add(Cuboid::new(2.2, 9.0, 2.2));
    let cup = meshes.add(Cuboid::new(4.2, 2.4, 4.2));
    // Plaque, bras: leurs dimensions dépendent du sens du mur (la plaque est plaquée contre lui, le bras en sort).
    let plate_ns = meshes.add(Cuboid::new(5.0, 8.0, 1.0));
    let plate_ew = meshes.add(Cuboid::new(1.0, 8.0, 5.0));
    let arm_ns = meshes.add(Cuboid::new(2.0, 1.8, REACH));
    let arm_ew = meshes.add(Cuboid::new(REACH, 1.8, 2.0));
    for (i, torch) in torches.iter().enumerate() {
        // Le point du mur d'où part la torche, puis le bout du bras où se tient le manche.
        let wall = torch.at - torch.normal * 1.5;
        let tip = wall + torch.normal * REACH;
        let east_west = torch.normal.x.abs() > 0.5;
        let part = |commands: &mut Commands, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, at: Vec2, height: f32| {
            commands.spawn((
                TorchMark,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(to_3d(at, height)),
                RenderLayers::layer(LAYER),
            ));
        };
        let plate = if east_west { &plate_ew } else { &plate_ns };
        let arm = if east_west { &arm_ew } else { &arm_ns };
        part(commands, plate, &iron, wall + torch.normal * 0.5, TORCH_HEIGHT - 9.0);
        part(commands, arm, &iron, wall + torch.normal * (REACH / 2.0), TORCH_HEIGHT - 9.0);
        part(commands, &stick, &wood, tip, TORCH_HEIGHT - 4.5);
        part(commands, &cup, &iron, tip, TORCH_HEIGHT - 0.5);
        // La flamme, dans la coupelle: le panneau se tient devant la caméra, centré sur le bout du bras.
        commands.spawn((
            TorchMark,
            Flame,
            Mesh3d(panel.clone()),
            MeshMaterial3d(flames.add(FlameMaterial { params: Vec4::new(i as f32 * 1.37, 0.0, 0.0, 0.0) })),
            Transform::from_translation(to_3d(tip, TORCH_HEIGHT + FLAME_SIZE.y / 2.0 - 1.5)).with_rotation(Quat::from_rotation_y(yaw)),
            RenderLayers::layer(LAYER),
        ));
    }
}

/// Les flammes restent tournées vers la caméra quand on tourne la vue.
pub fn face_flames(mut flames: Query<&mut Transform, With<Flame>>) {
    let rotation = Quat::from_rotation_y(super::yaw());
    for mut transform in &mut flames {
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
}
