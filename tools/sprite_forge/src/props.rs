//! Les objets du château, rendus comme les personnages. Chacun a vingt images (impulsion lumineuse,
//! rotation, balancement) pour que le décor vive un peu. Dimensions en mètres du modèle.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Mat3, Vec3};

use crate::render::Part;
use crate::shapes::*;

/// 20 images pour un cycle de 0,8 s, lu à 25 images par seconde.
pub const FRAMES: usize = 20;

#[derive(Clone, Copy, Debug)]
pub enum Prop {
    Generator,
    Turret,
    Coin,
    Food,
    Key,
    Lever,
    Rune,
    Mechanism,
    TrapArmed,
    TrapOff,
    DoorLocked,
    DoorRoyal,
    DoorRoyalEmblem,
    Portal,
    Potion,
    /// Panneaux de l'énigme de la porte royale, dans le mur (vus de face): violet pour l'Ensorceleur...
    WallRune,
    /// ... orange pour le Forgebraise (mécanisme grippé, puis réparé)...
    WallGearsJammed,
    WallGearsFixed,
    /// ... vert pour le Rôdeur (support vide, cible garnie, cible touchée par la flèche).
    WallTargetEmpty,
    WallTargetMounted,
    WallTargetHit,
    /// Herse qui barre le couloir devant la porte royale (une moitié; il en faut deux côte à côte).
    Portcullis,
    /// Pièces à trouver dans le château: ingrédient (violet), pièce de rechange (orange), disque de cible (vert).
    Ingredient,
    GearPart,
    TargetDisc,
}

impl Prop {
    /// Les panneaux de mur sont vus de face et centrés dans leur case; tous les autres objets sont posés au sol.
    pub fn is_wall_mounted(self) -> bool {
        matches!(
            self,
            Prop::WallRune | Prop::WallGearsJammed | Prop::WallGearsFixed | Prop::WallTargetEmpty | Prop::WallTargetMounted | Prop::WallTargetHit
        )
    }
}

/// Ordre des lignes dans la planche (le client utilise les mêmes indices).
pub const PROPS: [Prop; 25] = [
    Prop::Generator,
    Prop::Turret,
    Prop::Coin,
    Prop::Food,
    Prop::Key,
    Prop::Lever,
    Prop::Rune,
    Prop::Mechanism,
    Prop::TrapArmed,
    Prop::TrapOff,
    Prop::DoorLocked,
    Prop::DoorRoyal,
    Prop::DoorRoyalEmblem,
    Prop::Portal,
    Prop::Potion,
    Prop::WallRune,
    Prop::WallGearsJammed,
    Prop::WallGearsFixed,
    Prop::WallTargetEmpty,
    Prop::WallTargetMounted,
    Prop::WallTargetHit,
    Prop::Portcullis,
    Prop::Ingredient,
    Prop::GearPart,
    Prop::TargetDisc,
];

/// Couleurs des classes, reprises par tout ce que la classe doit débloquer: on voit tout de suite à qui s'adresse un panneau.
const KNIGHT_C: u32 = 0x4682b4;
const SORCERER_C: u32 = 0xba55d3;
const RANGER_C: u32 = 0x32cd50;
const FORGE_C: u32 = 0xff6a10;

const STONE: u32 = 0x5a5068;
const STONE_L: u32 = 0x7a708a;
const MAGENTA: u32 = 0xe040d0;
const BRASS: u32 = 0xc09040;

/// Disque ou cylindre dont l'axe est Z (face tournée vers le spectateur).
fn disc_z(center: Vec3, radius: f32, thick: f32, color: u32) -> Part {
    mk_disc(center, radius, thick, Mat3::from_rotation_x(FRAC_PI_2), color)
}

fn mk_disc(center: Vec3, radius: f32, thick: f32, rot: Mat3, color: u32) -> Part {
    crate::shapes::mk(
        crate::render::Shape::Frustum(radius, radius),
        center,
        rot,
        Vec3::new(1.0, thick / 2.0, 1.0),
        color,
    )
}

fn generator(t: f32) -> Vec<Part> {
    let pulse = 1.0 + 0.07 * (t * TAU).sin();
    let bob = 0.05 * (t * TAU).sin();
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.2, 0.0), 1.05, 0.9, STONE), 0.3),
        shiny(cone(Vec3::new(0.0, 0.2, 0.0), Vec3::new(0.0, 0.42, 0.0), 0.8, 0.62, STONE_L), 0.3),
        glow(cone(Vec3::new(0.0, 0.42, 0.0), Vec3::new(0.0, 0.45, 0.0), 0.6, 0.56, MAGENTA)),
    ];
    // Pointes d'os en couronne.
    for k in 0..7 {
        let a = k as f32 * TAU / 7.0;
        let (c, s) = (a.cos(), a.sin());
        p.push(shiny(cone(Vec3::new(c * 0.78, 0.2, s * 0.78), Vec3::new(c * 0.9, 0.85, s * 0.9), 0.07, 0.01, 0xe9e1cb), 0.5));
    }
    // Cristal flottant en deux pyramides, noyau blanc et étincelles en orbite.
    let y = 1.0 + bob;
    p.push(glow(cone(Vec3::new(0.0, y - 0.38 * pulse, 0.0), Vec3::new(0.0, y, 0.0), 0.02, 0.3 * pulse, MAGENTA)));
    p.push(glow(cone(Vec3::new(0.0, y, 0.0), Vec3::new(0.0, y + 0.4 * pulse, 0.0), 0.3 * pulse, 0.02, 0xff80ee)));
    p.push(glow(ball(Vec3::new(0.0, y, 0.0), 0.13 * pulse, 0xffffff)));
    for k in 0..3 {
        let a = t * TAU + k as f32 * 2.1;
        p.push(glow(ball(Vec3::new(a.cos() * 0.55, y + 0.1 * (a * 2.0).sin(), a.sin() * 0.55), 0.04, 0xffb0f0)));
    }
    p
}

