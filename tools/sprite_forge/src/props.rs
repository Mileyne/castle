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
    /// Escalier de sortie du niveau (la sortie d'un niveau intérieur).
    Stairs,
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
    /// Sorties des niveaux extérieurs, fermées puis ouvertes: la barrière du sentier, le pont brisé puis réparé, le pont-levis
    /// levé puis baissé.
    GateClosed,
    GateOpen,
    BridgeBroken,
    BridgeRepaired,
    DrawbridgeRaised,
    DrawbridgeLowered,
    /// Un prisonnier dans une cage de fer, à délivrer (quête de sauvetage).
    CaptiveCage,
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
pub const PROPS: [Prop; 26] = [
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
    Prop::Stairs,
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
    Prop::CaptiveCage,
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

/// Escalier de pierre qui s'enfonce sous une arche: la sortie du niveau. Des torches vacillent de chaque côté.
fn stairs(t: f32) -> Vec<Part> {
    let mut p = Vec::new();
    // Dalle de seuil, puis marches qui descendent vers le fond sombre.
    p.push(shiny(block(Vec3::new(0.0, 0.04, 0.2), Vec3::new(1.0, 0.04, 0.9), Mat3::IDENTITY, STONE_L), 0.3));
    for step in 0..5 {
        let depth = 0.55 - step as f32 * 0.2;
        let drop = step as f32 * 0.05;
        let color = if step % 2 == 0 { STONE } else { STONE_L };
        p.push(shiny(block(Vec3::new(0.0, 0.1 - drop, depth), Vec3::new(0.72 - 0.03 * step as f32, 0.05, 0.11), Mat3::IDENTITY, color), 0.3));
    }
    // Le trou sombre au fond, et l'arche qui l'encadre.
    p.push(block(Vec3::new(0.0, 0.5, -0.45), Vec3::new(0.62, 0.55, 0.05), Mat3::IDENTITY, DARK));
    // Dans l'ouverture, les marches qui descendent: vues d'en haut, les plus lointaines paraissent plus hautes et plus
    // sombres.
    for (step, color) in [0x8a8098u32, 0x6a6078, 0x4a4258, 0x34304a, 0x252036].into_iter().enumerate() {
        let k = step as f32;
        p.push(shiny(block(Vec3::new(0.0, 0.12 + 0.12 * k, -0.36 + 0.0 * k), Vec3::new(0.58 - 0.04 * k, 0.035, 0.07), Mat3::IDENTITY, color), 0.2));
    }
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(block(Vec3::new(sg * 0.8, 0.62, -0.3), Vec3::new(0.16, 0.62, 0.2), Mat3::IDENTITY, STONE), 0.4));
        p.push(shiny(block(Vec3::new(sg * 0.8, 1.28, -0.3), Vec3::new(0.2, 0.05, 0.24), Mat3::IDENTITY, STONE_L), 0.4));
        // Torche: manche, flamme qui vacille.
        let flicker = 1.0 + 0.25 * (t * TAU * 3.0 + sg).sin();
        p.push(block(Vec3::new(sg * 0.8, 0.86, -0.08), Vec3::new(0.025, 0.12, 0.025), Mat3::IDENTITY, 0x4a3020));
        p.push(glow(ball(Vec3::new(sg * 0.8, 1.04, -0.08), 0.07 * flicker, 0xffa030)));
        p.push(glow(ball(Vec3::new(sg * 0.8, 1.1, -0.08), 0.035 * flicker, 0xfff0a0)));
    }
    p.push(shiny(block(Vec3::new(0.0, 1.36, -0.3), Vec3::new(1.0, 0.1, 0.22), Mat3::IDENTITY, STONE), 0.4));
    p.push(shiny(block(Vec3::new(0.0, 1.5, -0.3), Vec3::new(0.7, 0.06, 0.18), Mat3::IDENTITY, STONE_L), 0.4));
    p
}

const WOOD: u32 = 0x7a5232;
const WOOD_D: u32 = 0x4a3220;
const WOOD_L: u32 = 0x9a6c40;
const ROPE: u32 = 0xb89860;
const IRON_BAND: u32 = 0x3a3e4a;

