//! Les quatre héros version sombre et sanglante, dans un esprit loufoque à la Gorillaz: têtes énormes,
//! corps filiformes, grandes mains et grands pieds, yeux dépareillés aux paupières tombantes, grandes bouches
//! pleines de dents recousues, et du sang un peu partout. Le squelette et les poses sont ceux de la version manga.

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::heroes::{Class, Dims, Joints, Palette, Pose, body, held, joints};
use crate::render::Part;
use crate::shapes::*;

const BLOOD: u32 = 0xa01820;
const BLOOD_D: u32 = 0x5a0c14;
const BONE: u32 = 0xe4dcc4;
const TOOTH: u32 = 0xe8e0a8;
const RUST: u32 = 0x7a4a30;
const RUST_D: u32 = 0x4a2c1c;
const IRON: u32 = 0x585c68;
const THREAD: u32 = 0x1c1418;

pub(crate) fn dims(class: Class) -> Dims {
    match class {
        Class::Knight => Dims { thigh: 0.36, shin: 0.36, torso: 0.45, hip_x: 0.1, shoulder_x: 0.22, upper: 0.3, fore: 0.3, head_r: 0.4, head_rise: 0.1, leg_r: 0.06, arm_r: 0.05, torso_rx: 0.2, torso_rz: 0.14 },
        Class::Sorcerer => Dims { thigh: 0.36, shin: 0.36, torso: 0.44, hip_x: 0.09, shoulder_x: 0.19, upper: 0.32, fore: 0.32, head_r: 0.4, head_rise: 0.1, leg_r: 0.05, arm_r: 0.045, torso_rx: 0.17, torso_rz: 0.12 },
        Class::Ranger => Dims { thigh: 0.38, shin: 0.38, torso: 0.45, hip_x: 0.1, shoulder_x: 0.21, upper: 0.32, fore: 0.32, head_r: 0.38, head_rise: 0.1, leg_r: 0.055, arm_r: 0.05, torso_rx: 0.19, torso_rz: 0.13 },
        Class::Forgebraise => Dims { thigh: 0.28, shin: 0.28, torso: 0.42, hip_x: 0.14, shoulder_x: 0.33, upper: 0.26, fore: 0.26, head_r: 0.43, head_rise: 0.08, leg_r: 0.09, arm_r: 0.085, torso_rx: 0.34, torso_rz: 0.26 },
    }
}