fn turret(t: f32) -> Vec<Part> {
    let recoil = 0.04 * (t * TAU).sin().max(0.0);
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.14, 0.0), 0.62, 0.55, BRASS), 0.6),
        shiny(cone(Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.0, 0.2, 0.0), 0.5, 0.5, 0x6a4a20), 0.3),
        shiny(ellipsoid(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.4, 0.34, 0.4), 0x8a94a8), 0.7),
        shiny(cone(Vec3::new(0.0, 0.46, 0.0), Vec3::new(0.0, 0.54, 0.0), 0.43, 0.43, BRASS), 0.6),
        shiny(ellipsoid(Vec3::new(0.0, 0.82, 0.0), Vec3::new(0.3, 0.22, 0.3), BRASS), 0.8),
        // Canon incliné vers l'avant à droite, avec sa bouche incandescente.
        shiny(limb(Vec3::new(0.05, 0.78, 0.15), Vec3::new(0.42 - recoil, 0.9, 0.78 - recoil), 0.11, 0x4a5060), 0.7),
        shiny(limb(Vec3::new(0.38 - recoil, 0.88, 0.72 - recoil), Vec3::new(0.44 - recoil, 0.9, 0.84 - recoil), 0.14, BRASS), 0.7),
        glow(ball(Vec3::new(0.45 - recoil, 0.9, 0.86 - recoil), 0.07, 0xff9a30)),
        // Cadran lumineux et tuyaux.
        glow(ball(Vec3::new(-0.22, 0.55, 0.34), 0.07, 0xffd040)),
        shiny(limb(Vec3::new(-0.38, 0.2, 0.0), Vec3::new(-0.3, 0.75, -0.1), 0.045, BRASS), 0.6),
        shiny(limb(Vec3::new(0.38, 0.2, -0.1), Vec3::new(0.3, 0.7, -0.2), 0.045, BRASS), 0.6),
    ];
    // Engrenage visible sur le flanc.
    let g = Vec3::new(-0.34, 0.45, 0.2);
    p.push(shiny(disc_z(g, 0.17, 0.06, 0x9aa0b0), 0.6));
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0 + t * TAU / 8.0;
        p.push(block(g + Vec3::new(a.cos() * 0.19, a.sin() * 0.19, 0.0), Vec3::new(0.035, 0.035, 0.03), Mat3::from_rotation_z(a), 0x9aa0b0));
    }
    p
}

fn coin(t: f32) -> Vec<Part> {
    let spin = Mat3::from_rotation_y(t * TAU / 2.0) * Mat3::from_rotation_x(FRAC_PI_2);
    let c = Vec3::new(0.0, 0.32 + 0.02 * (t * TAU).sin(), 0.0);
    vec![
        shiny(mk_disc(c, 0.3, 0.07, spin, 0xd9a520), 0.9),
        shiny(mk_disc(c + spin * Vec3::Y * 0.04, 0.22, 0.04, spin, 0xffd84a), 0.9),
        glow(mk_disc(c + spin * Vec3::Y * 0.065, 0.07, 0.02, spin, 0xfff0a0)),
    ]
}

fn food(t: f32) -> Vec<Part> {
    let bob = 0.02 * (t * TAU).sin();
    let meat = Vec3::new(0.0, 0.34 + bob, 0.0);
    vec![
        shiny(ellipsoid(meat, Vec3::new(0.3, 0.24, 0.24), 0xb8502a), 0.9),
        shiny(ellipsoid(meat + Vec3::new(-0.08, 0.1, 0.1), Vec3::new(0.16, 0.1, 0.1), 0xd8703a), 0.9),
        limb(meat + Vec3::new(0.2, -0.04, 0.0), meat + Vec3::new(0.55, -0.16, 0.0), 0.055, 0xf4f0e0),
        ball(meat + Vec3::new(0.58, -0.12, 0.05), 0.075, 0xf4f0e0),
        ball(meat + Vec3::new(0.58, -0.2, -0.05), 0.075, 0xf4f0e0),
        glow(ball(meat + Vec3::new(-0.15, 0.2, 0.12), 0.035, 0xffffff)),
    ]
}

