//! Vue à la première personne (F4): la caméra se place à hauteur des yeux du héros dirigé et regarde dans la direction de la vue. La souris
//! tourne la vue (et l'incline vers le haut ou le bas); les touches de direction vont déjà dans le sens de la vue (avant, arrière, côtés),
//! comme dans les autres modes. Le héros dirigé est caché; les autres héros, les monstres et le décor restent visibles.

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use castle_shared::{PartyId, Player};

use crate::LocalPlayer;
use crate::pause::PauseMenu;
use crate::title::TitleActive;

/// Hauteur des yeux au-dessus du sol, en unités du monde (un héros mesure environ 24 unités, une tuile 16).
pub const EYE_HEIGHT: f32 = 21.0;
/// Champ de vision vertical, en degrés.
pub const FIELD_OF_VIEW: f32 = 78.0;
/// Degrés de rotation de la vue par pixel de déplacement de la souris, pour l'angle (sur le côté) et l'inclinaison (haut et bas).
const MOUSE_TURN: f32 = 0.12;
const MOUSE_TILT: f32 = 0.10;
/// Les compagnons plus près que cela de la caméra sont cachés, en unités du monde.
const HIDE_DISTANCE: f32 = 40.0;
/// Limites de l'inclinaison de la vue, en degrés (négatif: vers le sol).
const PITCH_RANGE: (f32, f32) = (-55.0, 40.0);

/// Les murs sont plus hauts à la première personne: à hauteur des yeux, ceux de la vue de dessus paraissent bas.
pub const WALL_FACTOR: f32 = 2.0;

/// À la première personne, un héros s'arrête un peu plus loin des monstres, générateurs et autres corps: la caméra est au centre du héros,
/// et le dessin d'un monstre est plus large que sa zone de collision (sans marge, on se retrouverait dans son corps). Reste en deçà de la
/// portée de frappe des monstres (demi-taille + 6 + 6), pour qu'ils puissent encore toucher le héros.
pub const BODY_MARGIN: f32 = 5.0;

/// La marge ajoutée à la zone de collision des corps: `BODY_MARGIN` à la première personne, rien ailleurs.
pub fn body_margin() -> f32 {
    if ACTIVE.load(Ordering::Relaxed) { BODY_MARGIN } else { 0.0 }
}

/// Vrai quand la première personne est active (lu par le décor, qui n'a pas la ressource sous la main).
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Le facteur de hauteur des murs: 1 de dessus, `WALL_FACTOR` à la première personne.
pub fn wall_factor() -> f32 {
    if ACTIVE.load(Ordering::Relaxed) { WALL_FACTOR } else { 1.0 }
}

/// La vue à la première personne est-elle active, et de combien le regard est incliné (en degrés, négatif vers le sol).
#[derive(Resource, Default)]
pub struct FirstPerson {
    pub on: bool,
    pub pitch: f32,
}

pub struct FirstPersonPlugin;

impl Plugin for FirstPersonPlugin {
    fn build(&self, app: &mut App) {
        // `CASTLE_FIRST_PERSON=1` démarre déjà dans cette vue (pour les essais).
        // Sa valeur, si c'est un nombre, est l'inclinaison de départ du regard en degrés (positive: vers le haut).
        let start = std::env::var("CASTLE_FIRST_PERSON").ok();
        let on = start.is_some();
        let pitch = start.and_then(|v| v.parse::<f32>().ok()).unwrap_or(-8.0).clamp(PITCH_RANGE.0, PITCH_RANGE.1);
        ACTIVE.store(on, Ordering::Relaxed);
        app.insert_resource(FirstPerson { on, pitch }).add_systems(Update, (toggle, look, hide_own_model, grab_cursor).chain());
    }
}

/// F4 passe de la vue de dessus à la vue à la première personne et inversement.
fn toggle(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<FirstPerson>) {
    if keys.just_pressed(KeyCode::F4) {
        view.on = !view.on;
        ACTIVE.store(view.on, Ordering::Relaxed);
    }
}

/// La souris tourne la vue, tant que le jeu est en cours (ni menu de pause, ni écran titre).
fn look(motion: Res<AccumulatedMouseMotion>, mut view: ResMut<FirstPerson>, pause: Res<PauseMenu>, title: Res<TitleActive>) {
    if !view.on || pause.open || title.0 || motion.delta == Vec2::ZERO {
        return;
    }
    // La souris à droite tourne la vue vers la droite (l'angle de la vue diminue); vers le haut, le regard monte.
    crate::settings::turn_view(-motion.delta.x * MOUSE_TURN, 0.0);
    view.pitch = (view.pitch - motion.delta.y * MOUSE_TILT).clamp(PITCH_RANGE.0, PITCH_RANGE.1);
}

/// Dans cette vue, le héros dirigé est caché (on est dans sa tête), ainsi que ses compagnons qui sont si près de la caméra que leur corps
/// remplirait l'écran. Ils reviennent en sortant, ou en s'éloignant.
fn hide_own_model(
    view: Res<FirstPerson>,
    title: Res<TitleActive>,
    party: Option<Res<crate::LocalParty>>,
    local: Query<&Transform, With<LocalPlayer>>,
    mut heroes: Query<(&Transform, &PartyId, Has<LocalPlayer>, &mut Visibility), With<Player>>,
) {
    let active = view.on && !title.0;
    let eye = local.single().ok().map(|t| t.translation.truncate());
    for (transform, id, is_local, mut visibility) in &mut heroes {
        let close = eye.is_some_and(|eye| party.as_ref().is_some_and(|p| p.0 == id.0) && transform.translation.truncate().distance(eye) < HIDE_DISTANCE);
        let wanted = if active && (is_local || close) { Visibility::Hidden } else { Visibility::Inherited };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// La souris est captée pendant la partie dans cette vue, et rendue dans les menus et en sortant.
fn grab_cursor(view: Res<FirstPerson>, pause: Res<PauseMenu>, title: Res<TitleActive>, mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    let captured = view.on && !pause.open && !title.0;
    let (grab_mode, visible) = if captured { (CursorGrabMode::Locked, false) } else { (CursorGrabMode::None, true) };
    if cursor.grab_mode != grab_mode {
        cursor.grab_mode = grab_mode;
    }
    if cursor.visible != visible {
        cursor.visible = visible;
    }
}
