//! Colorimétrie du décor: les tuiles prennent la teinte du héros dirigé, qui glisse doucement quand on change de héros.
//!
//! (Une couche sombre éclairée autour des héros et des sources lumineuses existait ici: dessinée derrière le décor, elle ne
//! changeait rien à l'image et coûtait une passe de rendu et une mise à jour par image, elle a été retirée.)

use bevy::prelude::*;
use castle_shared::*;

use crate::LocalPlayer;

/// Vitesse à laquelle la colorimétrie glisse vers celle d'un nouveau héros.
const GRADE_SPEED: f32 = 4.0;
/// Part de la teinte du héros qui passe dans le décor.
const ENVIRONMENT_MIX: f32 = 0.6;

/// Multiplicateur de couleur du décor appliqué par un héros: gris pour le Chevalier, violet pour l'Ensorceleur, vert pour le
/// Rôdeur, orange pour le Forgebraise. Les personnages gardent leurs couleurs.
fn tiles_grade_of(class: HeroClass) -> Vec3 {
    match class {
        HeroClass::Knight => Vec3::new(0.8, 0.82, 0.86),
        HeroClass::Sorcerer => Vec3::new(0.84, 0.55, 1.0),
        HeroClass::Ranger => Vec3::new(0.55, 1.0, 0.6),
        HeroClass::Forgebraise => Vec3::new(1.0, 0.64, 0.34),
    }
}

/// Multiplicateur de couleur du décor selon le héros dirigé, lissé; les tuiles le lisent pour se teinter.
#[derive(Resource)]
pub struct Grade(pub Vec3);

impl Default for Grade {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Grade>().add_systems(Update, update_grade);
    }
}

fn update_grade(
    time: Res<Time>,
    active: Option<Single<&HeroClass, With<LocalPlayer>>>,
    mut smooth: Local<Option<Vec3>>,
    mut environment: ResMut<Grade>,
) {
    let target = active.map_or(Vec3::ONE, |class| tiles_grade_of(**class));
    let k = 1.0 - (-GRADE_SPEED * time.delta_secs()).exp();
    let tiles = smooth.get_or_insert(target);
    *tiles = tiles.lerp(target, k);
    let next = Vec3::ONE.lerp(*tiles, ENVIRONMENT_MIX);
    // Pas d'écriture (donc pas de détection de changement) tant que la teinte ne bouge plus.
    if environment.0.distance_squared(next) > 1e-8 {
        environment.0 = next;
    }
}