/// Flacon de verre rond rempli d'un liquide vert, bouchon de liège, bulles lumineuses qui montent et croix claire.
fn potion(t: f32) -> Vec<Part> {
    let c = Vec3::new(0.0, 0.34 + 0.02 * (t * TAU).sin(), 0.0);
    let mut p = vec![
        shiny(ball(c, 0.27, 0x2fcf5a), 0.9),
        shiny(ellipsoid(c + Vec3::new(0.0, -0.08, 0.02), Vec3::new(0.25, 0.15, 0.25), 0x1f9a42), 0.7),
        shiny(cone(c + Vec3::new(0.0, 0.2, 0.0), c + Vec3::new(0.0, 0.46, 0.0), 0.11, 0.085, 0xbfe8e0), 0.9),
        shiny(cone(c + Vec3::new(0.0, 0.44, 0.0), c + Vec3::new(0.0, 0.5, 0.0), 0.11, 0.11, 0xe6f6f2), 0.9),
        shiny(cone(c + Vec3::new(0.0, 0.5, 0.0), c + Vec3::new(0.0, 0.6, 0.0), 0.085, 0.075, 0x8a5a30), 0.2),
        glow(ball(c + Vec3::new(-0.12, 0.12, 0.2), 0.045, 0xffffff)),
        // Croix claire sur le ventre du flacon.
        glow(block(c + Vec3::new(0.0, -0.02, 0.26), Vec3::new(0.12, 0.035, 0.012), Mat3::IDENTITY, 0xe8ffe8)),
        glow(block(c + Vec3::new(0.0, -0.02, 0.26), Vec3::new(0.035, 0.12, 0.012), Mat3::IDENTITY, 0xe8ffe8)),
    ];
    for k in 0..3 {
        let phase = (t + k as f32 / 3.0).fract();
        p.push(glow(ball(c + Vec3::new((k as f32 - 1.0) * 0.1, -0.15 + phase * 0.5, 0.1), 0.03 * (1.0 - 0.4 * phase), 0x9affb0)));
    }
    p
}

/// Dalle de pierre encastrée dans le mur (vue de face, centrée), avec un cadre et des rivets.
fn wall_slab(frame: u32) -> Vec<Part> {
    let mut p = vec![
        shiny(block(Vec3::ZERO, Vec3::new(0.55, 0.55, 0.05), Mat3::IDENTITY, STONE), 0.3),
        shiny(block(Vec3::new(0.0, 0.53, 0.06), Vec3::new(0.57, 0.04, 0.05), Mat3::IDENTITY, frame), 0.6),
        shiny(block(Vec3::new(0.0, -0.53, 0.06), Vec3::new(0.57, 0.04, 0.05), Mat3::IDENTITY, frame), 0.6),
        shiny(block(Vec3::new(0.53, 0.0, 0.06), Vec3::new(0.04, 0.57, 0.05), Mat3::IDENTITY, frame), 0.6),
        shiny(block(Vec3::new(-0.53, 0.0, 0.06), Vec3::new(0.04, 0.57, 0.05), Mat3::IDENTITY, frame), 0.6),
    ];
    for (x, y) in [(-0.46, 0.46), (0.46, 0.46), (-0.46, -0.46), (0.46, -0.46)] {
        p.push(shiny(ball(Vec3::new(x, y, 0.1), 0.03, 0xb0b4c0), 0.8));
    }
    p
}

/// Disque dont l'axe est Z, légèrement avancé de `lift` pour que les anneaux concentriques ne se confondent pas.
fn ring(radius: f32, lift: f32, color: u32) -> Part {
    disc_z(Vec3::new(0.0, 0.0, 0.07 + lift), radius, 0.02, color)
}

/// Panneau violet de l'Ensorceleur: un cercle de runes qui palpite et trois logements vides pour les ingrédients.
fn wall_rune(t: f32) -> Vec<Part> {
    let pulse = 0.75 + 0.25 * (t * TAU).sin();
    let mut p = wall_slab(SORCERER_C);
    p.push(shiny(ring(0.4, 0.0, 0x20123a), 0.4));
    p.push(shiny(ring(0.36, 0.01, 0x3a1c60), 0.4));
    for k in 0..12 {
        let a = k as f32 * TAU / 12.0;
        let lit = if (k + (t * 12.0) as usize) % 4 == 0 { 1.0 } else { 0.55 };
        p.push(glow(block(
            Vec3::new(a.cos() * 0.31, 0.12 + a.sin() * 0.31, 0.11),
            Vec3::new(0.05 * lit, 0.018, 0.012),
            Mat3::from_rotation_z(a + 1.5708),
            SORCERER_C,
        )));
    }
    p.push(glow(block(Vec3::new(0.0, 0.12, 0.12), Vec3::new(0.1 * pulse, 0.1 * pulse, 0.012), Mat3::from_rotation_z(0.785), 0xe0a0ff)));
    p.push(glow(block(Vec3::new(0.0, 0.12, 0.125), Vec3::new(0.035, 0.035, 0.012), Mat3::from_rotation_z(0.785), 0xffffff)));
    // Trois logements pour les ingrédients, en bas.
    for x in [-0.25f32, 0.0, 0.25] {
        p.push(shiny(disc_z(Vec3::new(x, -0.36, 0.08), 0.07, 0.03, 0x14091f), 0.3));
        p.push(glow(disc_z(Vec3::new(x, -0.36, 0.1), 0.075, 0.005, SORCERER_C)));
        p.push(shiny(disc_z(Vec3::new(x, -0.36, 0.105), 0.055, 0.01, 0x14091f), 0.3));
    }
    p
}