/// Un prisonnier dans une cage de fer, suspendue à une chaîne: il s'agrippe aux barreaux, tête basse, les vêtements en
/// lambeaux; ses mains et sa tête bougent un peu. Un cadenas ferme la porte.
fn captive_cage(t: f32) -> Vec<Part> {
    let iron = 0x4a4e5a;
    let iron_l = 0x7a8090;
    let skin = 0xe0b890;
    let tunic = 0x8a7a54;
    let mut p = Vec::new();
    // Plancher de bois de la cage et ses pieds.
    p.push(shiny(block(Vec3::new(0.0, 0.06, 0.0), Vec3::new(0.56, 0.05, 0.56), Mat3::IDENTITY, WOOD_D), 0.2));
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        p.push(block(Vec3::new(sx * 0.5, 0.02, sz * 0.5), Vec3::new(0.07, 0.03, 0.07), Mat3::IDENTITY, WOOD_D));
    }
    // Le prisonnier: jambes, tunique déchirée, ceinture de corde, tête et cheveux, visage triste.
    let sway = 0.025 * (t * TAU).sin();
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(limb(Vec3::new(sg * 0.09, 0.14, 0.0), Vec3::new(sg * 0.1, 0.5, 0.02), 0.055, 0x5a4a3a), 0.15));
        p.push(shiny(ellipsoid(Vec3::new(sg * 0.1, 0.12, 0.05), Vec3::new(0.07, 0.05, 0.11), 0x3a2a20), 0.2));
    }
    p.push(shiny(ellipsoid(Vec3::new(0.0, 0.78, 0.0), Vec3::new(0.2, 0.3, 0.15), tunic), 0.1));
    p.push(block(Vec3::new(0.0, 0.62, 0.14), Vec3::new(0.2, 0.025, 0.02), Mat3::IDENTITY, ROPE));
    for k in 0..4 {
        let x = -0.15 + k as f32 * 0.1;
        p.push(ellipsoid(Vec3::new(x, 0.5 - 0.03 * (k % 2) as f32, 0.04), Vec3::new(0.05, 0.1, 0.03), tunic));
    }
    let head = Vec3::new(sway, 1.12, 0.03);
    p.push(shiny(ball(head, 0.15, skin), 0.2));
    p.push(shiny(ellipsoid(head + Vec3::new(0.0, 0.07, -0.03), Vec3::new(0.155, 0.1, 0.15), 0x4a3220), 0.2));
    for sg in [-1.0f32, 1.0] {
        p.push(ball(head + Vec3::new(sg * 0.055, -0.01, 0.13), 0.016, 0x2a2030));
        p.push(block(head + Vec3::new(sg * 0.055, 0.035, 0.14), Vec3::new(0.035, 0.008, 0.01), Mat3::from_rotation_z(-sg * 0.35), 0x2a2030));
    }
    p.push(ellipsoid(head + Vec3::new(0.0, -0.075, 0.13), Vec3::new(0.04, 0.012, 0.02), 0x8a4a40));
    // Bras levés: les mains agrippent les barreaux de devant et tremblent.
    for sg in [-1.0f32, 1.0] {
        let hand = Vec3::new(sg * 0.38, 0.98 + 0.03 * (t * TAU * 2.0 + sg).sin(), 0.42);
        p.push(shiny(limb(Vec3::new(sg * 0.2, 0.92, 0.04), hand, 0.045, skin), 0.15));
        p.push(shiny(ball(hand, 0.06, skin), 0.15));
    }
    // La cage: barreaux autour (ceux de devant sont plus fins pour laisser voir), anneaux, toit en dôme, chaîne et cadenas.
    for k in 0..12 {
        let a = k as f32 * TAU / 12.0;
        let (x, z) = (a.cos() * 0.5, a.sin() * 0.5);
        p.push(shiny(limb(Vec3::new(x, 0.1, z), Vec3::new(x * 0.96, 1.35, z * 0.96), 0.022, iron), 0.5));
    }
    for y in [0.32f32, 0.78, 1.28] {
        for k in 0..12 {
            let a0 = k as f32 * TAU / 12.0;
            let a1 = (k + 1) as f32 * TAU / 12.0;
            let r = 0.5 - 0.02 * (y / 1.35);
            p.push(shiny(limb(Vec3::new(a0.cos() * r, y, a0.sin() * r), Vec3::new(a1.cos() * r, y, a1.sin() * r), 0.016, iron_l), 0.5));
        }
    }
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0;
        p.push(shiny(limb(Vec3::new(a.cos() * 0.48, 1.34, a.sin() * 0.48), Vec3::new(0.0, 1.55, 0.0), 0.02, iron), 0.5));
    }
    p.push(shiny(ball(Vec3::new(0.0, 1.57, 0.0), 0.05, iron_l), 0.6));
    p.push(shiny(limb(Vec3::new(0.0, 1.6, 0.0), Vec3::new(0.0, 1.95, 0.0), 0.022, iron_l), 0.6));
    p.push(shiny(block(Vec3::new(0.0, 0.62, 0.52), Vec3::new(0.06, 0.07, 0.025), Mat3::IDENTITY, 0xb89030), 0.7));
    p.push(shiny(limb(Vec3::new(-0.03, 0.69, 0.52), Vec3::new(0.03, 0.69, 0.52), 0.012, 0xb89030), 0.7));
    p
}

