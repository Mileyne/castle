//! Caméra qui suit le héros du joueur, à une échelle entière pour garder des pixels nets.

use bevy::{camera::ScalingMode, prelude::*, window::PrimaryWindow};
use castle_shared::{Layout, PartyId, Player, Room, ScreenShake};

use crate::{LocalParty, LocalPlayer};

/// Hauteur visée de la vue, en pixels d'art. L'échelle entière choisie donne au moins cette hauteur.
const TARGET_VIEW_HEIGHT: f32 = 240.0;
/// Vitesse à laquelle la caméra rattrape le héros.
const FOLLOW_SPEED: f32 = 8.0;
/// Zoom supplémentaire dans un passage étroit entre deux salles, et vitesse à laquelle on y entre et on en sort.
const CORRIDOR_ZOOM: f32 = 1.2;
const ZOOM_SPEED: f32 = 1.6;
/// Le cadrage d'une salle commence quand le héros est à cette distance de ses portes (en tuiles), et se fait en
/// `ROOM_BLEND_SECONDS`, à vitesse régulière puis adoucie aux deux bouts.
const ROOM_APPROACH_TILES: i32 = 3;
const ROOM_BLEND_SECONDS: f32 = 1.6;
/// Quand les quatre héros de l'équipe sont groupés, on se rapproche jusqu'à les cadrer, sans dépasser ce zoom en plus.
const TEAM_ZOOM_MAX: f32 = 2.2;
/// Zoom maximal quand la vue cadre la salle où l'on se trouve, et marge laissée autour d'elle (en unités du monde).
const ROOM_ZOOM_MAX: f32 = 2.4;
const ROOM_MARGIN: f32 = 12.0;
/// Marge ajoutée autour du groupe (en unités du monde), et taille minimale du cadre pour ne pas zoomer à l'extrême quand ils
/// sont empilés.
const TEAM_MARGIN: f32 = 34.0;
const TEAM_MIN_FRAME: Vec2 = Vec2::new(90.0, 56.0);

/// Cadrage de l'équipe: le zoom supplémentaire qui fait tenir tous ses héros dans la vue (entre 1 et `TEAM_ZOOM_MAX`; 1
/// s'ils sont trop dispersés pour que ce soit possible) et le point à regarder (le centre du groupe). `base_view` est la
/// taille de la vue sans zoom supplémentaire, en unités du monde.
pub fn team_framing(positions: &[Vec2], base_view: Vec2) -> (f32, Vec2) {
    if positions.is_empty() {
        return (1.0, Vec2::ZERO);
    }
    let (mut low, mut high) = (positions[0], positions[0]);
    for p in positions {
        low = low.min(*p);
        high = high.max(*p);
    }
    let center = (low + high) / 2.0;
    let frame = (high - low + Vec2::splat(2.0 * TEAM_MARGIN)).max(TEAM_MIN_FRAME);
    let fit = (base_view.x / frame.x).min(base_view.y / frame.y);
    (fit.clamp(1.0, TEAM_ZOOM_MAX), center)
}

