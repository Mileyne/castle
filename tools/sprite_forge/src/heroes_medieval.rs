//! Les quatre héros version médiévale, inspirés des portraits fournis: allure réaliste (proportions d'adulte, matières
//! de cuir, de drap, de mailles et d'acier), tenues et accessoires repris des images. Le squelette et les poses
//! sont ceux des autres versions: la caméra tourne autour du modèle, l'arme principale reste dans la main droite.

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::heroes::{Class, Dims, Joints, Palette, Pose, body, held, joints};
use crate::render::Part;
use crate::shapes::*;

const STEEL_M: u32 = 0xa4a8b0;
const STEEL_MD: u32 = 0x60646e;
const MAIL: u32 = 0x7a7e86;
const LEATHER_M: u32 = 0x6a4228;
const LEATHER_MD: u32 = 0x3e2a1a;
const CREAM: u32 = 0xd6ccb4;
const LION_RED: u32 = 0x8a1c28;
const BRASS: u32 = 0xb08a3a;

pub(crate) fn dims(class: Class) -> Dims {
    match class {
        Class::Knight => Dims { thigh: 0.46, shin: 0.46, torso: 0.52, hip_x: 0.13, shoulder_x: 0.29, upper: 0.3, fore: 0.3, head_r: 0.2, head_rise: 0.07, leg_r: 0.1, arm_r: 0.075, torso_rx: 0.27, torso_rz: 0.18 },
        Class::Sorcerer => Dims { thigh: 0.46, shin: 0.46, torso: 0.5, hip_x: 0.12, shoulder_x: 0.25, upper: 0.29, fore: 0.29, head_r: 0.2, head_rise: 0.07, leg_r: 0.085, arm_r: 0.065, torso_rx: 0.23, torso_rz: 0.16 },
        Class::Ranger => Dims { thigh: 0.47, shin: 0.47, torso: 0.52, hip_x: 0.12, shoulder_x: 0.27, upper: 0.3, fore: 0.3, head_r: 0.195, head_rise: 0.07, leg_r: 0.09, arm_r: 0.07, torso_rx: 0.25, torso_rz: 0.17 },
        Class::Forgebraise => Dims { thigh: 0.42, shin: 0.42, torso: 0.5, hip_x: 0.14, shoulder_x: 0.31, upper: 0.28, fore: 0.28, head_r: 0.205, head_rise: 0.06, leg_r: 0.105, arm_r: 0.085, torso_rx: 0.31, torso_rz: 0.21 },
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Hair {
    /// Mi-long en bataille, frange sur le front.
    Shaggy,
    /// Boucles serrées sur les côtés et la nuque.
    Curly,
    /// Court, front dégarni.
    Short,
    /// Coupe ras.
    Cropped,
}

/// Les traits d'un visage, relevés sur les photos de référence.
struct FaceSpec {
    skin: u32,
    hair: u32,
    hair_style: Hair,
    beard: Option<u32>,
    /// 0,2 = barbe de trois jours, 1 = barbe fournie.
    beard_len: f32,
    eye: u32,
    eye_size: f32,
    /// 1 = yeux grands ouverts, 0,5 = plissés ou aux paupières lourdes.
    eye_open: f32,
    eye_spacing: f32,
    brow_thick: f32,
    /// > 0: sourcils baissés vers le nez (sérieux); < 0: relevés (surprise, malice).
    brow_tilt: f32,
    brow_arch: f32,
    nose_len: f32,
    nose_width: f32,
    /// 0 = bouche fermée, 1 = immense sourire denté.
    smile: f32,
    mouth_width: f32,
    width: f32,
    jaw: f32,
    ears: f32,
    /// Rides d'expression (pattes d'oie, plis du sourire).
    lines: bool,
    glasses: bool,
}

fn spec(class: Class) -> FaceSpec {
    match class {
        // Chevalier: cheveux châtains foncés mi-longs en bataille, visage étroit, nez long et droit, barbe brune
        // pleine reliée à la moustache, yeux verdâtres plissés par un grand sourire.
        Class::Knight => FaceSpec { skin: 0xd5a58c, hair: 0x3a2e28, hair_style: Hair::Shaggy, beard: Some(0x4e382c), beard_len: 0.75, eye: 0x6e7a5a, eye_size: 0.9, eye_open: 0.5, eye_spacing: 1.0, brow_thick: 1.4, brow_tilt: 0.08, brow_arch: 0.1, nose_len: 1.35, nose_width: 0.9, smile: 1.0, mouth_width: 1.1, width: 0.88, jaw: 0.85, ears: 0.9, lines: true, glasses: false },
        // Ensorceleur: boucles brun foncé, sourcils très arqués, grands yeux sombres écarquillés, gros nez, immense
        // sourire denté, barbe poivre et sel, rides profondes, grandes oreilles.
        Class::Sorcerer => FaceSpec { skin: 0xc98260, hair: 0x2e2230, hair_style: Hair::Curly, beard: Some(0x8e8a84), beard_len: 0.7, eye: 0x3a2418, eye_size: 1.3, eye_open: 1.0, eye_spacing: 1.05, brow_thick: 1.3, brow_tilt: -0.18, brow_arch: 0.9, nose_len: 1.1, nose_width: 1.25, smile: 1.0, mouth_width: 1.35, width: 1.0, jaw: 1.0, ears: 1.35, lines: true, glasses: false },
        // Rôdeur: cheveux courts poivre et sel, front dégarni, lunettes rectangulaires à monture noire, nez large,
        // barbe et moustache poivre et sel, bouche fermée, paupières lourdes.
        Class::Ranger => FaceSpec { skin: 0xcc9a7c, hair: 0x6a5d54, hair_style: Hair::Short, beard: Some(0x7a6a60), beard_len: 0.6, eye: 0x4a3428, eye_size: 0.9, eye_open: 0.55, eye_spacing: 1.05, brow_thick: 1.0, brow_tilt: 0.05, brow_arch: 0.0, nose_len: 1.0, nose_width: 1.35, smile: 0.1, mouth_width: 0.9, width: 1.0, jaw: 1.05, ears: 1.25, lines: false, glasses: true },
        // Forgebraise: jeune, cheveux noirs ras, sourcils droits marqués, yeux vert-gris, nez droit, lèvres fines serrées,
        // barbe de trois jours, mâchoire carrée, expression sérieuse.
        Class::Forgebraise => FaceSpec { skin: 0xd4a88e, hair: 0x2a2220, hair_style: Hair::Cropped, beard: Some(0x6a5248), beard_len: 0.22, eye: 0x6e6a50, eye_size: 0.95, eye_open: 0.8, eye_spacing: 0.95, brow_thick: 1.3, brow_tilt: 0.15, brow_arch: 0.0, nose_len: 1.1, nose_width: 1.0, smile: 0.0, mouth_width: 0.9, width: 0.95, jaw: 1.12, ears: 1.0, lines: false, glasses: false },
    }
}

/// Visage détaillé d'après les photos: forme du crâne et de la mâchoire, yeux (taille, ouverture, paupières, rides),
/// sourcils, nez, bouche et dents, barbe, oreilles, cheveux, lunettes.
fn face(h: Vec3, r: f32, s: &FaceSpec) -> Vec<Part> {
    let at = |x: f32, y: f32, z: f32| h + Vec3::new(x * r, y * r, z * r);
    let skin_d = lerp(s.skin, 0x000000, 0.18);
    let skin_dd = lerp(s.skin, 0x000000, 0.4);
    let brow_c = lerp(s.hair, 0x000000, 0.2);
    let mut p = vec![
        shiny(ellipsoid(h, Vec3::new(s.width, 1.08, 1.0) * r, s.skin), 0.15),
        // Mâchoire: plus ou moins carrée.
        shiny(ellipsoid(at(0.0, -0.5, 0.42), Vec3::new(0.7 * s.jaw * s.width, 0.5, 0.58) * r, s.skin), 0.15),
    ];
    for sg in [-1.0f32, 1.0] {
        let (x, es) = (sg * 0.36 * s.eye_spacing * s.width, s.eye_size);
        let open = s.eye_open;
        // Orbite ombrée, globe oculaire, iris, pupille.
        p.push(ellipsoid(at(x, 0.1, 0.8), Vec3::new(0.2 * es, 0.14 * es, 0.08) * r, skin_d));
        p.push(ellipsoid(at(x, 0.1, 0.87), Vec3::new(0.15 * es, 0.1 * es * open.max(0.45), 0.07) * r, 0xf0e8dc));
        p.push(ball(at(x, 0.1, 0.94), 0.07 * es * r * (0.8 + 0.2 * open), s.eye));
        p.push(ball(at(x, 0.1, 0.99), 0.032 * es * r, 0x140c08));
        // Paupière supérieure (plus épaisse quand l'œil est plissé), poche sous l'œil.
        p.push(ellipsoid(at(x, 0.1 + 0.11 * es * open, 0.9), Vec3::new(0.17 * es, 0.03 + 0.09 * (1.0 - open), 0.07) * r, skin_d));
        p.push(ellipsoid(at(x, 0.1 - 0.11 * es * open, 0.88), Vec3::new(0.16 * es, 0.03, 0.06) * r, skin_d));
        if s.lines {
            // Pattes d'oie au coin externe de l'œil.
            for k in -1..=1 {
                p.push(block(at(x + sg * 0.26 * es, 0.1 + k as f32 * 0.05, 0.78), Vec3::new(0.07, 0.006, 0.01) * r, Mat3::from_rotation_z(k as f32 * 0.35 * sg), skin_dd));
            }
        }
        // Sourcil épais, plus ou moins incliné ou arqué.
        let tilt = sg * s.brow_tilt;
        p.push(shiny(block(at(x * 1.04, 0.3 + 0.03 * s.brow_arch, 0.9), Vec3::new(0.18, 0.026 * s.brow_thick, 0.035) * r, Mat3::from_rotation_z(tilt), brow_c), 0.1));
        if s.brow_arch > 0.3 {
            p.push(shiny(block(at(x * 1.25, 0.34 + 0.06 * s.brow_arch, 0.88), Vec3::new(0.1, 0.024 * s.brow_thick, 0.03) * r, Mat3::from_rotation_z(tilt + sg * 0.35), brow_c), 0.1));
        }
        // Oreille.
        p.push(ellipsoid(at(sg * 1.0 * s.width, -0.05, -0.05), Vec3::new(0.1 * s.ears, 0.22 * s.ears, 0.14 * s.ears) * r, s.skin));
        p.push(ellipsoid(at(sg * 1.02 * s.width, -0.05, -0.03), Vec3::new(0.06 * s.ears, 0.14 * s.ears, 0.09 * s.ears) * r, skin_d));
    }
    // Nez: arête, bout, ailes.
    p.push(ellipsoid(at(0.0, 0.0, 0.97), Vec3::new(0.06 * s.nose_width, 0.3 * s.nose_len, 0.1) * r, s.skin));
    p.push(ellipsoid(at(0.0, -0.2 * s.nose_len, 1.04), Vec3::new(0.11 * s.nose_width, 0.09, 0.1) * r, skin_d));
    for sg in [-1.0f32, 1.0] {
        p.push(ball(at(sg * 0.1 * s.nose_width, -0.22 * s.nose_len, 0.99), 0.055 * r * s.nose_width, skin_d));
    }
    // Bouche: sourire denté plus ou moins large, ou lèvres fermées; joues remontées et plis du sourire.
    if s.smile > 0.4 {
        p.push(ellipsoid(at(0.0, -0.46, 0.88), Vec3::new(0.3 * s.mouth_width, 0.06 + 0.09 * s.smile, 0.06) * r, 0x3a1210));
        p.push(block(at(0.0, -0.41 + 0.02 * (s.smile - 1.0), 0.935), Vec3::new(0.26 * s.mouth_width, 0.035, 0.018) * r, Mat3::IDENTITY, 0xf2ecdc));
        if s.smile > 0.8 {
            p.push(block(at(0.0, -0.53, 0.935), Vec3::new(0.2 * s.mouth_width, 0.028, 0.018) * r, Mat3::IDENTITY, 0xece6d6));
        }
        for sg in [-1.0f32, 1.0] {
            p.push(shiny(ellipsoid(at(sg * 0.5 * s.mouth_width, -0.3, 0.82), Vec3::new(0.15, 0.13, 0.1) * r, lerp(s.skin, 0xffffff, 0.06)), 0.2));
            if s.lines {
                p.push(block(at(sg * 0.43 * s.mouth_width, -0.34, 0.9), Vec3::new(0.012, 0.13, 0.012) * r, Mat3::from_rotation_z(sg * 0.5), skin_dd));
            }
        }
    } else {
        p.push(ellipsoid(at(0.0, -0.46, 0.9), Vec3::new(0.2 * s.mouth_width, 0.034, 0.05) * r, 0x9a5a50));
        p.push(ellipsoid(at(0.0, -0.43, 0.93), Vec3::new(0.18 * s.mouth_width, 0.02, 0.04) * r, 0x7a4038));
    }
    // Barbe, moustache, favoris: fournie ou simple ombre de trois jours.
    if let Some(color) = s.beard {
        let thick = if s.beard_len < 0.4 { 0.97 } else { 1.0 };
        let c = if s.beard_len < 0.4 { lerp(s.skin, color, 0.7) } else { color };
        p.push(shiny(ellipsoid(at(0.0, -0.55 - 0.18 * (s.beard_len - 0.5), 0.5), Vec3::new(0.76 * s.jaw * s.width * thick, 0.5 + 0.28 * s.beard_len, 0.52 * thick) * r, c), 0.1));
        p.push(shiny(ellipsoid(at(0.0, -0.3, 0.95), Vec3::new(0.32 * s.mouth_width, 0.07, 0.09) * r, c), 0.1));
        for sg in [-1.0f32, 1.0] {
            p.push(shiny(ellipsoid(at(sg * 0.85 * s.width, -0.15, 0.35), Vec3::new(0.2, 0.45, 0.32) * r, c), 0.1));
        }
    }
    // Lunettes rectangulaires à large monture noire sur le haut (style « browline »).
    if s.glasses {
        let frame = 0x16161c;
        for sg in [-1.0f32, 1.0] {
            let x = sg * 0.36 * s.eye_spacing * s.width;
            p.push(block(at(x, 0.27, 1.0), Vec3::new(0.27, 0.045, 0.02) * r, Mat3::IDENTITY, frame));
            p.push(block(at(x, -0.1, 1.0), Vec3::new(0.25, 0.012, 0.015) * r, Mat3::IDENTITY, frame));
            for e in [-1.0f32, 1.0] {
                p.push(block(at(x + e * 0.25, 0.08, 1.0), Vec3::new(0.014, 0.19, 0.015) * r, Mat3::IDENTITY, frame));
            }
            p.push(limb(at(sg * 0.64 * s.width, 0.25, 0.9), at(sg * 1.0 * s.width, 0.25, 0.1), 0.012 * r, frame));
        }
        p.push(block(at(0.0, 0.2, 1.0), Vec3::new(0.12, 0.02, 0.02) * r, Mat3::IDENTITY, frame));
    }
    // Cheveux.
    let hair = s.hair;
    match s.hair_style {
        Hair::Shaggy => {
            p.push(shiny(ellipsoid(at(0.0, 0.32, -0.22), Vec3::new(1.1, 1.0, 1.08) * r, hair), 0.3));
            for sg in [-1.0f32, 1.0] {
                p.push(shiny(cone(at(sg * 0.95, 0.4, 0.0), at(sg * 1.02, -0.75, 0.15), 0.28 * r, 0.1 * r, hair), 0.3));
                p.push(shiny(cone(at(sg * 0.6, 0.3, -0.7), at(sg * 0.7, -0.9, -0.55), 0.3 * r, 0.1 * r, hair), 0.3));
            }
            // Frange en mèches qui tombent sur le front, de gauche à droite.
            for k in 0..8 {
                let t = k as f32 / 7.0;
                p.push(shiny(cone(at(-0.8 + t * 1.5, 1.0, 0.2), at(-0.55 + t * 1.4, 0.58 - 0.12 * (1.0 - t), 0.97), 0.2 * r, 0.03 * r, hair), 0.3));
            }
        }
        Hair::Curly => {
            p.push(shiny(ellipsoid(at(0.0, 0.2, -0.28), Vec3::new(1.06, 0.95, 1.05) * r, hair), 0.3));
            for k in 0..18 {
                let a = k as f32 * TAU / 18.0;
                let y = -0.25 + 0.55 * (k % 3) as f32 * 0.5;
                p.push(shiny(ball(at(a.cos() * 1.0 * s.width, y, a.sin() * 0.6 - 0.3), 0.26 * r, hair), 0.3));
            }
        }
        Hair::Short => {
            // Cheveux poivre et sel, front dégarni: la ligne des cheveux est reculée.
            p.push(shiny(ellipsoid(at(0.0, 0.62, -0.18), Vec3::new(1.02, 0.7, 1.0) * r, hair), 0.2));
            for sg in [-1.0f32, 1.0] {
                p.push(shiny(ellipsoid(at(sg * 0.95 * s.width, 0.25, -0.15), Vec3::new(0.13, 0.4, 0.62) * r, hair), 0.2));
            }
        }
        Hair::Cropped => {
            p.push(shiny(ellipsoid(at(0.0, 0.4, -0.1), Vec3::new(1.04, 0.8, 1.02) * r, hair), 0.2));
        }
    }
    p
}

fn lerp(a: u32, b: u32, t: f32) -> u32 {
    let (x, y) = (rgb(a), rgb(b));
    let ch = |i: usize| ((x[i] + (y[i] - x[i]) * t).clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    (ch(0) << 16) | (ch(1) << 8) | ch(2)
}

fn knight(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x3e3830, boots: LEATHER_MD, torso: MAIL, waist: 0x3e3830, sleeve: MAIL, glove: STEEL_MD };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    let (sway, trail) = (j.sway, j.trail);
    // Haubert de mailles, tabard crème à bandes rouges ceint de cuir, lion rouge sur la poitrine.
    p.push(shiny(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, 0.0), Vec3::new(d.torso_rx * 1.05, d.torso * 0.33, d.torso_rz * 1.06), MAIL), 0.3));
    p.push(cone(c + Vec3::new(0.0, -0.22, 0.0), c + Vec3::new(0.0, 0.12, 0.0), 0.3, 0.22, MAIL));
    for (x0, front) in [(0.0f32, true), (0.0, false)] {
        let z = if front { d.torso_rz + 0.02 } else { -d.torso_rz - 0.02 };
        let drift = if front { 0.04 } else { -0.05 };
        let top = c + Vec3::new(x0, d.torso * 0.95, z);
        let bottom = c + Vec3::new(x0 + sway * 0.3, -0.42, z + drift + if front { 0.0 } else { -trail * 0.6 });
        p.push(panel(top, bottom, 0.34, 0.015, CREAM));
        for sg in [-1.0f32, 1.0] {
            p.push(panel(top + Vec3::new(sg * 0.17, 0.0, 0.003), bottom + Vec3::new(sg * 0.17, 0.0, 0.003), 0.045, 0.016, LION_RED));
        }
    }
    // Lion héraldique rouge: corps, tête, queue et pattes.
    let lion = c + Vec3::new(0.0, d.torso * 0.6, d.torso_rz + 0.04);
    p.push(ellipsoid(lion, Vec3::new(0.06, 0.09, 0.012), LION_RED));
    p.push(ball(lion + Vec3::new(0.03, 0.1, 0.0), 0.03, LION_RED));
    p.push(limb(lion + Vec3::new(-0.04, 0.0, 0.0), lion + Vec3::new(-0.09, 0.08, 0.0), 0.01, LION_RED));
    for (dx, dy) in [(-0.04, -0.1), (0.04, -0.1), (0.07, 0.03), (-0.06, 0.05)] {
        p.push(limb(lion + Vec3::new(dx * 0.5, dy * 0.4, 0.0), lion + Vec3::new(dx, dy, 0.0), 0.012, LION_RED));
    }
    // Ceinturons croisés, boucle de laiton.
    p.push(cone(c + Vec3::new(0.0, 0.07, 0.0), c + Vec3::new(0.0, 0.15, 0.0), d.torso_rx * 1.0, d.torso_rx * 0.95, LEATHER_M));
    p.push(shiny(block(c + Vec3::new(0.0, 0.11, d.torso_rx + 0.02), Vec3::new(0.035, 0.03, 0.01), Mat3::IDENTITY, BRASS), 0.8));
    p.push(block(c + Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.2, 0.012, 0.18), Mat3::from_rotation_z(0.3), LEATHER_M));
    // Coiffe de mailles au cou, épaulières d'acier à lames, brassards, gantelets, genouillères et jambières.
    p.push(shiny(cone(Vec3::new(0.0, j.shoulder_y - 0.04, 0.0), Vec3::new(0.0, j.shoulder_y + 0.1, 0.0), 0.2, 0.15, MAIL), 0.3));
    for i in 0..2 {
        let out = if i == 0 { -1.0 } else { 1.0 };
        p.push(shiny(ball(j.shoulder[i] + Vec3::new(out * 0.05, 0.04, 0.0), 0.14, STEEL_M), 0.8));
        p.push(shiny(ellipsoid(j.shoulder[i] + Vec3::new(out * 0.06, -0.06, 0.0), Vec3::new(0.14, 0.07, 0.13), STEEL_M), 0.8));
        p.push(shiny(cone(j.elbow[i], j.hand[i] - (j.hand[i] - j.elbow[i]).normalize() * 0.05, d.arm_r * 1.1, d.arm_r * 0.95, STEEL_MD), 0.7));
        p.push(shiny(ball(j.hand[i], d.arm_r * 1.25, STEEL_MD), 0.6));
        p.push(shiny(cone(j.knee[i], j.ankle[i] + Vec3::Y * 0.03, d.leg_r * 0.95, d.leg_r * 0.75, STEEL_M), 0.8));
        p.push(shiny(ball(j.knee[i] + Vec3::new(0.0, 0.0, 0.07), d.leg_r * 1.05, STEEL_M), 0.8));
    }
    // Tête: les traits de la photo (voir `spec`).
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, &spec(Class::Knight)));
    // Épée longue dans la main droite, grand écu en amande à lion rouge au bras gauche.
    let rot = held(j, 0, 0.1);
    let hand = j.hand[0];
    p.push(limb(hand - rot * Vec3::Y * 0.08, hand + rot * Vec3::Y * 0.07, 0.025, LEATHER_MD));
    p.push(shiny(block(hand + rot * Vec3::Y * 0.1, Vec3::new(0.12, 0.02, 0.025), rot, STEEL_MD), 0.7));
    p.push(shiny(block(hand + rot * Vec3::Y * 0.6, Vec3::new(0.034, 0.5, 0.01), rot, 0xc4c8d0), 0.9));
    p.push(shiny(cone(hand + rot * Vec3::Y * 1.08, hand + rot * Vec3::Y * 1.22, 0.034, 0.004, 0xc4c8d0), 0.9));
    p.push(shiny(ball(hand - rot * Vec3::Y * 0.1, 0.034, BRASS), 0.8));
    for (a, b, r) in &j.fx.trail {
        p.push(glow(limb(*a, *b, *r, 0xd8e4f4)));
    }
    let center = (j.elbow[1] + j.hand[1]) / 2.0 + Vec3::new(0.08, 0.06, 0.09);
    let n = Vec3::new(0.3, 0.0, 1.0).normalize();
    let tilt = Mat3::from_rotation_y(0.3);
    p.push(shiny(ellipsoid(center - n * 0.02, Vec3::new(0.25, 0.4, 0.035), STEEL_MD), 0.8));
    p.push(shiny(ellipsoid(center, Vec3::new(0.225, 0.37, 0.035), CREAM), 0.2));
    p.push(shiny(ellipsoid(center + tilt * Vec3::new(0.0, 0.08, 0.034), Vec3::new(0.1, 0.14, 0.012), LION_RED), 0.2));
    p.push(ball(center + tilt * Vec3::new(0.05, 0.22, 0.036), 0.04, LION_RED));
    p
}