/// Un rondin vertical à pointe taillée.
fn log(x: f32, z: f32, height: f32, radius: f32, color: u32) -> Vec<Part> {
    vec![
        shiny(limb(Vec3::new(x, 0.0, z), Vec3::new(x, height, z), radius, color), 0.15),
        shiny(cone(Vec3::new(x, height - 0.02, z), Vec3::new(x, height + radius * 2.2, z), radius * 0.98, 0.012, color), 0.15),
    ]
}

/// La barrière du sentier: une palissade de rondins entre deux gros poteaux, que deux battants ferment ou laissent
/// ouverte. Une lanterne vacille sur chaque poteau.
fn gate(open: bool, t: f32) -> Vec<Part> {
    let mut p = Vec::new();
    for sg in [-1.0f32, 1.0] {
        // Gros poteau avec sa lanterne, ses cordes et sa pierre de calage.
        p.extend(log(sg * 1.05, 0.0, 1.55, 0.14, WOOD_D));
        p.push(shiny(block(Vec3::new(sg * 1.05, 0.05, 0.0), Vec3::new(0.24, 0.05, 0.24), Mat3::IDENTITY, STONE), 0.2));
        p.push(shiny(block(Vec3::new(sg * 1.05, 1.1, 0.0), Vec3::new(0.16, 0.025, 0.16), Mat3::IDENTITY, ROPE), 0.1));
        let flicker = 1.0 + 0.2 * (t * TAU * 3.0 + sg).sin();
        p.push(block(Vec3::new(sg * 1.05, 1.75, 0.0), Vec3::new(0.05, 0.07, 0.05), Mat3::IDENTITY, IRON_BAND));
        p.push(glow(ball(Vec3::new(sg * 1.05, 1.78, 0.0), 0.06 * flicker, 0xffb040)));
    }
    if open {
        // Les deux battants sont rabattus vers l'avant: vus de face, ils sont de profil le long des poteaux.
        for sg in [-1.0f32, 1.0] {
            for k in 0..5 {
                p.extend(log(sg * 0.86, 0.1 + k as f32 * 0.17, 1.05, 0.075, if k % 2 == 0 { WOOD } else { WOOD_L }));
            }
            p.push(shiny(block(Vec3::new(sg * 0.86, 0.5, 0.45), Vec3::new(0.04, 0.05, 0.42), Mat3::IDENTITY, IRON_BAND), 0.5));
        }
        // Le sentier derrière: de la terre battue.
        p.push(block(Vec3::new(0.0, 0.01, 0.1), Vec3::new(0.8, 0.012, 0.9), Mat3::IDENTITY, 0x7a5c38));
    } else {
        for k in 0..9 {
            let x = -0.82 + k as f32 * 0.205;
            p.extend(log(x, 0.0, 1.15 + 0.06 * ((k * 7) % 3) as f32, 0.095, if k % 2 == 0 { WOOD } else { WOOD_L }));
        }
        for y in [0.35f32, 0.85] {
            p.push(shiny(block(Vec3::new(0.0, y, 0.1), Vec3::new(0.95, 0.06, 0.04), Mat3::IDENTITY, WOOD_D), 0.2));
            for sg in [-1.0f32, 1.0] {
                p.push(shiny(block(Vec3::new(sg * 0.7, y, 0.14), Vec3::new(0.05, 0.075, 0.02), Mat3::IDENTITY, IRON_BAND), 0.6));
            }
        }
    }
    p
}

