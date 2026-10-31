//! Schéma de commandes unique, et mode de saisie courant (clavier, manette ou toucher).
//!
//! | Action | Clavier | Manette | Toucher |
//! |---|---|---|---|
//! | Se déplacer | ZQSD / flèches | stick gauche | joystick |
//! | Frapper | Espace | B | Frapper |
//! | Pouvoir spécial | Espace maintenu | B maintenu | Frapper maintenu |
//! | Agir | Maj | X | Agir |
//! | Changer de héros | Ctrl | Y | Héros |
//! | Valider / continuer | Entrée | A | OK |
//!
//! Les textes d'aide et les dialogues ne nomment jamais une touche: ils contiennent des marques (`%attack%`, `%action%`,
//! `%switch%`...) que `i18n.rs` remplace par le nom de la commande du mode de saisie en cours.

use bevy::{
    input::keyboard::{KeyCode, KeyboardInput},
    prelude::*,
};

use crate::settings;

/// Combien de temps il faut maintenir la touche de tir pour lancer le pouvoir spécial, en secondes.
pub const SPECIAL_HOLD: f32 = 0.45;

/// Comment le joueur joue en ce moment: le dernier appareil qu'il a utilisé.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputMode {
    Keyboard,
    Gamepad,
    Touch,
}

impl InputMode {
    /// Suffixe des clés de traduction propres à ce mode (`ctl-attack-gamepad`, `menu-hint-touch`...).
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Gamepad => "gamepad",
            Self::Touch => "touch",
        }
    }
}

impl Default for InputMode {
    /// Au départ: manette sur Apple TV, toucher si les commandes tactiles sont affichées, sinon clavier.
    fn default() -> Self {
        if settings::is_tv() {
            Self::Gamepad
        } else if settings::touch_ui() {
            Self::Touch
        } else {
            Self::Keyboard
        }
    }
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputMode>().add_systems(PreUpdate, detect_input_mode);
    }
}

/// Le mode suit le dernier appareil utilisé. Les touches simulées par la manette ou les boutons à l'écran ne comptent pas:
/// on regarde les vrais événements.
fn detect_input_mode(
    mut mode: ResMut<InputMode>,
    mut typed: MessageReader<KeyboardInput>,
    gamepads: Query<&Gamepad>,
    touches: Res<Touches>,
) {
    let mut seen = None;
    if typed.read().any(|e| e.state.is_pressed() && !matches!(e.key_code, KeyCode::Unidentified(_))) {
        seen = Some(InputMode::Keyboard);
    }
    if touches.iter_just_pressed().next().is_some() && settings::touch_ui() {
        seen = Some(InputMode::Touch);
    }
    for gamepad in &gamepads {
        let stick = gamepad.left_stick().length() > 0.6;
        if stick || gamepad.get_just_pressed().next().is_some() {
            seen = Some(InputMode::Gamepad);
        }
    }
    if let Some(seen) = seen {
        mode.set_if_neq(seen);
    }
}