fn sorcerer(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x2a2038, boots: 0x2a1c14, torso: 0x3a2860, waist: 0x2a2038, sleeve: 0x4a2a70, glove: 0xe0b894 };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    let (sway, trail) = (j.sway, j.trail);
    let purple = 0x4a2a70;
    let gold = 0xc09a40;
    // Longue robe ouverte sur une tunique violet sombre; larges galons brodés d'or, lunes et étoiles.
    p.push(shiny(cone(Vec3::new(0.0, 0.1, 0.0), c + Vec3::new(0.0, 0.42, 0.0), 0.38, 0.23, purple), 0.1));
    p.push(shiny(cone(Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.0, 0.18, 0.0), 0.385, 0.375, gold), 0.6));
    for sg in [-1.0f32, 1.0] {
        p.push(panel(c + Vec3::new(sg * 0.08, 0.4, 0.22), Vec3::new(sg * 0.12, 0.14, 0.38), 0.07, 0.012, gold));
    }
    for k in 0..8 {
        let a = -1.2 + k as f32 * 0.3;
        p.push(glow(ball(Vec3::new(-0.16 + a.cos() * 0.08, 0.5 + a.sin() * 0.08, 0.33), 0.014, 0xe0c870)));
    }
    for (x, y) in [(0.12, 0.45), (0.18, 0.28), (-0.05, 0.28)] {
        p.push(glow(ball(Vec3::new(x, y, 0.35), 0.016, 0xe0c870)));
    }
    // Ceinture de cuir large, besace, pendentif sphérique en laiton, fioles, bandoulière croisée.
    p.push(cone(c + Vec3::new(0.0, 0.05, 0.0), c + Vec3::new(0.0, 0.15, 0.0), d.torso_rx * 0.98, d.torso_rx * 0.93, LEATHER_M));
    p.push(shiny(ball(c + Vec3::new(0.0, 0.0, d.torso_rx + 0.08), 0.07, BRASS), 0.9));
    p.push(limb(c + Vec3::new(0.0, 0.1, d.torso_rx + 0.02), c + Vec3::new(0.0, 0.0, d.torso_rx + 0.08), 0.008, BRASS));
    p.push(block(c + Vec3::new(0.24, 0.0, 0.1), Vec3::new(0.07, 0.09, 0.05), Mat3::from_rotation_z(0.1), LEATHER_MD));
    p.push(shiny(ball(c + Vec3::new(-0.2, -0.02, 0.14), 0.035, 0xa060e0), 0.9));
    p.push(block(c + Vec3::new(0.0, d.torso * 0.72, d.torso_rz * 1.0), Vec3::new(0.02, 0.27, 0.012), Mat3::from_rotation_z(0.6), LEATHER_M));
    // Foulard, cape brodée au dos.
    p.push(cone(Vec3::new(0.0, j.shoulder_y - 0.02, 0.0), Vec3::new(0.0, j.shoulder_y + 0.09, 0.0), 0.2, 0.15, 0x3a2a5a));
    for (x, len, k) in [(-0.2, 0.85, 0.0), (0.0, 0.95, 1.0), (0.2, 0.8, 2.0)] {
        let top = Vec3::new(x, j.shoulder_y - 0.03, -d.torso_rz - 0.04);
        p.push(panel(top, top + Vec3::new(sway * (1.0 + k * 0.4), -len, -0.1 - trail * 0.8), 0.2, 0.012, if k == 1.0 { 0x38205a } else { purple }));
    }
    for i in 0..2 {
        // Manches évasées bordées d'or, bracelet de cuir.
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(cone(j.elbow[i], j.hand[i] - dir * 0.04, 0.065, 0.12, 0x4a2a70));
        p.push(cone(j.hand[i] - dir * 0.05, j.hand[i] - dir * 0.03, 0.122, 0.122, gold));
        p.push(cone(j.ankle[i] + Vec3::Y * 0.2, j.knee[i] - Vec3::Y * 0.02, d.leg_r * 0.95, d.leg_r * 0.9, 0x2a1c14));
    }
    // Tête: les traits de la photo (voir `spec`).
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, &spec(Class::Sorcerer)));
    // Chapeau violet usé à pointe retombante, bande de cuir, broche solaire dorée, plume.
    let brim = h + Vec3::Y * 0.55 * r;
    let hat = 0x4a2a6a;
    p.push(shiny(cone(brim, brim + Vec3::Y * 0.12 * r, 1.75 * r, 1.3 * r, hat), 0.1));
    p.push(cone(brim + Vec3::Y * 0.1 * r, brim + Vec3::Y * 0.5 * r, 1.0 * r, 0.92 * r, LEATHER_MD));
    p.push(shiny(cone(brim + Vec3::Y * 0.12 * r, brim + Vec3::new(0.3 * r, 1.6 * r, 0.0), 0.92 * r, 0.44 * r, hat), 0.1));
    p.push(shiny(cone(brim + Vec3::new(0.3 * r, 1.6 * r, 0.0), brim + Vec3::new(1.1 * r, 2.3 * r, -0.2 * r), 0.44 * r, 0.15 * r, hat), 0.1));
    p.push(shiny(cone(brim + Vec3::new(1.1 * r, 2.3 * r, -0.2 * r), brim + Vec3::new(1.7 * r, 1.9 * r, -0.5 * r), 0.15 * r, 0.03 * r, hat), 0.1));
    p.push(shiny(ball(brim + Vec3::new(0.0, 0.35 * r, 0.98 * r), 0.2 * r, gold), 0.9));
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        p.push(shiny(cone(brim + Vec3::new(0.0, 0.35 * r, 1.02 * r), brim + Vec3::new(a.cos() * 0.36 * r, 0.35 * r + a.sin() * 0.36 * r, 1.02 * r), 0.05 * r, 0.01 * r, gold), 0.8));
    }
    p.push(cone(brim + Vec3::new(0.85 * r, 0.4 * r, 0.6 * r), brim + Vec3::new(1.6 * r, 1.1 * r, 0.1 * r), 0.1 * r, 0.01 * r, 0x2a2a3a));
    // Bâton torsadé sculpté, couronné d'un anneau qui retient un orbe violet; breloques.
    let rot = held(j, 0, 0.12);
    let hand = j.hand[0];
    for k in 0..12 {
        let t = k as f32 / 11.0;
        let twist = (t * TAU * 2.5).sin() * 0.02;
        p.push(shiny(ball(hand + rot * Vec3::new(twist, -0.55 + t * 1.65, twist * 0.5), 0.035, 0x6a4628), 0.3));
    }
    p.push(limb(hand - rot * Vec3::Y * 0.55, hand + rot * Vec3::Y * 1.1, 0.024, 0x4a3018));
    let orb = hand + rot * Vec3::Y * 1.32;
    for k in 0..10 {
        let a = k as f32 * TAU / 10.0;
        p.push(shiny(ball(orb + rot * Vec3::new(a.cos() * 0.17, a.sin() * 0.17, 0.0), 0.028, 0x6a4628), 0.3));
    }
    p.push(glow(ball(orb, 0.1 * j.fx.glow, 0xa050ff)));
    p.push(shiny(ball(hand + rot * Vec3::new(0.03, 0.7, 0.03), 0.035, gold), 0.9));
    if let Some((center, axis, radius)) = j.fx.ring {
        let n = axis.normalize();
        p.push(glow(cone(center - n * 0.012, center + n * 0.012, radius, radius, 0xa050ff)));
        p.push(cone(center - n * 0.004, center + n * 0.02, radius * 0.8, radius * 0.8, 0x20103a));
        let up = if n.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let (u, v) = (n.cross(up).normalize(), n.cross(n.cross(up)).normalize());
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            p.push(glow(ball(center + (u * a.cos() + v * a.sin()) * radius * 0.62 + n * 0.025, 0.03, 0xe0c8ff)));
        }
        p.push(glow(ball(center + n * 0.03, 0.05 * j.fx.glow, 0xffffff)));
    }
    p
}