/// Panneau orange du Forgebraise: deux engrenages. Grippé, ils sont figés, une barre tordue les bloque, la rouille coule et
/// le voyant est éteint. Réparé, ils tournent et le voyant brille.
fn wall_gears(fixed: bool, t: f32) -> Vec<Part> {
    let mut p = wall_slab(0x6a5a50);
    p.push(shiny(block(Vec3::ZERO, Vec3::new(0.5, 0.5, 0.06), Mat3::IDENTITY, 0x2c2a34), 0.3));
    let step = if fixed { t * TAU / 12.0 } else { 0.2 };
    let cog = |center: Vec3, radius: f32, teeth: usize, turn: f32, color: u32| {
        let mut parts = vec![shiny(disc_z(center + Vec3::Z * 0.02, radius, 0.06, color), 0.7)];
        for k in 0..teeth {
            let a = k as f32 * TAU / teeth as f32 + turn;
            parts.push(shiny(
                block(center + Vec3::new(a.cos() * (radius + 0.02), a.sin() * (radius + 0.02), 0.02), Vec3::new(0.045, 0.045, 0.03), Mat3::from_rotation_z(a), color),
                0.7,
            ));
        }
        parts.push(shiny(disc_z(center + Vec3::Z * 0.06, radius * 0.3, 0.04, 0x2a1a10), 0.5));
        parts
    };
    let big = if fixed { FORGE_C } else { 0xa04a14 };
    p.extend(cog(Vec3::new(-0.12, 0.08, 0.06), 0.27, 12, step, big));
    p.extend(cog(Vec3::new(0.27, -0.2, 0.06), 0.15, 7, -step * 12.0 / 7.0 + 0.2, if fixed { 0xffa040 } else { 0x8a4010 }));
    // Voyant: éteint et rouge sombre quand c'est grippé, orange vif quand c'est réparé.
    if fixed {
        p.push(glow(ball(Vec3::new(0.34, 0.38, 0.12), 0.055 * (0.9 + 0.1 * (t * TAU).sin()), 0xffb050)));
    } else {
        p.push(shiny(ball(Vec3::new(0.34, 0.38, 0.12), 0.05, 0x5a1c10), 0.3));
        // Barre de fer tordue coincée entre les dents, coulées de rouille.
        p.push(shiny(limb(Vec3::new(-0.42, -0.1, 0.14), Vec3::new(0.2, 0.28, 0.14), 0.035, 0x4a4e5a), 0.6));
        p.push(shiny(limb(Vec3::new(0.2, 0.28, 0.14), Vec3::new(0.3, 0.12, 0.14), 0.035, 0x4a4e5a), 0.6));
        for (x, h) in [(-0.3, 0.2), (0.0, 0.3), (0.2, 0.15)] {
            p.push(block(Vec3::new(x, -0.1 - h / 2.0, 0.12), Vec3::new(0.018, h / 2.0, 0.01), Mat3::IDENTITY, 0x7a3a1a));
        }
    }
    // Léger rappel orange sur le cadre, pour qu'on lise « Forgebraise » de loin.
    for (x, y, w, h) in [(0.0, 0.5, 0.5, 0.025), (0.0, -0.5, 0.5, 0.025)] {
        p.push(glow(block(Vec3::new(x, y, 0.12), Vec3::new(w, h, 0.01), Mat3::IDENTITY, if fixed { FORGE_C } else { 0x8a3a10 })));
    }
    p
}

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Empty,
    Mounted,
    Hit,
}

/// Panneau vert du Rôdeur: un support vide, puis une cible garnie, puis la cible touchée par la flèche.
fn wall_target(state: Target, t: f32) -> Vec<Part> {
    let pulse = 0.85 + 0.15 * (t * TAU).sin();
    let mut p = wall_slab(0x4a6a50);
    p.push(shiny(ring(0.44, 0.0, 0x10281a), 0.4));
    match state {
        Target::Empty => {
            // Support vide: quatre crochets et un anneau vert éteint.
            p.push(glow(ring(0.4, 0.005, 0x1f5a2c)));
            p.push(shiny(ring(0.36, 0.012, 0x0c1c12), 0.3));
            for k in 0..4 {
                let a = k as f32 * TAU / 4.0 + 0.785;
                p.push(shiny(block(Vec3::new(a.cos() * 0.38, a.sin() * 0.38, 0.12), Vec3::new(0.05, 0.02, 0.02), Mat3::from_rotation_z(a), 0x9aa0b0), 0.8));
            }
        }
        Target::Mounted | Target::Hit => {
            // Cercle vert lumineux derrière, puis les anneaux blancs et verts de plus en plus en avant.
            p.push(glow(ring(0.46, 0.0, RANGER_C)));
            p.push(ring(0.41, 0.01, 0xf0f0e0));
            p.push(ring(0.33, 0.02, 0x2aa040));
            p.push(ring(0.25, 0.03, 0xf0f0e0));
            p.push(ring(0.17, 0.04, 0x2aa040));
            p.push(glow(ring(0.08 * pulse, 0.05, 0xb0ffb8)));
            if state == Target::Hit {
                // La flèche plantée au centre, empennage vers le spectateur.
                p.push(shiny(limb(Vec3::new(0.0, 0.0, 0.12), Vec3::new(0.09, 0.06, 0.55), 0.022, 0x8a6a40), 0.4));
                p.push(shiny(cone(Vec3::new(0.08, 0.055, 0.45), Vec3::new(0.14, 0.12, 0.62), 0.07, 0.012, 0xe8e0c8), 0.4));
                p.push(shiny(cone(Vec3::new(0.08, 0.055, 0.45), Vec3::new(0.03, 0.14, 0.62), 0.07, 0.012, 0xe8e0c8), 0.4));
                p.push(glow(ball(Vec3::new(0.0, 0.0, 0.14), 0.045, 0xffffff)));
            }
        }
    }
    p
}

