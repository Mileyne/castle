//! Caméra qui suit le héros du joueur, à une échelle entière pour garder des pixels nets.

use bevy::{camera::ScalingMode, prelude::*, window::PrimaryWindow};
use castle_shared::{Layout, Room, ScreenShake};

use crate::LocalPlayer;

/// Hauteur visée de la vue, en pixels d'art. L'échelle entière choisie donne au moins cette hauteur.
const TARGET_VIEW_HEIGHT: f32 = 200.0;
/// Vitesse à laquelle la caméra rattrape le héros.
const FOLLOW_SPEED: f32 = 8.0;
/// Zoom supplémentaire dans un passage étroit entre deux salles, et vitesse à laquelle on y entre et on en sort.
const CORRIDOR_ZOOM: f32 = 1.4;
const ZOOM_SPEED: f32 = 4.0;

/// Tremblement d'écran: la « traumatisme » monte avec chaque secousse demandée par le serveur puis retombe;
/// l'amplitude du tremblement grandit avec son carré, donc les petites secousses restent très légères.
#[derive(Resource, Default)]
struct Shake {
    trauma: f32,
}

const SHAKE_DECAY: f32 = 0.85;
/// Amplitude maximale du décalage, en unités du monde.
const SHAKE_AMPLITUDE: f32 = 5.0;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shake>()
            .add_observer(|shake: On<ScreenShake>, mut state: ResMut<Shake>| {
                state.trauma = (state.trauma + shake.0).min(1.0);
            })
            .add_systems(Startup, setup_camera)
            .add_systems(Update, follow_hero.after(crate::smooth_positions));
    }
}

fn setup_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::lighting::LightingMaterial>>,
) {
    let overlay = crate::lighting::overlay(&mut meshes, &mut materials);
    commands
        .spawn((
            Camera2d,
            Projection::from(OrthographicProjection {
                scaling_mode: ScalingMode::WindowSize,
                ..OrthographicProjection::default_2d()
            }),
        ))
        .with_children(|camera| {
            camera.spawn(overlay);
        });
}

fn follow_hero(
    time: Res<Time>,
    room: Option<Res<Room>>,
    layout: Option<Res<Layout>>,
    mut corridor_zoom: Local<Option<f32>>,
    mut base: Local<Option<Vec2>>,
    mut shake: ResMut<Shake>,
    window: Single<&Window, With<PrimaryWindow>>,
    hero: Option<Single<&Transform, With<LocalPlayer>>>,
    camera: Single<(&mut Transform, &mut Projection), (With<Camera2d>, Without<LocalPlayer>)>,
) {
    let (mut transform, mut projection) = camera.into_inner();

    // Zoom d'ambiance: on se rapproche quand le héros dirigé est dans un passage étroit, hors de toute salle.
    let in_corridor = match (&hero, &room, &layout) {
        (Some(hero), Some(room), Some(layout)) => {
            let tile = room.world_to_tile(hero.translation.truncate());
            !layout.rooms.iter().any(|r| {
                (r.min.x as i32..=r.max.x as i32).contains(&tile.x) && (r.min.y as i32..=r.max.y as i32).contains(&tile.y)
            })
        }
        _ => false,
    };
    let target = if in_corridor { CORRIDOR_ZOOM } else { 1.0 };
    let boost = corridor_zoom.get_or_insert(1.0);
    *boost += (target - *boost) * (1.0 - (-ZOOM_SPEED * time.delta_secs()).exp());

    // Échelle entière: un pixel d'art occupe `zoom` pixels d'écran.
    let zoom = (window.height() / TARGET_VIEW_HEIGHT).floor().max(1.0) * *boost;
    if let Projection::Orthographic(ortho) = &mut *projection {
        let scale = 1.0 / zoom;
        if ortho.scale != scale {
            ortho.scale = scale;
        }
    }

    let (Some(hero), Some(room)) = (hero, room) else {
        return;
    };
    // Demi-vue en unités du monde; la caméra ne montre jamais au-delà de la salle.
    let half_view = Vec2::new(window.width(), window.height()) / (2.0 * zoom);
    let limit = ((room.size().as_vec2() * castle_shared::TILE_SIZE) / 2.0 - half_view).max(Vec2::ZERO);
    let target = hero.translation.truncate().clamp(-limit, limit);

    // La position « propre » de la caméra est mémorisée à part: la secousse s'y ajoute sans la déformer.
    let t = 1.0 - (-FOLLOW_SPEED * time.delta_secs()).exp();
    let current = base.unwrap_or(transform.translation.truncate());
    let next = current.lerp(target, t);
    *base = Some(next);

    shake.trauma = (shake.trauma - SHAKE_DECAY * time.delta_secs()).max(0.0);
    let intensity = shake.trauma * shake.trauma * SHAKE_AMPLITUDE;
    let at = time.elapsed_secs();
    // Deux oscillations rapides et incommensurables donnent un tremblement irrégulier.
    let offset = Vec2::new((at * 61.0).sin() + 0.5 * (at * 97.0).sin(), (at * 53.0).cos() + 0.5 * (at * 89.0).cos()) * intensity;
    transform.translation.x = next.x + offset.x;
    transform.translation.y = next.y + offset.y;
}