/// Visage loufoque: yeux dépareillés (un énorme, un minuscule), paupières tombantes, cernes, grosse bouche
/// pleine de dents inégales recousue de fil, nez bulbeux, balafre. `grin` ouvre grand la bouche.
fn face(h: Vec3, r: f32, skin: u32, big_eye: u32, scar: bool, grin: bool) -> Vec<Part> {
    let at = |x: f32, y: f32, z: f32| h + Vec3::new(x * r, y * r, z * r);
    let skin_d = lerp(skin, 0x000000, 0.25);
    let mut p = vec![ball(h, r, skin)];
    // Menton lourd.
    p.push(ellipsoid(at(0.0, -0.55, 0.45), Vec3::new(0.7, 0.55, 0.6) * r, skin));
    // (signe, largeur, hauteur, hauteur du centre, couleur de l'iris): l'oeil gauche du personnage est énorme.
    for (sg, w, hh, y, iris) in [(1.0f32, 0.42, 0.46, 0.14, big_eye), (-1.0, 0.25, 0.26, 0.26, 0x3a3030)] {
        let x = sg * 0.38;
        p.push(ellipsoid(at(x, -0.0 + y - 0.12, 0.8), Vec3::new(w, hh, 0.14) * r, 0xe8e4b8));
        p.push(ellipsoid(at(x + sg * 0.03, y - 0.14, 0.92), Vec3::new(w * 0.45, hh * 0.5, 0.08) * r, iris));
        p.push(ellipsoid(at(x + sg * 0.03, y - 0.15, 0.98), Vec3::new(w * 0.2, hh * 0.22, 0.05) * r, DARK));
        // Vaisseaux sanguins.
        p.push(block(at(x - sg * w * 0.5, y - 0.02, 0.94), Vec3::new(0.1, 0.008, 0.01) * r, Mat3::from_rotation_z(0.5 * sg), 0xc03030));
        // Paupière tombante et cerne marqué.
        p.push(ellipsoid(at(x, y + hh * 0.45 - 0.12, 0.88), Vec3::new(w * 1.1, hh * 0.55, 0.14) * r, skin_d));
        p.push(ellipsoid(at(x, y - hh * 0.95 - 0.1, 0.8), Vec3::new(w * 1.05, 0.06, 0.1) * r, 0x4a3a52));
    }
    // Nez bulbeux, narines.
    p.push(ellipsoid(at(0.0, -0.2, 0.98), Vec3::new(0.16, 0.2, 0.15) * r, skin_d));
    for sg in [-1.0f32, 1.0] {
        p.push(ellipsoid(at(sg * 0.06, -0.3, 1.1), Vec3::new(0.035, 0.04, 0.03) * r, DARK));
    }
    // Bouche immense, dents en désordre, langue, fil de suture.
    let mouth_y = -0.52;
    p.push(ellipsoid(at(0.0, mouth_y, 0.84), Vec3::new(0.62, if grin { 0.26 } else { 0.14 }, 0.12) * r, 0x3a0a10));
    if grin {
        p.push(ellipsoid(at(0.05, mouth_y - 0.1, 0.9), Vec3::new(0.26, 0.09, 0.07) * r, 0xc05060));
    }
    for k in 0..9 {
        let x = (k as f32 - 4.0) * 0.12;
        let tall = 0.07 + 0.05 * (((k * 5) % 3) as f32);
        p.push(block(at(x, mouth_y + 0.12 + if grin { 0.06 } else { 0.0 }, 0.93), Vec3::new(0.045, tall, 0.03) * r, Mat3::from_rotation_z(((k % 3) as f32 - 1.0) * 0.12), TOOTH));
        if grin && k % 2 == 0 {
            p.push(block(at(x + 0.05, mouth_y - 0.14, 0.93), Vec3::new(0.04, 0.06, 0.03) * r, Mat3::IDENTITY, TOOTH));
        }
    }
    for k in 0..5 {
        let x = -0.5 + k as f32 * 0.25;
        p.push(block(at(x, mouth_y, 0.97), Vec3::new(0.012, 0.2, 0.012) * r, Mat3::from_rotation_z(0.12), THREAD));
    }
    p.push(block(at(0.0, mouth_y + 0.01, 0.98), Vec3::new(0.6, 0.01, 0.01) * r, Mat3::IDENTITY, THREAD));
    // Gouttes de sang au coin de la bouche.
    p.push(cone(at(0.45, mouth_y - 0.05, 0.85), at(0.47, mouth_y - 0.42, 0.88), 0.05 * r, 0.012 * r, BLOOD));
    if scar {
        p.push(block(at(-0.3, 0.1, 1.0), Vec3::new(0.012, 0.42, 0.012) * r, Mat3::from_rotation_z(0.5), BLOOD_D));
        for k in 0..4 {
            p.push(block(at(-0.3 + (k as f32 - 1.5) * 0.09, 0.1 + (k as f32 - 1.5) * 0.17, 1.0), Vec3::new(0.05, 0.01, 0.01) * r, Mat3::from_rotation_z(-0.9), THREAD));
        }
    }
    p
}