/// Une moitié de herse: barreaux de fer à pointes, traverses, montants sombres, et un voyant vert (le Rôdeur la désactive).
fn portcullis(t: f32) -> Vec<Part> {
    let iron = 0x4a5060;
    let mut p = vec![
        shiny(block(Vec3::new(-0.57, 0.95, 0.0), Vec3::new(0.06, 0.95, 0.14), Mat3::IDENTITY, 0x2c303a), 0.5),
        shiny(block(Vec3::new(0.57, 0.95, 0.0), Vec3::new(0.06, 0.95, 0.14), Mat3::IDENTITY, 0x2c303a), 0.5),
        shiny(block(Vec3::new(0.0, 1.88, 0.0), Vec3::new(0.63, 0.07, 0.14), Mat3::IDENTITY, 0x2c303a), 0.5),
    ];
    for x in [-0.45f32, -0.225, 0.0, 0.225, 0.45] {
        p.push(shiny(block(Vec3::new(x, 0.95, 0.0), Vec3::new(0.035, 0.85, 0.035), Mat3::IDENTITY, iron), 0.7));
        p.push(shiny(cone(Vec3::new(x, 0.12, 0.0), Vec3::new(x, 0.02, 0.0), 0.04, 0.004, 0xb0b6c4), 0.9));
    }
    for y in [0.5, 1.05, 1.6] {
        p.push(shiny(block(Vec3::new(0.0, y, 0.04), Vec3::new(0.52, 0.03, 0.03), Mat3::IDENTITY, iron), 0.7));
    }
    let glint = 0.9 + 0.1 * (t * TAU).sin();
    p.push(shiny(ball(Vec3::new(0.0, 1.88, 0.16), 0.07, 0x1c3a24), 0.5));
    p.push(glow(ball(Vec3::new(0.0, 1.88, 0.2), 0.045 * glint, RANGER_C)));
    p
}

/// Ingrédient du sort de l'Ensorceleur: trois cristaux violets sur une petite coupelle de pierre.
fn ingredient(t: f32) -> Vec<Part> {
    let bob = 0.015 * (t * TAU).sin();
    let mut p = vec![shiny(cone(Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.0, 0.12, 0.0), 0.3, 0.25, 0x4a4058), 0.4)];
    for (x, z, h, lean) in [(-0.1, 0.0, 0.46, -0.15), (0.1, 0.04, 0.36, 0.18), (0.0, -0.08, 0.3, 0.0)] {
        let base = Vec3::new(x, 0.12, z);
        let tip = base + Vec3::new(lean * 0.4, h + bob, 0.0);
        p.push(shiny(cone(base, tip, 0.075, 0.012, SORCERER_C), 0.9));
        p.push(glow(ball(tip, 0.022, 0xf0c8ff)));
    }
    let a = t * TAU;
    p.push(glow(ball(Vec3::new(a.cos() * 0.26, 0.38 + 0.05 * a.sin(), a.sin() * 0.2), 0.022, 0xe0a0ff)));
    p
}

/// Pièce de rechange du Forgebraise: un engrenage orange posé debout, qui tourne doucement.
fn gear_part(t: f32) -> Vec<Part> {
    let c = Vec3::new(0.0, 0.34 + 0.02 * (t * TAU).sin(), 0.0);
    let turn = t * TAU / 10.0;
    let mut p = vec![
        shiny(disc_z(c, 0.22, 0.07, FORGE_C), 0.9),
        shiny(disc_z(c + Vec3::Z * 0.04, 0.07, 0.05, 0x2a1a10), 0.5),
        glow(ball(c + Vec3::new(-0.08, 0.1, 0.08), 0.03, 0xffe0b0)),
    ];
    for k in 0..10 {
        let a = k as f32 * TAU / 10.0 + turn;
        p.push(shiny(block(c + Vec3::new(a.cos() * 0.25, a.sin() * 0.25, 0.0), Vec3::new(0.05, 0.05, 0.035), Mat3::from_rotation_z(a), FORGE_C), 0.9));
    }
    p
}