/// Le pont du marais, de planches sur l'eau noire: brisé (les planches du milieu manquent, des débris flottent) ou réparé
/// (planches neuves, claires). Des cordes courent le long de poteaux.
fn bridge(repaired: bool, t: f32) -> Vec<Part> {
    let mut p = Vec::new();
    // L'eau en dessous, qui ondule un peu.
    p.push(shiny(ellipsoid(Vec3::new(0.0, 0.02, 0.1), Vec3::new(1.35, 0.02, 0.85), 0x1e3f42), 0.9));
    p.push(shiny(ellipsoid(Vec3::new(-0.3 + 0.1 * (t * TAU).sin(), 0.04, 0.35), Vec3::new(0.35, 0.012, 0.12), 0x4a8a84), 0.9));
    for k in 0..15 {
        let x = -1.0 + k as f32 * (2.0 / 14.0);
        let middle = x.abs() < 0.42;
        if middle && !repaired {
            continue;
        }
        let color = if middle { WOOD_L } else if k % 2 == 0 { WOOD } else { WOOD_D };
        p.push(shiny(block(Vec3::new(x, 0.22, 0.0), Vec3::new(0.065, 0.03, 0.42), Mat3::IDENTITY, color), 0.2));
    }
    // Poutres longitudinales sous les planches (aux deux bouts quand le pont est brisé).
    for x in [-0.72f32, 0.72] {
        p.push(block(Vec3::new(x, 0.15, 0.0), Vec3::new(0.3, 0.04, 0.3), Mat3::IDENTITY, WOOD_D));
    }
    if repaired {
        p.push(block(Vec3::new(0.0, 0.15, 0.0), Vec3::new(0.45, 0.04, 0.3), Mat3::IDENTITY, WOOD_L));
    } else {
        // Planches cassées qui flottent, et deux moignons de bois.
        p.push(shiny(block(Vec3::new(0.15, 0.06, 0.25), Vec3::new(0.2, 0.025, 0.07), Mat3::from_rotation_y(0.4), WOOD_D), 0.2));
        p.push(shiny(block(Vec3::new(-0.2, 0.06, 0.05), Vec3::new(0.18, 0.025, 0.07), Mat3::from_rotation_y(-0.3), WOOD), 0.2));
        for x in [-0.42f32, 0.42] {
            p.push(block(Vec3::new(x, 0.2, 0.0), Vec3::new(0.02, 0.05, 0.4), Mat3::from_rotation_z(x.signum() * 0.5), WOOD_D));
        }
    }
    // Poteaux d'angle et cordes de garde-corps.
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            p.extend(log(sx * 1.05, sz * 0.4, 0.75, 0.05, WOOD_D));
        }
    }
    for sz in [-1.0f32, 1.0] {
        p.push(limb(Vec3::new(-1.05, 0.62, sz * 0.4), Vec3::new(1.05, 0.62 - 0.02 * if repaired { 0.0 } else { 3.0 }, sz * 0.4), 0.014, ROPE));
    }
    p
}