fn ranger(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x4a3a28, boots: 0x4a3220, torso: 0x5a6a30, waist: 0x4a3a28, sleeve: 0x5a6a30, glove: 0x4a3220 };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    let (sway, trail) = (j.sway, j.trail);
    let cloak = 0x4a5a28;
    // Tunique verte à bords effilochés, ceintures de cuir croisées, besace, dague au fourreau.
    p.push(cone(c + Vec3::new(0.0, -0.26, 0.0), c + Vec3::new(0.0, 0.22, 0.0), 0.3, 0.24, 0x5a6a30));
    for k in 0..7 {
        let a = k as f32 * TAU / 7.0;
        p.push(cone(c + Vec3::new(a.cos() * 0.29, -0.26, a.sin() * 0.29), c + Vec3::new(a.cos() * 0.3, -0.34 - 0.04 * (k % 3) as f32, a.sin() * 0.3), 0.045, 0.01, 0x5a6a30));
    }
    p.push(cone(c + Vec3::new(0.0, 0.06, 0.0), c + Vec3::new(0.0, 0.16, 0.0), d.torso_rx * 1.0, d.torso_rx * 0.95, LEATHER_M));
    p.push(cone(c + Vec3::new(0.0, -0.02, 0.0), c + Vec3::new(0.0, 0.05, 0.0), d.torso_rx * 1.0, d.torso_rx * 0.97, LEATHER_MD));
    p.push(shiny(block(c + Vec3::new(0.0, 0.1, d.torso_rx + 0.02), Vec3::new(0.035, 0.03, 0.01), Mat3::IDENTITY, BRASS), 0.8));
    p.push(block(c + Vec3::new(-0.24, 0.02, 0.1), Vec3::new(0.07, 0.08, 0.05), Mat3::from_rotation_z(-0.1), LEATHER_MD));
    p.push(limb(c + Vec3::new(0.14, 0.05, 0.2), c + Vec3::new(0.2, -0.3, 0.18), 0.03, LEATHER_MD));
    p.push(shiny(ball(c + Vec3::new(0.13, 0.08, 0.21), 0.035, BRASS), 0.8));
    p.push(block(c + Vec3::new(0.0, d.torso * 0.72, d.torso_rz * 1.0), Vec3::new(0.02, 0.28, 0.012), Mat3::from_rotation_z(0.55), LEATHER_M));
    p.push(block(c + Vec3::new(0.0, d.torso * 0.72, d.torso_rz * 1.0), Vec3::new(0.02, 0.28, 0.012), Mat3::from_rotation_z(-0.55), LEATHER_MD));
    // Cape à capuche rabattue, déchirée au bas, broche en feuille de chêne.
    p.push(ellipsoid(Vec3::new(0.0, j.shoulder_y + 0.0, -0.12), Vec3::new(0.29, 0.13, 0.2), cloak));
    p.push(cone(Vec3::new(0.0, j.shoulder_y - 0.02, 0.0), Vec3::new(0.0, j.shoulder_y + 0.1, 0.0), 0.2, 0.16, cloak));
    for (dx, len, k) in [(-0.22, 0.9, 0.0), (-0.08, 1.1, 1.0), (0.08, 0.95, 2.0), (0.22, 1.05, 3.0)] {
        let top = Vec3::new(dx, j.shoulder_y - 0.02, -d.torso_rz - 0.05);
        p.push(panel(top, top + Vec3::new(sway * (1.0 + k * 0.3), -len, -0.1 - trail * 0.8), 0.17, 0.012, if k % 2.0 == 0.0 { cloak } else { 0x3a4a20 }));
    }
    p.push(shiny(ellipsoid(Vec3::new(0.14, j.shoulder_y - 0.02, d.torso_rz + 0.04), Vec3::new(0.05, 0.065, 0.012), 0xb8944a), 0.6));
    // Brassards de cuir gravés de feuilles, gants sans doigts.
    for i in 0..2 {
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(shiny(cone(j.hand[i] - dir * 0.2, j.hand[i] - dir * 0.02, d.arm_r * 1.15, d.arm_r * 1.0, 0x6a4020), 0.3));
        for k in 0..3 {
            p.push(ellipsoid(j.hand[i] - dir * (0.06 + 0.05 * k as f32) + Vec3::new(0.0, 0.0, d.arm_r), Vec3::new(0.025, 0.012, 0.008), 0x8a5a30));
        }
        // Hautes bottes lacées à revers.
        p.push(shiny(cone(j.ankle[i] + Vec3::Y * 0.02, j.knee[i] - Vec3::Y * 0.02, d.leg_r * 0.95, d.leg_r * 0.92, 0x4a3220), 0.2));
        p.push(shiny(cone(j.knee[i] - Vec3::Y * 0.04, j.knee[i] + Vec3::Y * 0.06, d.leg_r * 1.05, d.leg_r * 1.1, 0x5a3e28), 0.2));
    }
    // Tête: les traits de la photo (voir `spec`).
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, &spec(Class::Ranger)));
    // Carquois dans le dos, plumes crème et flèches.
    let (qa, qb) = (c + Vec3::new(0.12, 0.0, -0.27), c + Vec3::new(0.2, 0.7, -0.3));
    p.push(shiny(cone(qa, qb, 0.065, 0.085, 0x6a4020), 0.3));
    p.push(cone(qb - Vec3::Y * 0.02, qb + Vec3::Y * 0.02, 0.09, 0.09, 0x3e2a1a));
    for (dx, dz) in [(-0.04, 0.0), (0.0, 0.03), (0.04, 0.0), (0.0, -0.03), (0.02, 0.02)] {
        let base = qb + Vec3::new(dx, 0.02, dz);
        p.push(limb(base, base + Vec3::new(dx * 0.6, 0.2, dz), 0.008, 0xc8a870));
        p.push(shiny(block(base + Vec3::new(dx * 0.6, 0.26, dz), Vec3::new(0.012, 0.05, 0.03), Mat3::IDENTITY, 0xf0eadc), 0.1));
    }
    // Grand arc recourbé en bois sombre dans la main droite, corde fine.
    let hand = j.hand[0];
    let rot = held(j, 0, 0.0);
    let arc = |t: f32| hand + rot * Vec3::new(-0.08 * (1.0 - t * t), t * 0.62, 0.15 * (1.0 - t * t) - 0.08 * (t * t * t).abs());
    for k in 0..10 {
        let (t0, t1) = (-1.0 + 2.0 * k as f32 / 10.0, -1.0 + 2.0 * (k + 1) as f32 / 10.0);
        p.push(shiny(limb(arc(t0), arc(t1), 0.03 - 0.008 * (t0.abs()), 0x5a3418), 0.4));
    }
    let string = 0xe8e0c8;
    match j.fx.string {
        Some(pull) => {
            p.push(limb(arc(-1.0), pull, 0.007, string));
            p.push(limb(pull, arc(1.0), 0.007, string));
        }
        None => p.push(limb(arc(-1.0), arc(1.0), 0.007, string)),
    }
    if let Some((tail, tip)) = j.fx.arrow {
        let dir = (tip - tail).normalize_or(Vec3::Z);
        p.push(shiny(limb(tail, tip, 0.012, 0xc8a870), 0.4));
        p.push(shiny(cone(tip - dir * 0.02, tip + dir * 0.09, 0.03, 0.004, 0xb8bcc4), 0.9));
        p.push(shiny(block(tail + dir * 0.04, Vec3::new(0.012, 0.04, 0.05), Mat3::IDENTITY, 0xf0eadc), 0.1));
    }
    p
}