/// Disque de cible du Rôdeur: anneaux verts et blancs sur un petit pied.
fn target_disc(t: f32) -> Vec<Part> {
    let c = Vec3::new(0.0, 0.4 + 0.02 * (t * TAU).sin(), 0.0);
    let mut p = vec![
        shiny(limb(Vec3::new(-0.16, 0.0, -0.05), c + Vec3::new(-0.02, -0.12, -0.04), 0.022, 0x5a3a20), 0.3),
        shiny(limb(Vec3::new(0.16, 0.0, -0.05), c + Vec3::new(0.02, -0.12, -0.04), 0.022, 0x5a3a20), 0.3),
        shiny(disc_z(c, 0.27, 0.05, 0xf0f0e0), 0.5),
        shiny(disc_z(c + Vec3::Z * 0.01, 0.2, 0.05, 0x2aa040), 0.6),
        shiny(disc_z(c + Vec3::Z * 0.02, 0.13, 0.05, 0xf0f0e0), 0.5),
        shiny(disc_z(c + Vec3::Z * 0.03, 0.07, 0.05, 0x2aa040), 0.6),
        glow(ball(c + Vec3::new(0.0, 0.0, 0.07), 0.025 * (0.9 + 0.1 * (t * TAU).sin()), 0xb0ffb8)),
    ];
    // Liseré vert lumineux derrière le disque.
    p.push(glow(disc_z(c - Vec3::Z * 0.02, 0.3, 0.01, RANGER_C)));
    p
}

fn key(t: f32) -> Vec<Part> {
    let yaw = Mat3::from_rotation_y(0.5 * (t * TAU).sin());
    let c = Vec3::new(0.0, 0.4 + 0.03 * (t * TAU).sin(), 0.0);
    let mut p = Vec::new();
    for k in 0..10 {
        let a = k as f32 * TAU / 10.0;
        p.push(shiny(ball(c + yaw * Vec3::new(-0.22 + a.cos() * 0.15, a.sin() * 0.15, 0.0), 0.045, 0xf6c93a), 0.9));
    }
    p.push(shiny(limb(c + yaw * Vec3::new(-0.08, 0.0, 0.0), c + yaw * Vec3::new(0.45, 0.0, 0.0), 0.045, 0xf6c93a), 0.9));
    for x in [0.3, 0.4] {
        p.push(shiny(block(c + yaw * Vec3::new(x, -0.07, 0.0), Vec3::new(0.025, 0.07, 0.03), yaw, 0xf6c93a), 0.9));
    }
    p.push(glow(ball(c + yaw * Vec3::new(-0.3, 0.14, 0.04), 0.025, 0xffffff)));
    p
}

fn lever(t: f32) -> Vec<Part> {
    let sway = 0.04 * (t * TAU).sin();
    vec![
        shiny(block(Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.45, 0.14, 0.32), Mat3::IDENTITY, STONE), 0.3),
        block(Vec3::new(0.0, 0.29, 0.0), Vec3::new(0.3, 0.015, 0.06), Mat3::IDENTITY, DARK),
        shiny(limb(Vec3::new(0.0, 0.3, 0.0), Vec3::new(-0.2 + sway, 1.0, 0.08), 0.05, 0x9aa0b0), 0.8),
        shiny(ball(Vec3::new(-0.21 + sway, 1.06, 0.09), 0.11, 0xe03040), 0.9),
        ball(Vec3::new(0.0, 0.3, 0.0), 0.08, BRASS),
    ]
}

fn rune(t: f32) -> Vec<Part> {
    let pulse = 0.5 + 0.5 * (t * TAU).sin();
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.04, 0.0), 1.0, 1.0, 0x2a2040), 0.3),
        glow(cone(Vec3::new(0.0, 0.04, 0.0), Vec3::new(0.0, 0.06, 0.0), 0.92, 0.82, 0x40d8f0)),
        glow(cone(Vec3::new(0.0, 0.04, 0.0), Vec3::new(0.0, 0.07, 0.0), 0.5, 0.44, 0x7a60ff)),
    ];
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        p.push(glow(block(
            Vec3::new(a.cos() * 0.68, 0.07, a.sin() * 0.68),
            Vec3::new(0.14, 0.01, 0.03),
            Mat3::from_rotation_y(-a),
            0x60e8ff,
        )));
    }
    for k in 0..3 {
        let a = t * TAU / 3.0 + k as f32 * TAU / 3.0;
        p.push(glow(block(
            Vec3::new(a.cos() * 0.5, 0.45 + 0.12 * pulse + 0.05 * k as f32, a.sin() * 0.5),
            Vec3::new(0.06, 0.06, 0.012),
            Mat3::from_rotation_y(-a) * Mat3::from_rotation_z(0.785),
            0x9affff,
        )));
    }
    p
}

