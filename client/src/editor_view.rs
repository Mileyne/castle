//! Ce que le jeu sait de l'éditeur de niveaux (voir `editor`): son existence (sans musique, sans interface, sans météo, sans prologue)
//! et la caméra libre qu'il pilote.

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;

static EDITOR: AtomicBool = AtomicBool::new(false);

/// Le jeu est celui de l'éditeur de niveaux.
pub fn set_editor() {
    EDITOR.store(true, Ordering::Relaxed);
}

pub fn active() -> bool {
    EDITOR.load(Ordering::Relaxed)
}

/// La caméra libre de l'éditeur: le point regardé (en tuiles), le cap (0: la caméra est au sud), l'inclinaison au-dessus de l'horizon
/// (radians) et la distance (en tuiles).
#[derive(Resource, Clone, Copy)]
pub struct EditorCamera {
    pub target: Vec2,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self { target: Vec2::new(32.0, 20.0), yaw: 0.0, pitch: 0.95, dist: 36.0 }
    }
}

/// Champ de vision vertical de la caméra de l'éditeur, en degrés.
pub const FIELD_OF_VIEW: f32 = 45.0;