/// Mélange deux couleurs.
fn lerp(a: u32, b: u32, t: f32) -> u32 {
    let (x, y) = (rgb(a), rgb(b));
    let ch = |i: usize| ((x[i] + (y[i] - x[i]) * t).clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    (ch(0) << 16) | (ch(1) << 8) | ch(2)
}

/// Grandes mains et grands pieds, façon dessin animé.
fn big_extremities(j: &Joints, glove: u32, shoe: u32) -> Vec<Part> {
    let mut p = Vec::new();
    for i in 0..2 {
        p.push(shiny(ball(j.hand[i], 0.1, glove), 0.3));
        for k in 0..3 {
            let a = (k as f32 - 1.0) * 0.5;
            p.push(ball(j.hand[i] + Vec3::new(a * 0.07, -0.08, 0.03), 0.032, glove));
        }
        p.push(shiny(ellipsoid(j.ankle[i] + Vec3::new(0.0, -0.01, 0.1), Vec3::new(0.1, 0.07, 0.22), shoe), 0.3));
    }
    p
}

/// Taches de sang sur un vêtement (centre, rayon, nombre de taches), et coulures.
fn splatter(rng: &mut Rng, center: Vec3, spread: Vec3, count: usize) -> Vec<Part> {
    let mut p = Vec::new();
    for _ in 0..count {
        let at = center + Vec3::new(rng.spread(spread.x), rng.spread(spread.y), 0.0);
        p.push(ellipsoid(at, Vec3::new(0.03 + rng.next() * 0.05, 0.03 + rng.next() * 0.06, 0.012), BLOOD));
        if rng.next() < 0.5 {
            p.push(cone(at, at - Vec3::Y * (0.08 + rng.next() * 0.14), 0.012, 0.004, BLOOD));
        }
    }
    p
}

fn knight(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x3a3438, boots: RUST_D, torso: IRON, waist: 0x3a2a30, sleeve: 0x4a4a52, glove: 0x6a6a72 };
    let mut p = body(d, j, &pal, true);
    let mut rng = Rng(11);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    p.extend(big_extremities(j, 0x7a8a60, RUST_D));
    // Cuirasse rouillée éventrée: côtes apparentes, lambeaux de tabard sanglant.
    p.push(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, 0.0), Vec3::new(d.torso_rx * 1.1, d.torso * 0.33, d.torso_rz * 1.1), RUST));
    p.push(ellipsoid(c + Vec3::new(0.0, d.torso * 0.7, d.torso_rz * 0.9), Vec3::new(0.11, 0.13, 0.05), BLOOD_D));
    for k in 0..3 {
        p.push(shiny(ellipsoid(c + Vec3::new(0.0, d.torso * 0.58 + k as f32 * 0.07, d.torso_rz * 1.0), Vec3::new(0.1, 0.014, 0.025), BONE), 0.4));
    }
    for (x, len) in [(-0.1, 0.4), (0.0, 0.5), (0.09, 0.36)] {
        let top = c + Vec3::new(x, 0.12, d.torso_rz + 0.02);
        p.push(panel(top, top + Vec3::new(j.sway * 2.0, -len, 0.04 + j.trail * 0.2), 0.08, 0.01, if x == 0.0 { BLOOD_D } else { 0x3a4a78 }));
    }
    p.extend(splatter(&mut rng, c + Vec3::new(0.0, d.torso * 0.7, d.torso_rz * 1.1), Vec3::new(0.15, 0.12, 0.0), 6));
    // Épaulière hérissée de pointes d'os.
    for k in 0..3 {
        let a = (k as f32 - 1.0) * 0.5;
        p.push(cone(j.shoulder[1] + Vec3::new(0.08, 0.06, 0.0), j.shoulder[1] + Vec3::new(0.2 + a * 0.05, 0.22 + a * 0.1, a * 0.12), 0.05, 0.008, BONE));
    }
    p.push(shiny(ball(j.shoulder[1] + Vec3::new(0.05, 0.04, 0.0), 0.11, RUST), 0.4));
    // Tête: peau verdâtre, seau cabossé sur le crâne, œil globuleux.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0x8aa878, 0xe8d030, true, false));
    p.push(shiny(cone(h + Vec3::Y * 0.62 * r, h + Vec3::Y * 1.22 * r, 0.9 * r, 0.8 * r, RUST), 0.5));
    p.push(cone(h + Vec3::Y * 1.18 * r, h + Vec3::Y * 1.28 * r, 0.8 * r, 0.6 * r, RUST_D));
    p.push(ellipsoid(h + Vec3::new(0.45 * r, 0.95 * r, 0.62 * r), Vec3::new(0.25, 0.2, 0.08) * r, RUST_D));
    for k in 0..5 {
        let a = k as f32 * TAU / 5.0;
        p.push(ball(h + Vec3::new(a.cos() * 0.88 * r, 0.7 * r, a.sin() * 0.88 * r), 0.04 * r, 0x9a9aa0));
    }
    // Épée ébréchée qui dégouline, bouclier-planche orné d'un crâne.
    let rot = held(j, 0, 0.1);
    let hand = j.hand[0];
    p.push(limb(hand - rot * Vec3::Y * 0.07, hand + rot * Vec3::Y * 0.07, 0.03, RUST_D));
    p.push(block(hand + rot * Vec3::Y * 0.1, Vec3::new(0.12, 0.025, 0.03), rot, IRON));
    p.push(shiny(block(hand + rot * Vec3::Y * 0.58, Vec3::new(0.065, 0.46, 0.014), rot, 0xa0a4ac), 0.6));
    for k in 0..3 {
        p.push(block(hand + rot * Vec3::new(0.07, 0.3 + k as f32 * 0.14, 0.0), Vec3::new(0.02, 0.025, 0.016), rot, 0x1a1418));
    }
    p.push(cone(hand + rot * Vec3::new(0.0, 0.9, 0.0), hand + rot * Vec3::new(0.0, 0.66, 0.0), 0.03, 0.008, BLOOD));
    p.push(ellipsoid(hand + rot * Vec3::new(0.0, 0.55, 0.0), Vec3::new(0.07, 0.2, 0.016), BLOOD_D));
    for (a, b, r) in &j.fx.trail {
        p.push(glow(limb(*a, *b, *r, 0xff5040)));
    }
    let center = (j.elbow[1] + j.hand[1]) / 2.0 + Vec3::new(0.06, 0.0, 0.08);
    let face_dir = Vec3::new(0.35, 0.0, 1.0).normalize();
    p.push(shiny(cone(center - face_dir * 0.025, center + face_dir * 0.025, 0.24, 0.24, 0x6a4a30), 0.2));
    p.push(ball(center + face_dir * 0.04, 0.09, BONE));
    for sg in [-1.0f32, 1.0] {
        p.push(ball(center + face_dir * 0.1 + Vec3::new(sg * 0.035, 0.015, 0.0), 0.022, DARK));
    }
    p.push(cone(center + Vec3::new(0.0, -0.2, 0.05), center + Vec3::new(0.0, -0.4, 0.05), 0.03, 0.008, BLOOD));
    p
}

