//! Effets du mode sanglant: gouttes qui giclent quand quelqu'un est blessé, éclaboussures qui restent au sol,
//! mare de sang quand un ennemi meurt.

use std::collections::VecDeque;

use bevy::prelude::*;
use castle_shared::*;

use crate::art::{Art, ArtTheme, Stain};

/// Nombre maximal de traces de sang gardées au sol (les plus anciennes disparaissent).
const MAX_STAINS: usize = 160;

pub struct GorePlugin;

impl Plugin for GorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stains>()
            .insert_resource(GoreRng(Rng::new(0xB10_0D)))
            .add_systems(Update, (track_health, blood_on_hit, move_drops).chain())
            .add_observer(blood_on_death);
    }
}

#[derive(Resource, Default)]
struct Stains(VecDeque<Entity>);

#[derive(Resource)]
struct GoreRng(Rng);

/// Points de vie vus à l'image précédente: une baisse déclenche une giclée.
#[derive(Component)]
struct LastHealth(f32);

/// Goutte de sang projetée: elle file dans une direction en ralentissant, puis disparaît.
#[derive(Component)]
struct BloodDrop {
    velocity: Vec2,
    life: Timer,
}

fn track_health(mut commands: Commands, new: Query<(Entity, &Health), Without<LastHealth>>) {
    for (entity, health) in &new {
        commands.entity(entity).insert(LastHealth(health.current));
    }
}

/// Pose une éclaboussure persistante au sol.
fn splat(
    commands: &mut Commands,
    art: &Art,
    stains: &mut Stains,
    rng: &mut Rng,
    at: Vec2,
    tiles: f32,
    alpha: f32,
) {
    let mut sprite = art.decals.sprite_sized(10 + rng.range(0, 2) as usize, Vec2::splat(tiles * TILE_SIZE));
    sprite.color = Color::srgba(0.4 + rng.next_f32() * 0.1, 0.02, 0.04, alpha);
    let entity = commands
        .spawn((
            Stain,
            sprite,
            Transform {
                translation: at.extend(0.03),
                rotation: Quat::from_rotation_z(rng.next_f32() * std::f32::consts::TAU),
                ..default()
            },
        ))
        .id();
    stains.0.push_back(entity);
    while stains.0.len() > MAX_STAINS {
        if let Some(old) = stains.0.pop_front() {
            commands.entity(old).despawn();
        }
    }
}

fn blood_on_hit(
    mut commands: Commands,
    theme: Res<ArtTheme>,
    art: Res<Art>,
    mut stains: ResMut<Stains>,
    mut rng: ResMut<GoreRng>,
    mut hurt: Query<(&Transform, &Health, &mut LastHealth)>,
) {
    for (transform, health, mut last) in &mut hurt {
        let lost = last.0 - health.current;
        last.0 = health.current;
        if *theme != ArtTheme::Gore || lost < 0.5 {
            continue;
        }
        let at = transform.translation.truncate();
        for _ in 0..(5 + lost as usize / 4).min(14) {
            let angle = rng.0.next_f32() * std::f32::consts::TAU;
            commands.spawn((
                Sprite::from_color(Color::srgb(0.55, 0.03, 0.05), Vec2::splat(1.5 + rng.0.next_f32() * 1.5)),
                Transform::from_translation((at + Vec2::new(0.0, 6.0)).extend(1.3)),
                BloodDrop {
                    velocity: Vec2::from_angle(angle) * (35.0 + rng.0.next_f32() * 60.0),
                    life: Timer::from_seconds(0.35 + rng.0.next_f32() * 0.2, TimerMode::Once),
                },
            ));
        }
        splat(&mut commands, &art, &mut stains, &mut rng.0, at + Vec2::new(0.0, -2.0), 1.1 + lost.min(30.0) / 30.0, 0.5);
    }
}

fn move_drops(mut commands: Commands, time: Res<Time>, mut drops: Query<(Entity, &mut BloodDrop, &mut Transform, &mut Sprite)>) {
    let dt = time.delta_secs();
    for (entity, mut drop, mut transform, mut sprite) in &mut drops {
        drop.life.tick(time.delta());
        transform.translation += (drop.velocity * dt).extend(0.0);
        drop.velocity *= 0.93;
        sprite.color = sprite.color.with_alpha(1.0 - drop.life.fraction());
        if drop.life.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Un ennemi ou un générateur qui disparaît laisse une grande mare de sang.
fn blood_on_death(
    removed: On<Remove, Monster>,
    mut commands: Commands,
    theme: Res<ArtTheme>,
    art: Res<Art>,
    mut stains: ResMut<Stains>,
    mut rng: ResMut<GoreRng>,
    positions: Query<&Transform>,
) {
    if *theme != ArtTheme::Gore {
        return;
    }
    if let Ok(transform) = positions.get(removed.entity) {
        splat(&mut commands, &art, &mut stains, &mut rng.0, transform.translation.truncate(), 3.2, 0.65);
    }
}