/// Le pont-levis du château: le tablier, retenu par deux chaînes, devant une arche de pierre. Levé, il ferme l'entrée comme une
/// porte; baissé, il fait un pont.
fn drawbridge(lowered: bool, t: f32) -> Vec<Part> {
    let mut p = Vec::new();
    // Arche de pierre et fossé sombre.
    p.push(block(Vec3::new(0.0, 0.5, -0.5), Vec3::new(0.78, 0.5, 0.05), Mat3::IDENTITY, DARK));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(block(Vec3::new(sg * 0.95, 0.85, -0.3), Vec3::new(0.2, 0.85, 0.25), Mat3::IDENTITY, STONE), 0.4));
        p.push(shiny(block(Vec3::new(sg * 0.95, 1.72, -0.3), Vec3::new(0.24, 0.05, 0.29), Mat3::IDENTITY, STONE_L), 0.4));
        // Torche sur chaque pilier.
        let flicker = 1.0 + 0.25 * (t * TAU * 3.0 + sg).sin();
        p.push(glow(ball(Vec3::new(sg * 0.78, 1.05, -0.02), 0.06 * flicker, 0xffa030)));
    }
    p.push(shiny(block(Vec3::new(0.0, 1.62, -0.3), Vec3::new(1.15, 0.12, 0.27), Mat3::IDENTITY, STONE), 0.4));
    p.push(shiny(block(Vec3::new(0.0, 1.78, -0.3), Vec3::new(0.85, 0.06, 0.22), Mat3::IDENTITY, STONE_L), 0.4));
    p.push(shiny(ellipsoid(Vec3::new(0.0, 0.02, 0.45), Vec3::new(1.35, 0.02, 0.5), 0x1a2f34), 0.9));
    if lowered {
        // Tablier à plat sur le fossé, chaînes tendues vers les anneaux de l'arche.
        p.push(shiny(block(Vec3::new(0.0, 0.14, 0.25), Vec3::new(0.78, 0.05, 0.62), Mat3::IDENTITY, WOOD), 0.2));
        for k in 0..6 {
            p.push(block(Vec3::new(0.0, 0.2, -0.3 + k as f32 * 0.22), Vec3::new(0.78, 0.012, 0.012), Mat3::IDENTITY, WOOD_D));
        }
        for sg in [-1.0f32, 1.0] {
            p.push(shiny(block(Vec3::new(sg * 0.74, 0.2, 0.25), Vec3::new(0.03, 0.03, 0.6), Mat3::IDENTITY, IRON_BAND), 0.5));
            p.push(shiny(limb(Vec3::new(sg * 0.74, 0.2, 0.8), Vec3::new(sg * 0.74, 1.35, -0.08), 0.02, 0x6a6e78), 0.7));
        }
    } else {
        // Tablier dressé contre l'arche: une porte de planches cerclée de fer, avec ses chaînes tendues vers le haut.
        p.push(shiny(block(Vec3::new(0.0, 0.78, 0.0), Vec3::new(0.78, 0.78, 0.06), Mat3::IDENTITY, WOOD), 0.2));
        for k in 0..7 {
            p.push(block(Vec3::new(-0.66 + k as f32 * 0.22, 0.78, 0.07), Vec3::new(0.01, 0.76, 0.01), Mat3::IDENTITY, WOOD_D));
        }
        for y in [0.3f32, 0.78, 1.26] {
            p.push(shiny(block(Vec3::new(0.0, y, 0.09), Vec3::new(0.8, 0.05, 0.02), Mat3::IDENTITY, IRON_BAND), 0.6));
        }
        for sg in [-1.0f32, 1.0] {
            p.push(shiny(limb(Vec3::new(sg * 0.74, 1.5, 0.1), Vec3::new(sg * 0.74, 1.6, -0.08), 0.02, 0x6a6e78), 0.7));
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
        Prop::Stairs => stairs(t),
        Prop::GateClosed => gate(false, t),
        Prop::GateOpen => gate(true, t),
        Prop::BridgeBroken => bridge(false, t),
        Prop::BridgeRepaired => bridge(true, t),
        Prop::DrawbridgeRaised => drawbridge(false, t),
        Prop::DrawbridgeLowered => drawbridge(true, t),
        Prop::CaptiveCage => captive_cage(t),
    }
}

/// Les sorties de niveau, dans l'ordre des lignes de leur planche (le client utilise les mêmes indices): chacune est dessinée
/// en grand, sur une planche à cases de 192 px, avec plus d'éléments que son modèle de base.
pub const EXITS: [Prop; 7] = [
    Prop::Stairs,
    Prop::GateClosed,
    Prop::GateOpen,
    Prop::BridgeBroken,
    Prop::BridgeRepaired,
    Prop::DrawbridgeRaised,
    Prop::DrawbridgeLowered,
];

/// Facteur d'agrandissement des modèles de sortie.
const EXIT_SCALE: f32 = 1.85;

/// Une bannière de tissu qui pend d'une hampe et ondule.
fn banner(x: f32, top: f32, length: f32, color: u32, t: f32, side: f32) -> Vec<Part> {
    let sway = 0.06 * (t * TAU + x).sin();
    vec![
        block(Vec3::new(x, top + 0.04, 0.0), Vec3::new(0.2, 0.025, 0.025), Mat3::IDENTITY, WOOD_D),
        panel(Vec3::new(x, top, 0.04), Vec3::new(x + sway, top - length, 0.04 + side * 0.03), 0.26, 0.015, color),
        glow(ball(Vec3::new(x + sway * 0.8, top - length * 0.55, 0.07), 0.04, 0xf0d060)),
    ]
}

/// Un brasero de pierre avec ses flammes qui vacillent.
fn brazier(x: f32, t: f32) -> Vec<Part> {
    let flicker = 1.0 + 0.25 * (t * TAU * 2.0 + x).sin();
    vec![
        shiny(block(Vec3::new(x, 0.3, 0.0), Vec3::new(0.14, 0.3, 0.14), Mat3::IDENTITY, STONE), 0.3),
        shiny(cone(Vec3::new(x, 0.58, 0.0), Vec3::new(x, 0.75, 0.0), 0.12, 0.22, STONE_L), 0.3),
        glow(cone(Vec3::new(x, 0.75, 0.0), Vec3::new(x, 0.75 + 0.34 * flicker, 0.0), 0.17, 0.02, 0xff8a20)),
        glow(cone(Vec3::new(x, 0.75, 0.0), Vec3::new(x, 0.75 + 0.2 * flicker, 0.0), 0.09, 0.01, 0xfff0a0)),
    ]
}