fn forgebraise(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x3a2a1c, boots: 0x4a3020, torso: 0x5a4a30, waist: 0x3a2a1c, sleeve: 0x5a4a30, glove: 0x4a3220 };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    // Chemise brune aux manches retroussées, tablier de cuir patiné à bretelles, large ceinture à boucle d'enclume.
    p.push(shiny(block(c + Vec3::new(0.0, 0.0, d.torso_rz - 0.005), Vec3::new(0.25, 0.3, 0.03), Mat3::IDENTITY, 0x6a3a22), 0.3));
    p.push(cone(c + Vec3::new(0.0, -0.34, 0.0), c + Vec3::new(0.0, -0.1, 0.0), 0.28, 0.25, 0x6a3a22));
    for sg in [-1.0f32, 1.0] {
        p.push(panel(c + Vec3::new(sg * 0.17, 0.5, d.torso_rz - 0.02), c + Vec3::new(sg * 0.2, 0.9, 0.0), 0.07, 0.02, LEATHER_MD));
        p.push(shiny(block(c + Vec3::new(sg * 0.17, 0.56, d.torso_rz + 0.01), Vec3::new(0.03, 0.025, 0.01), Mat3::IDENTITY, BRASS), 0.8));
    }
    p.push(cone(c + Vec3::new(0.0, 0.05, 0.0), c + Vec3::new(0.0, 0.17, 0.0), d.torso_rx * 1.0, d.torso_rx * 0.96, LEATHER_M));
    p.push(shiny(cone(c + Vec3::new(0.0, 0.11, d.torso_rx + 0.01), c + Vec3::new(0.0, 0.11, d.torso_rx + 0.03), 0.07, 0.07, BRASS), 0.9));
    p.push(block(c + Vec3::new(0.0, 0.11, d.torso_rx + 0.04), Vec3::new(0.04, 0.02, 0.008), Mat3::IDENTITY, 0x2a2a30));
    // Tenailles suspendues à la ceinture, bourse, mailles aux avant-bras.
    for (x, dx) in [(-0.14, -0.02), (-0.09, 0.02)] {
        p.push(shiny(limb(c + Vec3::new(x, 0.05, d.torso_rx + 0.03), c + Vec3::new(x + dx, -0.35, d.torso_rx + 0.04), 0.014, 0x3a3a42), 0.5));
    }
    p.push(shiny(cone(c + Vec3::new(-0.2, 0.03, d.torso_rx + 0.03), c + Vec3::new(-0.2, 0.06, d.torso_rx + 0.03), 0.05, 0.05, 0x3a3a42), 0.5));
    for i in 0..2 {
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(shiny(cone(j.elbow[i], j.hand[i] - dir * 0.14, d.arm_r * 0.95, d.arm_r * 0.9, MAIL), 0.2));
        // Gros gants de cuir, hautes bottes à sangles.
        p.push(shiny(cone(j.hand[i] - dir * 0.14, j.hand[i] + dir * 0.02, d.arm_r * 1.05, d.arm_r * 1.2, 0x4a3220), 0.3));
        p.push(shiny(ball(j.hand[i], d.arm_r * 1.4, 0x4a3220), 0.3));
        p.push(shiny(cone(j.ankle[i] + Vec3::Y * 0.02, j.knee[i], d.leg_r * 1.0, d.leg_r * 0.98, 0x4a3020), 0.3));
        p.push(block(j.ankle[i] + Vec3::new(0.0, 0.18, d.leg_r * 0.9), Vec3::new(d.leg_r * 0.7, 0.02, 0.012), Mat3::IDENTITY, LEATHER_MD));
    }
    // Casque de fer riveté à nasal et jugulaires, visage sérieux à barbe de trois jours.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, &spec(Class::Forgebraise)));
    // Calotte qui s'arrête au front: les yeux restent dégagés.
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, 0.95 * r, -0.03 * r), Vec3::new(1.1, 0.62, 1.1) * r, 0x6a6e78), 0.7));
    p.push(shiny(cone(h + Vec3::Y * 0.3 * r, h + Vec3::Y * 0.44 * r, 1.1 * r, 1.1 * r, 0x8a8e98), 0.8));
    p.push(shiny(block(h + Vec3::new(0.0, -0.05, 1.0 * r), Vec3::new(0.06, 0.4, 0.04) * r, Mat3::IDENTITY, 0x8a8e98), 0.8));
    p.push(shiny(block(h + Vec3::new(0.0, 1.5 * r, 0.0), Vec3::new(0.05, 0.05, 1.0) * r, Mat3::IDENTITY, 0x8a8e98), 0.8));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(ellipsoid(h + Vec3::new(sg * 1.0 * r, -0.2 * r, 0.1 * r), Vec3::new(0.08, 0.5, 0.45) * r, 0x7a7e88), 0.7));
        for k in 0..3 {
            p.push(shiny(ball(h + Vec3::new(sg * 1.06 * r, 0.1 * r - 0.28 * r * k as f32, 0.3 * r), 0.035 * r, 0xb0b4bc), 0.9));
        }
    }
    // Énorme masse de forge à tête de fer (marque d'enclume), manche de bois.
    let rot = held(j, 0, 0.25);
    let hand = j.hand[0];
    p.push(shiny(limb(hand - rot * Vec3::Y * 0.4, hand + rot * Vec3::Y * 0.6, 0.036, 0x5a3a20), 0.2));
    p.push(shiny(ball(hand - rot * Vec3::Y * 0.42, 0.05, BRASS), 0.8));
    let head_c = hand + rot * Vec3::Y * 0.72;
    p.push(shiny(block(head_c, Vec3::new(0.2, 0.13, 0.13), rot, 0x5a5e68), 0.6));
    p.push(shiny(block(head_c, Vec3::new(0.21, 0.11, 0.11), rot, 0x4a4e58), 0.5));
    p.push(shiny(block(head_c + rot * Vec3::new(0.0, 0.0, 0.135), Vec3::new(0.07, 0.05, 0.008), rot, 0x2a2a30), 0.5));
    for (a, b, r) in &j.fx.trail {
        p.push(glow(limb(*a, *b, *r, 0xffa040)));
    }
    if let Some((center, radius)) = j.fx.shock {
        for k in 0..18 {
            let a = k as f32 * TAU / 18.0;
            let at = center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
            p.push(glow(ball(at, 0.05, 0xff9a30)));
            if k % 3 == 0 {
                p.push(glow(cone(at, at + Vec3::new(a.cos() * 0.08, 0.28, a.sin() * 0.08), 0.03, 0.006, 0xffe080)));
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
