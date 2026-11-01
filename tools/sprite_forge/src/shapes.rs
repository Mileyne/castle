//! Briques de modélisation communes: couleurs, volumes, panneaux, mèches, et un petit générateur aléatoire.

use glam::{Mat3, Quat, Vec3};

use crate::render::{Part, Shape};

pub(crate) fn rgb(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 255) as f32 / 255.0, ((hex >> 8) & 255) as f32 / 255.0, (hex & 255) as f32 / 255.0]
}

pub(crate) fn align_y(dir: Vec3) -> Mat3 {
    Mat3::from_quat(Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::Y)))
}

pub(crate) fn mk(shape: Shape, center: Vec3, rot: Mat3, scale: Vec3, color: u32) -> Part {
    Part { shape, center, rot, scale, color: rgb(color), glow: false, shine: 0.0, layer: crate::render::Layer::Outfit }
}

pub(crate) fn ellipsoid(c: Vec3, radii: Vec3, color: u32) -> Part {
    mk(Shape::Sphere, c, Mat3::IDENTITY, radii, color)
}

pub(crate) fn ball(c: Vec3, r: f32, color: u32) -> Part {
    ellipsoid(c, Vec3::splat(r), color)
}

pub(crate) fn block(c: Vec3, half: Vec3, rot: Mat3, color: u32) -> Part {
    mk(Shape::Cube, c, rot, half, color)
}

/// Tronc de cône de `a` (rayon `r0`) à `b` (rayon `r1`): sert aux membres effilés, aux mèches, aux jupes.
pub(crate) fn cone(a: Vec3, b: Vec3, r0: f32, r1: f32, color: u32) -> Part {
    mk(Shape::Frustum(r0, r1), (a + b) / 2.0, align_y(b - a), Vec3::new(1.0, (b - a).length() / 2.0, 1.0), color)
}

/// Capsule entre deux points.
pub(crate) fn limb(a: Vec3, b: Vec3, r: f32, color: u32) -> Part {
    let len = (b - a).length();
    mk(Shape::Capsule(((len / 2.0 - r) / r).max(0.0)), (a + b) / 2.0, align_y(b - a), Vec3::splat(r), color)
}

/// Panneau plat de largeur `w` (le long de X) entre deux points: pan de cape, ruban, écharpe.
pub(crate) fn panel(a: Vec3, b: Vec3, w: f32, thick: f32, color: u32) -> Part {
    block((a + b) / 2.0, Vec3::new(w / 2.0, (b - a).length() / 2.0, thick / 2.0), align_y(b - a), color)
}

pub(crate) fn glow(mut p: Part) -> Part {
    p.glow = true;
    p
}

pub(crate) fn shiny(mut p: Part, amount: f32) -> Part {
    p.shine = amount;
    p
}

/// Direction artistique: le jeu n'en a qu'une, médiévale (héros réalistes inspirés des portraits fournis). Le type reste pour que
/// les fonctions de rendu disent de quelle direction elles parlent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Medieval,
}

impl Theme {
    pub const ALL: [Theme; 1] = [Theme::Medieval];

    /// Nom du dossier de sortie.
    pub fn folder(self) -> &'static str {
        match self {
            Theme::Medieval => "medieval",
        }
    }
}

// Couleurs communes.
pub(crate) const SKIN: u32 = 0xfad2b4;
pub(crate) const LEATHER_D: u32 = 0x4e2e18;
pub(crate) const GOLD: u32 = 0xf6c93a;
pub(crate) const WHITE: u32 = 0xf6f4ff;
pub(crate) const DARK: u32 = 0x1a1424;

/// Petit générateur pseudo-aléatoire déterministe, pour varier les tuiles et les décors.
pub(crate) struct Rng(pub u64);

impl Rng {
    pub(crate) fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }

    /// Valeur dans `[-a, a]`.
    pub(crate) fn spread(&mut self, a: f32) -> f32 {
        (self.next() * 2.0 - 1.0) * a
    }
}

/// Agrandit un modèle autour de l'origine (les pieds): positions et tailles de toutes les parties sont multipliées.
pub(crate) fn scaled(mut parts: Vec<Part>, k: f32) -> Vec<Part> {
    for part in &mut parts {
        part.center *= k;
        part.scale *= k;
    }
    parts
}

/// Met dans la couche `layer` toutes les pièces à partir de la `from`-ième.
pub(crate) fn mark(parts: &mut [Part], from: usize, layer: crate::render::Layer) {
    for part in &mut parts[from..] {
        part.layer = layer;
    }
}