fn sorcerer(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x2a2236, boots: 0x3a2a44, torso: 0x34283e, waist: 0x2a2036, sleeve: 0x3a2a48, glove: 0x8a9ec0 };
    let mut p = body(d, j, &pal, true);
    let mut rng = Rng(23);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    p.extend(big_extremities(j, 0x8a9ec0, 0x2a2036));
    // Robe en lambeaux: jupe effilochée aux pointes déchiquetées, ceinture d'os, manches pendantes.
    p.push(cone(c + Vec3::new(0.0, -0.64, 0.0), c + Vec3::new(0.0, 0.3, 0.0), 0.4, 0.16, 0x3a2848));
    for k in 0..9 {
        let a = k as f32 * TAU / 9.0;
        let (x, z) = (a.cos() * 0.4, a.sin() * 0.4);
        let len = 0.1 + 0.12 * (((k * 7) % 4) as f32 / 3.0);
        p.push(cone(c + Vec3::new(x, -0.62, z), c + Vec3::new(x * 1.08 + j.sway * 0.5, -0.62 - len, z * 1.08 - j.trail * 0.3), 0.07, 0.01, 0x3a2848));
    }
    p.push(cone(c + Vec3::new(0.0, 0.16, 0.0), c + Vec3::new(0.0, 0.22, 0.0), 0.2, 0.19, BONE));
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0;
        p.push(ball(c + Vec3::new(a.cos() * 0.2, 0.2, a.sin() * 0.2), 0.03, BONE));
    }
    p.push(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, 0.02), Vec3::new(d.torso_rx * 1.05, d.torso * 0.3, d.torso_rz * 1.1), 0x4a3858));
    p.extend(splatter(&mut rng, c + Vec3::new(0.0, d.torso * 0.7, d.torso_rz * 1.1), Vec3::new(0.12, 0.1, 0.0), 5));
    for i in 0..2 {
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(cone(j.elbow[i], j.hand[i] - dir * 0.05, 0.05, 0.14, 0x3a2a48));
        for k in 0..3 {
            let off = (k as f32 - 1.0) * 0.07;
            p.push(cone(j.hand[i] - dir * 0.08 + Vec3::new(off, 0.0, 0.0), j.hand[i] - dir * 0.08 + Vec3::new(off * 1.4, -0.14, 0.0), 0.035, 0.006, 0x3a2a48));
        }
    }
    // Tête: peau bleu-gris, cheveux noirs en pagaille avec une mèche violette, grand chapeau tordu orné d'un crâne.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0x8a9ec0, 0xd040a0, false, false));
    // Bouc gris et sale.
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, -0.95 * r, 0.5 * r), Vec3::new(0.3, 0.38, 0.22) * r, 0x8a8a96), 0.3));
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, 0.2 * r, -0.28 * r), Vec3::new(1.12, 1.05, 1.08) * r, 0x14101c), 0.5));
    for k in 0..9 {
        let a = (k as f32 - 4.0) * 0.4;
        let root = h + Vec3::new(a * 0.5 * r, 0.9 * r, -0.4 * r);
        let tip = root + Vec3::new((a * 0.45 + j.sway * 4.0) * r, -1.9 * r - 0.4 * r * (k % 3) as f32, (-0.9 - j.trail * 4.0 - 0.2 * (k % 2) as f32) * r);
        p.push(shiny(cone(root, tip, 0.2 * r, 0.02 * r, if k == 6 { 0x9a30c0 } else { 0x14101c }), 0.6));
    }
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(cone(h + Vec3::new(sg * 0.95 * r, 0.5 * r, 0.1 * r), h + Vec3::new(sg * 1.1 * r, -0.9 * r, 0.5 * r), 0.2 * r, 0.02 * r, 0x14101c), 0.6));
    }
    let brim = h + Vec3::Y * 0.68 * r;
    p.push(shiny(cone(brim, brim + Vec3::Y * 0.12 * r, 1.4 * r, 1.05 * r, 0x1c1428), 0.3));
    p.push(cone(brim + Vec3::Y * 0.1 * r, brim + Vec3::Y * 0.4 * r, 0.8 * r, 0.74 * r, 0x6a1828));
    p.push(shiny(cone(brim + Vec3::Y * 0.12 * r, brim + Vec3::new(0.2 * r, 1.4 * r, 0.0), 0.78 * r, 0.4 * r, 0x1c1428), 0.3));
    p.push(shiny(cone(brim + Vec3::new(0.2 * r, 1.5 * r, 0.0), brim + Vec3::new(-0.35 * r, 2.5 * r, -0.2 * r), 0.45 * r, 0.06 * r, 0x1c1428), 0.3));
    p.push(shiny(ball(brim + Vec3::new(0.0, 0.35 * r, 0.82 * r), 0.17 * r, BONE), 0.5));
    for sg in [-1.0f32, 1.0] {
        p.push(ball(brim + Vec3::new(sg * 0.07 * r, 0.38 * r, 0.96 * r), 0.04 * r, DARK));
    }
    // Bâton: une colonne vertébrale surmontée d'un crâne aux orbites enflammées.
    let rot = held(j, 0, 0.12);
    let hand = j.hand[0];
    for k in 0..9 {
        p.push(shiny(ball(hand + rot * Vec3::Y * (-0.5 + k as f32 * 0.18), 0.04 + 0.012 * (k % 2) as f32, BONE), 0.4));
    }
    p.push(limb(hand - rot * Vec3::Y * 0.5, hand + rot * Vec3::Y * 1.05, 0.02, 0x4a3a34));
    let skull = hand + rot * Vec3::Y * 1.25;
    let s = j.fx.glow;
    p.push(shiny(ball(skull, 0.13, BONE), 0.5));
    p.push(ellipsoid(skull + rot * Vec3::new(0.0, -0.1, 0.03), Vec3::new(0.08, 0.05, 0.08), BONE));
    for sg in [-1.0f32, 1.0] {
        p.push(glow(ball(skull + rot * Vec3::new(sg * 0.05, 0.02, 0.1), 0.04 * s, 0x60ff60)));
    }
    p.push(glow(cone(skull + rot * Vec3::Y * 0.1, skull + rot * Vec3::new(0.0, 0.1 + 0.2 * s, -0.05), 0.07 * s, 0.01, 0x60ff60)));
    if let Some((center, axis, radius)) = j.fx.ring {
        let n = axis.normalize();
        p.push(glow(cone(center - n * 0.012, center + n * 0.012, radius, radius, 0x50e050)));
        p.push(cone(center - n * 0.004, center + n * 0.02, radius * 0.8, radius * 0.8, 0x0c2010));
        let up = if n.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let (u, v) = (n.cross(up).normalize(), n.cross(n.cross(up)).normalize());
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            p.push(glow(ball(center + (u * a.cos() + v * a.sin()) * radius * 0.62 + n * 0.025, 0.03, 0xc0ffc0)));
        }
    }
    p
}