fn mechanism(t: f32) -> Vec<Part> {
    let g = Vec3::new(0.0, 0.85, 0.3);
    let mut p = vec![
        shiny(block(Vec3::new(0.0, 0.22, 0.0), Vec3::new(0.65, 0.22, 0.45), Mat3::IDENTITY, STONE), 0.3),
        shiny(disc_z(g, 0.4, 0.1, 0xb08838), 0.8),
        shiny(disc_z(g + Vec3::Z * 0.06, 0.12, 0.06, 0x6a4a20), 0.6),
        glow(block(Vec3::new(0.3, 0.3, 0.46), Vec3::new(0.16, 0.1, 0.01), Mat3::IDENTITY, 0xff8a2a)),
        shiny(limb(Vec3::new(-0.5, 0.42, 0.0), Vec3::new(-0.5, 1.3, -0.1), 0.055, 0x7f8aa6), 0.6),
    ];
    // Dents de l'engrenage: elles tournent d'un pas sur le cycle.
    for k in 0..10 {
        let a = k as f32 * TAU / 10.0 + t * TAU / 10.0 * 0.9;
        p.push(shiny(block(g + Vec3::new(a.cos() * 0.44, a.sin() * 0.44, 0.0), Vec3::new(0.065, 0.065, 0.055), Mat3::from_rotation_z(a), 0xb08838), 0.8));
    }
    let small = Vec3::new(0.45, 0.62, 0.3);
    p.push(shiny(disc_z(small, 0.16, 0.08, 0x9aa0b0), 0.7));
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0 - t * TAU / 6.0 * 0.9;
        p.push(block(small + Vec3::new(a.cos() * 0.18, a.sin() * 0.18, 0.0), Vec3::new(0.04, 0.04, 0.04), Mat3::from_rotation_z(a), 0x9aa0b0));
    }
    p
}

fn trap(armed: bool, t: f32) -> Vec<Part> {
    let mut p = vec![
        shiny(block(Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.62, 0.05, 0.62), Mat3::IDENTITY, 0x5a6070), 0.5),
        block(Vec3::new(0.0, 0.11, 0.0), Vec3::new(0.56, 0.015, 0.56), Mat3::IDENTITY, 0x3a4050),
    ];
    for ix in -1..=1 {
        for iz in -1..=1 {
            let (x, z) = (ix as f32 * 0.32, iz as f32 * 0.32);
            if armed {
                let h = 0.5 + 0.03 * (t * TAU + ix as f32 + iz as f32).sin();
                p.push(shiny(cone(Vec3::new(x, 0.1, z), Vec3::new(x, 0.1 + h, z), 0.07, 0.004, 0xd8dff0), 0.9));
            } else {
                p.push(ellipsoid(Vec3::new(x, 0.11, z), Vec3::new(0.07, 0.02, 0.07), DARK));
                p.push(cone(Vec3::new(x, 0.1, z), Vec3::new(x, 0.17, z), 0.05, 0.004, 0x9aa0b0));
            }
        }
    }
    if armed {
        p.push(block(Vec3::new(0.22, 0.12, 0.45), Vec3::new(0.05, 0.005, 0.03), Mat3::IDENTITY, 0x8a1c2c));
    }
    p
}

fn door_locked(t: f32) -> Vec<Part> {
    let mut p = vec![
        shiny(block(Vec3::new(-0.58, 0.95, 0.0), Vec3::new(0.12, 0.95, 0.16), Mat3::IDENTITY, STONE_L), 0.3),
        shiny(block(Vec3::new(0.58, 0.95, 0.0), Vec3::new(0.12, 0.95, 0.16), Mat3::IDENTITY, STONE_L), 0.3),
        shiny(block(Vec3::new(0.0, 1.95, 0.0), Vec3::new(0.7, 0.1, 0.16), Mat3::IDENTITY, STONE), 0.3),
    ];
    for (k, x) in [-0.36, -0.12, 0.12, 0.36].into_iter().enumerate() {
        p.push(shiny(block(Vec3::new(x, 0.9, 0.0), Vec3::new(0.115, 0.9, 0.06), Mat3::IDENTITY, if k % 2 == 0 { 0x8a5630 } else { 0x74462a }), 0.4));
    }
    for y in [0.3, 1.45] {
        p.push(shiny(block(Vec3::new(0.0, y, 0.06), Vec3::new(0.5, 0.07, 0.04), Mat3::IDENTITY, 0x4a5060), 0.7));
        for x in [-0.4, -0.13, 0.13, 0.4] {
            p.push(ball(Vec3::new(x, y, 0.1), 0.025, 0x9aa0b0));
        }
    }
    p.push(shiny(block(Vec3::new(0.0, 0.85, 0.1), Vec3::new(0.14, 0.17, 0.03), Mat3::IDENTITY, 0xf6c93a), 0.9));
    p.push(ball(Vec3::new(0.0, 0.88, 0.14), 0.04, DARK));
    p.push(block(Vec3::new(0.0, 0.8, 0.14), Vec3::new(0.014, 0.05, 0.01), Mat3::IDENTITY, DARK));
    p.push(glow(ball(Vec3::new(0.06 + 0.02 * (t * TAU).sin(), 0.95, 0.15), 0.015, 0xffffff)));
    p
}

