//! Les lucioles du monde des elfes: de petits points lumineux verts qui flottent, dérivent et scintillent autour des héros, et
//! s'éteignent en s'éloignant d'eux (elles ne vivent qu'à proximité de l'équipe). Leur nombre est un réglage du niveau
//! (`LevelLight::fireflies`, 0 partout ailleurs).

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use castle_shared::{Player, Position, Room};

use super::look::ActiveLight;
use super::{LAYER, to_3d};

/// Distance à partir de laquelle une luciole s'éteint et va se poser près d'un héros, en unités du monde.
const RANGE: f32 = 150.0;
/// Distance des héros à laquelle une luciole brille de tout son éclat.
const NEAR: f32 = 70.0;

#[derive(Component)]
pub struct Firefly {
    anchor: Vec2,
    height: f32,
    phase: f32,
    speed: f32,
    drift: f32,
}

/// Le matériel et le maillage communs à toutes les lucioles.
#[derive(Resource)]
pub struct FireflyLook {
    mesh: Handle<Mesh>,
    core: Handle<StandardMaterial>,
    halo: Handle<StandardMaterial>,
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut standard: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(FireflyLook {
        mesh: meshes.add(Sphere::new(0.8).mesh().ico(1).unwrap()),
        core: standard.add(StandardMaterial { base_color: Color::srgb(0.9, 1.0, 0.45), unlit: true, ..default() }),
        halo: standard.add(StandardMaterial {
            base_color: Color::srgba(0.45, 1.0, 0.3, 0.2),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
    });
}

/// Un nombre pseudo-aléatoire dans [0, 1[ (xorshift).
fn next(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state >> 8) as f32 / (1u32 << 24) as f32
}

/// Un point libre (hors des murs) à une distance entre `min` et `max` de `center`.
fn spot_near(center: Vec2, min: f32, max: f32, room: &Room, state: &mut u32) -> Option<Vec2> {
    for _ in 0..6 {
        let angle = next(state) * std::f32::consts::TAU;
        let at = center + Vec2::from_angle(angle) * (min + next(state) * (max - min));
        if !room.overlaps_wall(at, 2.0) {
            return Some(at);
        }
    }
    None
}

pub fn run_fireflies(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLight>,
    look: Option<Res<FireflyLook>>,
    room: Option<Res<Room>>,
    heroes: Query<&Position, With<Player>>,
    mut flies: Query<(Entity, &mut Firefly, &mut Transform)>,
    mut seed: Local<u32>,
) {
    let (Some(look), Some(room)) = (look, room) else {
        return;
    };
    if *seed == 0 {
        *seed = 0x9E37_79B9;
    }
    let wanted = active.0.fireflies as usize;
    let places: Vec<Vec2> = heroes.iter().map(|p| p.0).collect();
    // Hors du monde des elfes, ou sans héros: plus de lucioles.
    if wanted == 0 || places.is_empty() {
        for (entity, ..) in &flies {
            commands.entity(entity).despawn();
        }
        return;
    }
    let t = time.elapsed_secs();
    let mut count = 0;
    for (entity, mut fly, mut transform) in &mut flies {
        count += 1;
        if count > wanted {
            commands.entity(entity).despawn();
            continue;
        }
        let here = fly.anchor + Vec2::new((t * fly.speed + fly.phase).sin(), (t * fly.speed * 0.8 + fly.phase * 1.7).cos()) * fly.drift;
        let nearest = places.iter().map(|p| p.distance(here)).fold(f32::MAX, f32::min);
        if nearest > RANGE {
            // Trop loin de l'équipe: elle va se poser près d'un héros.
            let hero = places[(next(&mut seed) * places.len() as f32) as usize % places.len()];
            if let Some(anchor) = spot_near(hero, 25.0, RANGE * 0.85, &room, &mut seed) {
                fly.anchor = anchor;
            }
            transform.scale = Vec3::ZERO;
            continue;
        }
        let height = fly.height + 3.0 * (t * 1.3 + fly.phase).sin();
        // Le scintillement: un pouls irrégulier, plus vif que doux; l'éclat décroît avec l'éloignement des héros.
        let pulse = (0.5 + 0.5 * (t * (3.0 + fly.phase.fract() * 3.0) + fly.phase).sin()).powf(3.0);
        let closeness = ((RANGE - nearest) / (RANGE - NEAR)).clamp(0.0, 1.0);
        let size = (0.25 + 0.75 * pulse) * closeness;
        transform.translation = to_3d(here, height);
        transform.scale = Vec3::splat(size);
    }
    // Les lucioles manquantes apparaissent, un peu à chaque image.
    if count < wanted {
        let hero = places[(next(&mut seed) * places.len() as f32) as usize % places.len()];
        if let Some(anchor) = spot_near(hero, 25.0, RANGE * 0.85, &room, &mut seed) {
            let phase = next(&mut seed) * 40.0;
            commands
                .spawn((
                    Firefly { anchor, height: 7.0 + next(&mut seed) * 20.0, phase, speed: 0.4 + next(&mut seed) * 0.6, drift: 8.0 + next(&mut seed) * 14.0 },
                    Mesh3d(look.mesh.clone()),
                    MeshMaterial3d(look.core.clone()),
                    Transform::from_scale(Vec3::ZERO).with_translation(to_3d(anchor, 12.0)),
                    Visibility::default(),
                    RenderLayers::layer(LAYER),
                ))
                .with_children(|fly| {
                    fly.spawn((Mesh3d(look.mesh.clone()), MeshMaterial3d(look.halo.clone()), Transform::from_scale(Vec3::splat(4.5)), RenderLayers::layer(LAYER)));
                });
        }
    }
}