/// Le zoom supplémentaire qui fait tenir toute la salle (`frame`: sa taille, murs compris) dans la vue, entre 1 et
/// `ROOM_ZOOM_MAX`; 1 si la salle est plus grande que la vue.
pub fn room_zoom(frame: Vec2, base_view: Vec2) -> f32 {
    let fit = (base_view.x / (frame.x + 2.0 * ROOM_MARGIN)).min(base_view.y / (frame.y + 2.0 * ROOM_MARGIN));
    fit.clamp(1.0, ROOM_ZOOM_MAX)
}

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
    mut team_zoom: Local<Option<f32>>,
    mut room_blend: Local<f32>,
    mut last_frame: Local<Option<(Vec2, Vec2)>>,
    mut base: Local<Option<Vec2>>,
    party: Option<Res<LocalParty>>,
    heroes: Query<(&Transform, &PartyId), (With<Player>, Without<Camera2d>)>,
    mut shake: ResMut<Shake>,
    window: Single<&Window, With<PrimaryWindow>>,
    hero: Option<Single<&Transform, With<LocalPlayer>>>,
    camera: Single<(&mut Transform, &mut Projection), (With<Camera2d>, Without<LocalPlayer>)>,
) {
    let (mut transform, mut projection) = camera.into_inner();

    // Salle où se trouve le héros dirigé (aucune dans un passage étroit).
    let hero_room = match (&hero, &room, &layout) {
        (Some(hero), Some(room), Some(layout)) => {
            layout.room_at(room, hero.translation.truncate()).map(|index| layout.room_frame(room, index))
        }
        _ => None,
    };
    // Zoom d'ambiance: on se rapproche quand le héros dirigé est dans un passage étroit, hors de toute salle.
    let in_corridor = matches!((&hero, &room, &layout), (Some(_), Some(_), Some(_))) && hero_room.is_none();
    let target = if in_corridor { CORRIDOR_ZOOM } else { 1.0 };
    let boost = corridor_zoom.get_or_insert(1.0);
    *boost += (target - *boost) * (1.0 - (-ZOOM_SPEED * time.delta_secs()).exp());
    let corridor_boost = *boost;

    // Zoom d'équipe: quand les quatre héros sont groupés, on se rapproche pour les cadrer tous.
    // L'échelle entière se compte en pixels physiques: sur un écran Retina (téléphone surtout), compter en points
    // logiques montrerait deux à trois fois trop de terrain, en sprites minuscules.
    let scale_factor = window.scale_factor();
    let base_zoom = (window.physical_height() as f32 / TARGET_VIEW_HEIGHT).floor().max(1.0) / scale_factor;
    let base_view = Vec2::new(window.width(), window.height()) / base_zoom;
    let team: Vec<Vec2> = match &party {
        Some(party) => heroes.iter().filter(|(_, id)| id.0 == party.0).map(|(t, _)| t.translation.truncate()).collect(),
        None => Vec::new(),
    };
    let (team_target, team_center) = if team.len() >= 2 { team_framing(&team, base_view) } else { (1.0, Vec2::ZERO) };
    let team_boost = team_zoom.get_or_insert(1.0);
    *team_boost += (team_target - *team_boost) * (1.0 - (-ZOOM_SPEED * time.delta_secs()).exp());
    let team_boost = *team_boost;

    // Cadrage de salle: un fondu unique (0 hors de la salle, 1 dedans) qui mène à la fois le zoom et le centrage, de façon
    // régulière et adoucie plutôt qu'un bond. Il démarre un peu avant la porte, et la salle quittée reste cadrée pendant
    // le fondu de sortie.
    let approaching = match (&hero, &room, &layout) {
        (Some(hero), Some(room), Some(layout)) => layout
            .room_near(room, hero.translation.truncate(), ROOM_APPROACH_TILES)
            .map(|index| layout.room_frame(room, index)),
        _ => None,
    };
    if approaching.is_some() {
        *last_frame = approaching;
    }
    let blend_target = if approaching.is_some() { 1.0 } else { 0.0 };
    let step = time.delta_secs() / ROOM_BLEND_SECONDS;
    *room_blend += (blend_target - *room_blend).clamp(-step, step);
    let blend = *room_blend * *room_blend * (3.0 - 2.0 * *room_blend);
    let ambient = corridor_boost.max(team_boost);
    let room_boost = last_frame.map_or(1.0, |(_, frame)| room_zoom(frame, base_view));
    // Interpolation géométrique: le zoom paraît régulier à l'œil.
    let extra = (ambient.ln() * (1.0 - blend) + room_boost.ln() * blend).exp();

    // Échelle: un pixel d'art occupe `zoom` pixels d'écran (entier au repos, plus fin pendant les zooms d'ambiance).
    let zoom = base_zoom * extra;
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
    // Plus l'équipe est cadrée de près, plus on regarde le centre du groupe (le héros dirigé en fait toujours partie).
    let grouped = ((team_boost - 1.0) / (TEAM_ZOOM_MAX - 1.0)).clamp(0.0, 1.0);
    let mut focus = hero.translation.truncate().lerp(team_center, grouped);
    // Dans une salle, la vue se centre sur la salle, qui tient tout entière à l'écran.
    if let Some((center, frame)) = *last_frame {
        if frame.x + 2.0 * ROOM_MARGIN <= base_view.x && frame.y + 2.0 * ROOM_MARGIN <= base_view.y {
            focus = focus.lerp(center, blend);
        }
    }
    let target = focus.clamp(-limit, limit);

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Une vue typique: 340 x 200 unités du monde.
    const VIEW: Vec2 = Vec2::new(340.0, 200.0);

    fn square(center: Vec2, half: f32) -> Vec<Vec2> {
        [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].iter().map(|(x, y)| center + Vec2::new(x * half, y * half)).collect()
    }

    #[test]
    fn a_tight_team_is_framed_closely_around_its_center() {
        let team = square(Vec2::new(500.0, -200.0), 20.0);
        let (boost, center) = team_framing(&team, VIEW);
        assert!(boost > 1.4, "un petit groupe doit être bien plus zoomé: {boost}");
        assert!(boost <= TEAM_ZOOM_MAX);
        assert_eq!(center, Vec2::new(500.0, -200.0));
    }

    #[test]
    fn a_small_room_is_zoomed_to_fill_the_view() {
        let zoom = room_zoom(Vec2::new(160.0, 128.0), VIEW);
        assert!(zoom > 1.0 && zoom <= ROOM_ZOOM_MAX, "{zoom}");
        // Toute la salle tient dans la vue zoomée.
        assert!(160.0 + 2.0 * ROOM_MARGIN <= VIEW.x / zoom + 0.01 && 128.0 + 2.0 * ROOM_MARGIN <= VIEW.y / zoom + 0.01);
    }

    #[test]
    fn a_room_bigger_than_the_view_gets_no_extra_zoom() {
        assert_eq!(room_zoom(Vec2::new(900.0, 700.0), VIEW), 1.0);
    }

    #[test]
    fn a_spread_out_team_gets_no_extra_zoom() {
        // Plus large que la vue: impossible de les cadrer.
        let team = [Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0), Vec2::new(0.0, 10.0), Vec2::new(10.0, 10.0)];
        assert_eq!(team_framing(&team, VIEW).0, 1.0);
    }

    #[test]
    fn closer_teams_zoom_more_and_stacked_heroes_do_not_zoom_without_limit() {
        let zoom = |half: f32| team_framing(&square(Vec2::ZERO, half), VIEW).0;
        assert!(zoom(10.0) >= zoom(40.0) && zoom(40.0) >= zoom(90.0) && zoom(90.0) >= zoom(160.0));
        assert!(zoom(40.0) > zoom(90.0), "le zoom grandit quand le groupe se resserre");
        // Tous au même point: le cadre minimal borne le zoom.
        let stacked = team_framing(&[Vec2::ZERO; 4], VIEW).0;
        assert!(stacked <= TEAM_ZOOM_MAX && stacked >= 1.0);
        // Un héros seul ou personne: rien de particulier.
        assert_eq!(team_framing(&[], VIEW).0, 1.0);
    }

    #[test]
    fn the_whole_team_fits_inside_the_zoomed_view() {
        for half in [10.0f32, 25.0, 50.0, 70.0, 100.0] {
            let team = square(Vec2::new(30.0, -40.0), half);
            let (boost, center) = team_framing(&team, VIEW);
            let view = VIEW / boost;
            for p in &team {
                let offset = (*p - center).abs();
                assert!(offset.x <= view.x / 2.0 + 1e-3 && offset.y <= view.y / 2.0 + 1e-3, "demi-côté {half}: héros hors de la vue");
            }
        }
    }
}