fn door_royal(emblem: bool, t: f32) -> Vec<Part> {
    let slab = 0x3e4672;
    let mut p = vec![
        shiny(block(Vec3::new(0.0, 0.95, 0.0), Vec3::new(0.6, 0.95, 0.14), Mat3::IDENTITY, slab), 0.4),
        shiny(block(Vec3::new(-0.58, 0.95, 0.04), Vec3::new(0.05, 0.95, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        shiny(block(Vec3::new(0.58, 0.95, 0.04), Vec3::new(0.05, 0.95, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        shiny(block(Vec3::new(0.0, 1.85, 0.04), Vec3::new(0.6, 0.05, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        shiny(block(Vec3::new(0.0, 0.05, 0.04), Vec3::new(0.6, 0.05, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        block(Vec3::new(0.0, 0.95, 0.12), Vec3::new(0.44, 0.78, 0.02), Mat3::IDENTITY, 0x4a5488),
        glow(block(Vec3::new(0.0, 0.95, 0.15), Vec3::new(0.01, 0.78, 0.01), Mat3::IDENTITY, 0xf6c93a)),
    ];
    if emblem {
        let glint = 0.9 + 0.1 * (t * TAU).sin();
        p.push(shiny(block(Vec3::new(0.0, 0.95, 0.16), Vec3::new(0.26, 0.09, 0.03), Mat3::IDENTITY, 0xf6c93a), 0.9));
        for (x, h) in [(-0.2, 0.2), (-0.1, 0.3), (0.0, 0.38), (0.1, 0.3), (0.2, 0.2)] {
            p.push(shiny(cone(Vec3::new(x, 1.0, 0.16), Vec3::new(x, 1.0 + h, 0.16), 0.045, 0.012, 0xf6c93a), 0.9));
            p.push(glow(ball(Vec3::new(x, 1.03 + h, 0.16), 0.03 * glint, 0x58a0ff)));
        }
        // Écu d'acier bleu: c'est le Chevalier qui pousse cette porte.
        p.push(glow(block(Vec3::new(0.0, 0.5, 0.17), Vec3::new(0.14, 0.14, 0.015), Mat3::from_rotation_z(0.785), KNIGHT_C)));
        p.push(glow(block(Vec3::new(0.0, 0.5, 0.18), Vec3::new(0.05, 0.05, 0.015), Mat3::from_rotation_z(0.785), 0xd8ecff)));
    }
    p
}

fn portal(t: f32) -> Vec<Part> {
    let c = Vec3::new(0.0, 1.15, 0.0);
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.14, 0.0), 1.25, 1.1, STONE), 0.3),
        shiny(cone(Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.0, 0.26, 0.0), 1.0, 0.9, STONE_L), 0.3),
    ];
    // Anneau de pierre runique.
    for k in 0..14 {
        let a = k as f32 * TAU / 14.0;
        let pos = c + Vec3::new(a.cos() * 1.0, a.sin() * 1.0, 0.0);
        p.push(shiny(block(pos, Vec3::new(0.16, 0.12, 0.14), Mat3::from_rotation_z(a), if k % 2 == 0 { STONE } else { STONE_L }), 0.4));
        p.push(glow(block(pos + Vec3::Z * 0.13, Vec3::new(0.05, 0.02, 0.01), Mat3::from_rotation_z(a), 0x60e8ff)));
    }
    // Tourbillon: disques concentriques et bras en spirale qui tournent.
    p.push(glow(disc_z(c + Vec3::Z * -0.02, 0.86, 0.02, 0x3a2a9a)));
    p.push(glow(disc_z(c, 0.64, 0.02, 0x6a50e8)));
    p.push(glow(disc_z(c + Vec3::Z * 0.02, 0.42, 0.02, 0x40c0f0)));
    p.push(glow(disc_z(c + Vec3::Z * 0.04, 0.2, 0.02, 0xd8f8ff)));
    for arm in 0..4 {
        for step in 0..6 {
            let a = arm as f32 * FRAC_PI_2 + t * FRAC_PI_2 + step as f32 * 0.35;
            let r = 0.12 + step as f32 * 0.12;
            p.push(glow(ball(c + Vec3::new(a.cos() * r, a.sin() * r, 0.08), 0.045 + 0.005 * step as f32, 0xaaf4ff)));
        }
    }
    p
}

pub fn build(prop: Prop, t: f32, theme: Theme) -> Vec<Part> {
    let mut parts = match theme {
        Theme::Manga | Theme::Medieval => build_manga(prop, t),
        Theme::Gore => crate::props_gore::build(prop, t),
    };
    crate::tuning::apply(&crate::tuning::prop_key(prop), &mut parts);
    parts
}

pub(crate) fn build_manga(prop: Prop, t: f32) -> Vec<Part> {
    match prop {
        Prop::Generator => generator(t),
        Prop::Turret => turret(t),
        Prop::Coin => coin(t),
        Prop::Food => food(t),
        Prop::Potion => potion(t),
        Prop::WallRune => wall_rune(t),
        Prop::WallGearsJammed => wall_gears(false, t),
        Prop::WallGearsFixed => wall_gears(true, t),
        Prop::WallTargetEmpty => wall_target(Target::Empty, t),
        Prop::WallTargetMounted => wall_target(Target::Mounted, t),
        Prop::WallTargetHit => wall_target(Target::Hit, t),
        Prop::Portcullis => portcullis(t),
        Prop::Ingredient => ingredient(t),
        Prop::GearPart => gear_part(t),
        Prop::TargetDisc => target_disc(t),
        Prop::Key => key(t),
        Prop::Lever => lever(t),
        Prop::Rune => rune(t),
        Prop::Mechanism => mechanism(t),
        Prop::TrapArmed => trap(true, t),
        Prop::TrapOff => trap(false, t),
        Prop::DoorLocked => door_locked(t),
        Prop::DoorRoyal => door_royal(false, t),
        Prop::DoorRoyalEmblem => door_royal(true, t),
        Prop::Portal => portal(t),
    }
}