fn ranger(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x3a3028, boots: RUST_D, torso: 0x4a3a2a, waist: 0x2a2018, sleeve: 0x5a4a3a, glove: 0x8a4a38 };
    let mut p = body(d, j, &pal, true);
    let mut rng = Rng(37);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    p.extend(big_extremities(j, 0xb87a68, RUST_D));
    // Pelisse de fourrure sale, côtes peintes, baudrier de dents, ceinture de trophées.
    p.push(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, 0.0), Vec3::new(d.torso_rx * 1.12, d.torso * 0.32, d.torso_rz * 1.12), 0x5a4632));
    for k in 0..4 {
        p.push(block(c + Vec3::new(0.0, d.torso * 0.6 + k as f32 * 0.06, d.torso_rz * 1.1), Vec3::new(0.09, 0.01, 0.01), Mat3::IDENTITY, BONE));
    }
    for k in 0..7 {
        let t = k as f32 / 6.0;
        p.push(cone(c + Vec3::new(-0.15 + t * 0.3, d.torso * 0.92 - t * 0.35, d.torso_rz * 1.12), c + Vec3::new(-0.15 + t * 0.3, d.torso * 0.85 - t * 0.35, d.torso_rz * 1.16), 0.025, 0.006, TOOTH));
    }
    p.extend(splatter(&mut rng, c + Vec3::new(0.0, d.torso * 0.7, d.torso_rz * 1.12), Vec3::new(0.14, 0.12, 0.0), 6));
    for (dx, len, k) in [(-0.16, 0.5, 0.0), (0.0, 0.62, 1.0), (0.15, 0.44, 2.0)] {
        let top = Vec3::new(dx, j.shoulder_y - 0.02, -d.torso_rz - 0.05);
        p.push(panel(top, top + Vec3::new(j.sway * (1.0 + k * 0.4), -len, -0.08 - j.trail * 0.6), 0.14, 0.012, 0x3a2a1c));
    }
    // Épaulière faite d'une mâchoire d'animal.
    let sh = j.shoulder[1] + Vec3::new(0.08, 0.06, 0.0);
    p.push(shiny(limb(sh + Vec3::new(-0.08, 0.0, 0.1), sh + Vec3::new(0.1, 0.0, -0.1), 0.04, BONE), 0.4));
    for k in 0..5 {
        let t = k as f32 / 4.0;
        let at = sh + Vec3::new(-0.08 + 0.18 * t, 0.0, 0.1 - 0.2 * t);
        p.push(cone(at, at + Vec3::new(0.0, -0.1, 0.02), 0.025, 0.004, TOOTH));
    }
    // Tête: capuche à oreilles, bandana qui cache le bas du visage avec un sourire de crocs peints.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0xb87a68, 0xf0a020, false, true));
    p.push(ellipsoid(h + Vec3::new(0.0, 0.3 * r, -0.3 * r), Vec3::new(1.1, 1.02, 1.08) * r, 0x3a3228));
    p.push(ellipsoid(h + Vec3::new(0.0, -0.62 * r, 0.62 * r), Vec3::new(0.72, 0.34, 0.42) * r, 0x6a3a2a));
    for k in 0..5 {
        let x = (k as f32 - 2.0) * 0.28;
        p.push(cone(h + Vec3::new(x * r, -0.55 * r, 1.0 * r), h + Vec3::new(x * r, -0.78 * r, 1.04 * r), 0.07 * r, 0.01 * r, WHITE));
    }
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(cone(h + Vec3::new(sg * 0.65 * r, 0.9 * r, -0.1 * r), h + Vec3::new(sg * 0.95 * r + j.sway * r, 1.7 * r, -0.2 * r), 0.3 * r, 0.04 * r, 0x3a3228), 0.2));
        p.push(cone(h + Vec3::new(sg * 0.65 * r, 0.95 * r, 0.0), h + Vec3::new(sg * 0.9 * r, 1.55 * r, -0.1 * r), 0.16 * r, 0.03 * r, 0xc07a8a));
    }
    // Carquois à tête de mort, arc en côtes avec une corde en boyau.
    let (qa, qb) = (c + Vec3::new(0.1, 0.0, -0.27), c + Vec3::new(0.2, 0.66, -0.3));
    p.push(cone(qa, qb, 0.065, 0.085, 0x2a2018));
    p.push(shiny(ball(qb + Vec3::new(0.0, 0.1, 0.06), 0.08, BONE), 0.4));
    for (dx, color) in [(-0.035, BONE), (0.0, BLOOD), (0.04, BONE)] {
        p.push(block(qb + Vec3::new(dx, 0.2, 0.0), Vec3::new(0.014, 0.06, 0.014), Mat3::IDENTITY, color));
    }
    let hand = j.hand[0];
    let rot = held(j, 0, 0.0);
    let arc = |t: f32| hand + rot * Vec3::new(-0.09 * (1.0 - t * t), t * 0.6, 0.13 * (1.0 - t * t));
    for k in 0..8 {
        let (t0, t1) = (-1.0 + 2.0 * k as f32 / 8.0, -1.0 + 2.0 * (k + 1) as f32 / 8.0);
        p.push(shiny(limb(arc(t0), arc(t1), 0.03, BONE), 0.5));
        p.push(ball(arc(t1), 0.038, BONE_D));
    }
    let string = 0xc07a70;
    match j.fx.string {
        Some(pull) => {
            p.push(limb(arc(-1.0), pull, 0.009, string));
            p.push(limb(pull, arc(1.0), 0.009, string));
        }
        None => p.push(limb(arc(-1.0), arc(1.0), 0.009, string)),
    }
    if let Some((tail, tip)) = j.fx.arrow {
        let dir = (tip - tail).normalize_or(Vec3::Z);
        p.push(shiny(limb(tail, tip, 0.016, BONE), 0.5));
        p.push(cone(tip - dir * 0.02, tip + dir * 0.1, 0.035, 0.004, IRON));
        p.push(glow(cone(tip - dir * 0.02, tip + dir * 0.0, 0.03, 0.03, 0xff4030)));
        p.push(cone(tail, tail + dir * 0.09, 0.03, 0.03, BLOOD_D));
    }
    p
}