/// Le modèle d'une sortie en grand: le modèle de base agrandi, plus des éléments propres à chacune (bannières, braseros,
/// palissade qui se prolonge, tours et créneaux, rochers et roseaux).
pub fn build_exit(prop: Prop, t: f32, theme: Theme) -> Vec<Part> {
    let mut parts = scaled(build(prop, t, theme), EXIT_SCALE);
    match prop {
        Prop::Stairs => {
            // Large parvis, braseros de chaque côté, bannières aux piliers.
            parts.push(shiny(block(Vec3::new(0.0, 0.05, 0.9), Vec3::new(2.0, 0.05, 0.9), Mat3::IDENTITY, STONE_L), 0.3));
            parts.push(shiny(block(Vec3::new(0.0, 0.11, 0.9), Vec3::new(1.7, 0.03, 0.7), Mat3::IDENTITY, STONE), 0.3));
            for sg in [-1.0f32, 1.0] {
                parts.extend(brazier(sg * 2.05, t));
                parts.extend(banner(sg * 1.55, 2.5, 0.9, 0x8a1c2c, t, sg));
            }
        }
        Prop::GateClosed | Prop::GateOpen => {
            // La palissade se prolonge de chaque côté, avec une lanterne et une bannière à chaque poteau.
            for sg in [-1.0f32, 1.0] {
                for k in 0..3 {
                    let x = sg * (2.2 + k as f32 * 0.2);
                    parts.extend(log(x, 0.0, 1.9 - 0.18 * k as f32, 0.14, if k % 2 == 0 { WOOD } else { WOOD_L }));
                }
                parts.extend(banner(sg * 1.55, 2.55, 0.95, 0x2c5a8a, t, sg));
            }
            // Le sentier de terre devant la barrière.
            parts.push(block(Vec3::new(0.0, 0.01, 1.4), Vec3::new(1.5, 0.012, 1.0), Mat3::IDENTITY, 0x7a5c38));
        }
        Prop::BridgeBroken | Prop::BridgeRepaired => {
            // Rochers des berges, roseaux, et une lanterne à chaque bout.
            for sg in [-1.0f32, 1.0] {
                for (dx, r) in [(0.0f32, 0.42f32), (0.45, 0.3), (-0.4, 0.26)] {
                    parts.push(shiny(ellipsoid(Vec3::new(sg * (2.15 + dx * 0.3), 0.18, -0.9 + dx), Vec3::new(r, r * 0.7, r * 0.9), STONE), 0.3));
                }
                for k in 0..6 {
                    let x = sg * (2.0 + k as f32 * 0.07) + 0.1;
                    parts.push(cone(Vec3::new(x, 0.0, 0.9), Vec3::new(x + 0.05 * (k % 2) as f32, 0.7 + 0.12 * (k % 3) as f32, 0.9), 0.03, 0.005, 0x4a6a2c));
                }
                let flicker = 1.0 + 0.2 * (t * TAU * 3.0 + sg).sin();
                parts.push(glow(ball(Vec3::new(sg * 1.95, 1.65, 0.7), 0.09 * flicker, 0xffb040)));
            }
        }
        Prop::DrawbridgeRaised | Prop::DrawbridgeLowered => {
            // Deux tours de pierre à créneaux, avec bannières et torches.
            for sg in [-1.0f32, 1.0] {
                parts.push(shiny(block(Vec3::new(sg * 2.2, 1.4, -0.5), Vec3::new(0.5, 1.4, 0.5), Mat3::IDENTITY, STONE), 0.4));
                parts.push(shiny(block(Vec3::new(sg * 2.2, 2.85, -0.5), Vec3::new(0.58, 0.07, 0.58), Mat3::IDENTITY, STONE_L), 0.4));
                for k in 0..4 {
                    let x = sg * 2.2 - 0.42 + k as f32 * 0.28;
                    parts.push(shiny(block(Vec3::new(x, 3.05, -0.5), Vec3::new(0.09, 0.14, 0.58), Mat3::IDENTITY, STONE), 0.4));
                }
                parts.extend(banner(sg * 1.75, 3.0, 1.1, 0x8a1c2c, t, sg));
            }
        }
        _ => {}
    }
    parts
}