const BONE_D: u32 = 0xb8ae94;

fn forgebraise(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x3a3844, boots: 0x2a2018, torso: 0xa8a090, waist: 0x3a3844, sleeve: 0xa8a090, glove: 0x4a4e58 };
    let mut p = body(d, j, &pal, true);
    let mut rng = Rng(53);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    p.extend(big_extremities(j, 0xd8c890, 0x2a2018));
    // Tablier de boucher ensanglanté, bras droit mécanique, bandoulière de saucisses explosives.
    p.push(shiny(block(c + Vec3::new(0.0, 0.26, d.torso_rz - 0.01), Vec3::new(0.24, 0.3, 0.03), Mat3::IDENTITY, 0xcfc8b0), 0.2));
    p.extend(splatter(&mut rng, c + Vec3::new(0.0, 0.26, d.torso_rz + 0.03), Vec3::new(0.2, 0.26, 0.0), 12));
    p.push(shiny(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, 0.0), Vec3::new(d.torso_rx * 1.02, d.torso * 0.3, d.torso_rz * 1.02), 0xa8a090), 0.2));
    p.push(shiny(cone(j.shoulder[0], j.hand[0], 0.1, 0.085, IRON), 0.7));
    p.push(shiny(ball(j.elbow[0], 0.1, IRON), 0.7));
    for k in 0..3 {
        let at = j.shoulder[0] + (j.elbow[0] - j.shoulder[0]) * (0.2 + k as f32 * 0.25);
        p.push(limb(at + Vec3::new(-0.1, 0.0, 0.0), at + Vec3::new(-0.1, -0.12, -0.06), 0.022, 0x8a2020));
    }
    for k in 0..5 {
        let t = k as f32 / 4.0;
        let at = c + Vec3::new(-0.26 + t * 0.52, 0.55 - t * 0.38, d.torso_rz + 0.02);
        p.push(shiny(ellipsoid(at, Vec3::new(0.05, 0.05, 0.1), 0xa83a3a), 0.6));
        p.push(glow(ball(at + Vec3::new(0.0, 0.06, 0.04), 0.014, 0xffd040)));
    }
    p.push(block(c + Vec3::new(0.0, 0.1, -d.torso_rz - 0.12), Vec3::new(0.24, 0.3, 0.1), Mat3::IDENTITY, 0x4a3a2a));
    for x in [-0.12, 0.0, 0.12] {
        p.push(shiny(ball(c + Vec3::new(x, 0.1, -d.torso_rz - 0.24), 0.07, 0x2a2a30), 0.6));
        p.push(glow(ball(c + Vec3::new(x, 0.19, -d.torso_rz - 0.24), 0.016, 0xffd040)));
    }
    // Tête: peau jaunâtre luisante, crâne dégarni, lunettes de soudeur fêlées relevées sur le front, énorme sourire.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0xd8c890, 0x60d040, true, true));
    for k in 0..6 {
        let a = (k as f32 - 2.5) * 0.35;
        p.push(shiny(cone(h + Vec3::new(a * r, 0.95 * r, -0.2 * r), h + Vec3::new(a * 1.5 * r + j.sway * r, 1.7 * r, -0.5 * r), 0.1 * r, 0.015 * r, 0x2a2018), 0.5));
    }
    p.push(shiny(cone(h + Vec3::Y * 0.75 * r, h + Vec3::Y * 0.95 * r, 0.96 * r, 0.9 * r, IRON), 0.6));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(cone(h + Vec3::new(sg * 0.42 * r, 0.85 * r, 0.6 * r), h + Vec3::new(sg * 0.42 * r, 0.85 * r, 0.92 * r), 0.3 * r, 0.3 * r, IRON), 0.7));
        p.push(glow(ball(h + Vec3::new(sg * 0.42 * r, 0.85 * r, 0.95 * r), 0.22 * r, 0x70e0ff)));
    }
    p.push(block(h + Vec3::new(0.42 * r, 0.85 * r, 1.12 * r), Vec3::new(0.01, 0.2, 0.01) * r, Mat3::from_rotation_z(0.6), DARK));
    // Merlin-hachoir: manche de bois, énorme lame de boucher et tête de marteau, tous deux dégoulinants.
    let rot = held(j, 0, 0.25);
    let hand = j.hand[0];
    p.push(limb(hand - rot * Vec3::Y * 0.4, hand + rot * Vec3::Y * 0.55, 0.036, RUST_D));
    let head_c = hand + rot * Vec3::Y * 0.68;
    p.push(shiny(block(head_c, Vec3::new(0.15, 0.12, 0.1), rot, IRON), 0.7));
    p.push(shiny(block(head_c + rot * Vec3::new(0.22, 0.0, 0.0), Vec3::new(0.2, 0.2, 0.012), rot, 0xb8bcc4), 0.8));
    p.push(ball(head_c + rot * Vec3::new(0.3, 0.1, 0.0), 0.03, DARK));
    p.push(ellipsoid(head_c + rot * Vec3::new(0.22, -0.14, 0.0), Vec3::new(0.15, 0.05, 0.016), BLOOD_D));
    for k in 0..3 {
        let x = 0.14 + k as f32 * 0.1;
        p.push(cone(head_c + rot * Vec3::new(x, -0.18, 0.0), head_c + rot * Vec3::new(x, -0.3 - 0.06 * k as f32, 0.0), 0.02, 0.005, BLOOD));
    }
    for (a, b, r) in &j.fx.trail {
        p.push(glow(limb(*a, *b, *r, 0xff5030)));
    }
    if let Some((center, radius)) = j.fx.shock {
        for k in 0..18 {
            let a = k as f32 * TAU / 18.0;
            let at = center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
            p.push(glow(ball(at, 0.05, 0xd02020)));
            if k % 3 == 0 {
                p.push(glow(cone(at, at + Vec3::new(a.cos() * 0.1, 0.34, a.sin() * 0.1), 0.035, 0.006, 0xff4030)));
            }
        }
    }
    p
}

pub fn build(class: Class, pose: Pose) -> Vec<Part> {
    let dims = dims(class);
    let joints = joints(&dims, pose, Some(class));
    match class {
        Class::Knight => knight(&dims, &joints),
        Class::Sorcerer => sorcerer(&dims, &joints),
        Class::Ranger => ranger(&dims, &joints),
        Class::Forgebraise => forgebraise(&dims, &joints),
    }
}
