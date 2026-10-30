//! Les tuiles du décor: sol, dessus de mur et face de mur, dans le style des six régions du château.
//!
//! Une tuile fait 1,2 m de côté (48 px). Le sol et le dessus des murs sont vus d'en haut; la face
//! d'un mur est vue de face, éclairée comme les personnages.

use glam::{Mat3, Vec3};

use crate::render::Part;
use crate::shapes::*;

/// Dimensions d'une ligne de la planche (une ligne par région), en tuiles.
///
/// Le sol et les faces de mur sont dessinés en grands motifs qui s'étendent sur plusieurs tuiles (des dalles
/// et des blocs irréguliers, dont les joints traversent les bordures de tuiles), pour éviter l'effet carrelage.
pub const MACRO: usize = 4;
pub const FLOOR_PATTERNS: usize = 3;
pub const FACE_PATTERNS: usize = 2;
pub const TOP_PATTERNS: usize = 2;
pub const DECOR_FACES: usize = 2;
/// Colonnes: motifs de sol (3 de 4x4 tuiles), dessus de mur (2 de 4x4), motifs de face (2 de 4 tuiles), faces décorées.
pub const FLOOR_COLUMNS: usize = FLOOR_PATTERNS * MACRO * MACRO;
pub const TOP_START: usize = FLOOR_COLUMNS;
pub const FACE_START: usize = TOP_START + TOP_PATTERNS * MACRO * MACRO;
pub const DECOR_START: usize = FACE_START + FACE_PATTERNS * MACRO;
pub const COLUMNS: usize = DECOR_START + DECOR_FACES;
pub const REGIONS: usize = 9;

/// Demi-côté d'une tuile, en mètres du modèle.
const H: f32 = 0.6;

pub(crate) struct Style {
    pub(crate) slab: [u32; 3],
    pub(crate) mortar: u32,
    pub(crate) wall: [u32; 3],
    pub(crate) accent: u32,
    pub(crate) glow: u32,
}

fn style(region: usize, gore: bool) -> Style {
    if gore {
        return style_gore(region);
    }
    // Ambiance de donjon: les pierres du mode manga sont assombries d'un cran (pas les décors extérieurs).
    let base = style_manga(region);
    if region >= 6 {
        return base;
    }
    Style {
        slab: base.slab.map(|c| darker(c, 0.82)),
        wall: base.wall.map(|c| darker(c, 0.85)),
        ..base
    }
}

fn style_manga(region: usize) -> Style {
    match region {
        0 => Style { slab: [0x7c869c, 0x8a94ac, 0x707a92], mortar: 0x1c2028, wall: [0x2a3046, 0x343a52, 0x222638], accent: 0xe0dcc8, glow: 0x70a0ff },
        1 => Style { slab: [0x74966e, 0x80a478, 0x688a62], mortar: 0x182418, wall: [0x2e4034, 0x384c3e, 0x26342c], accent: 0x4ca850, glow: 0xff9ac0 },
        2 => Style { slab: [0xb07844, 0xa06a38, 0xc08850], mortar: 0x24140a, wall: [0x4a2e1c, 0x3c2416, 0x573824], accent: 0xc02840, glow: 0xffd060 },
        3 => Style { slab: [0x5a5464, 0x665f72, 0x4e4858], mortar: 0x100c10, wall: [0x3a2a2c, 0x2c1e20, 0x4a3636], accent: 0xff6a20, glow: 0xff8a30 },
        4 => Style { slab: [0x5a60b0, 0x6a70c4, 0x4e549c], mortar: 0x14163a, wall: [0x1c2056, 0x262a6a, 0x161a44], accent: 0xf6c93a, glow: 0x80b0ff },
        5 => Style { slab: [0xe8e0d0, 0xd8c8a8, 0xc8b890], mortar: 0x7a6a40, wall: [0x5a1830, 0x4a1226, 0x6c2640], accent: 0xf6c93a, glow: 0xff5070 },
        // Forêt: herbe et sous-bois; murs de grands arbres.
        6 => Style { slab: [0x5a8a3c, 0x4e7c34, 0x6a9a44], mortar: 0x2a3a1a, wall: [0x2a5a2a, 0x1e4a24, 0x34681e], accent: 0xf0d040, glow: 0xc8f080 },
        // Marais: vase verdâtre; murs d'eau noire.
        7 => Style { slab: [0x566a3c, 0x4a5e34, 0x627444], mortar: 0x1c241a, wall: [0x1e3a36, 0x28483f, 0x162e2c], accent: 0x8ad0b0, glow: 0x80ffc0 },
        // Abords du château: terre battue; murs de rochers.
        _ => Style { slab: [0x9a8a68, 0x8c7e5e, 0xa89874], mortar: 0x2a2218, wall: [0x6a665c, 0x5a564e, 0x78746a], accent: 0xb0a890, glow: 0xffd890 },
    }
}

/// Mélange deux couleurs: `t` = 0 donne `a`, 1 donne `b`.
pub(crate) fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let (x, y) = (rgb(a), rgb(b));
    let ch = |i: usize| ((x[i] + (y[i] - x[i]) * t).clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    (ch(0) << 16) | (ch(1) << 8) | ch(2)
}

/// Assombrit une couleur: `f` = 1 ne change rien, 0 donne du noir.
pub(crate) fn darker(c: u32, f: f32) -> u32 {
    lerp_color(c, 0x000000, 1.0 - f)
}

/// Dalle de sol taillée à la main, vue d'en haut: un socle, une surface faite de petites facettes inégales et de
/// teintes variées (arêtes chanfreinées), des piqûres, une tache de crasse, des arêtes usées plus claires.
/// `rough` va de 0 (marbre poli) à 1 (pierre brute).
fn rough_slab(p: &mut Vec<Part>, rng: &mut Rng, center: Vec3, half: Vec3, tone: u32, mortar: u32, rough: f32) {
    // Le socle reste plat: incliné, il percerait à travers les facettes de la surface.
    p.push(shiny(block(center, half, Mat3::IDENTITY, darker(tone, 0.78)), 0.1));
    let inner = Vec3::new(half.x - 0.05, half.y, half.z - 0.05);
    let cell = 0.1 + 0.1 * (1.0 - rough);
    let (nx, nz) = (((inner.x * 2.0) / cell).ceil().max(1.0) as i32, ((inner.z * 2.0) / cell).ceil().max(1.0) as i32);
    let (sx, sz) = (inner.x * 2.0 / nx as f32, inner.z * 2.0 / nz as f32);
    let top = center.y + half.y;
    for ix in 0..nx {
        for iz in 0..nz {
            let (cx, cz) = (center.x - inner.x + (ix as f32 + 0.5) * sx, center.z - inner.z + (iz as f32 + 0.5) * sz);
            let lift = 0.012 + rough * rng.next() * 0.006;
            let shade = if rng.next() < 0.5 { mortar } else { 0xffffff };
            let facet = lerp_color(tone, shade, rng.next() * (0.02 + 0.045 * rough));
            p.push(shiny(
                block(
                    Vec3::new(cx, top + lift / 2.0, cz),
                    Vec3::new(sx / 2.0 * 0.985, lift / 2.0, sz / 2.0 * 0.985),
                    Mat3::from_rotation_x(rng.spread(0.015 * rough)) * Mat3::from_rotation_z(rng.spread(0.015 * rough)),
                    facet,
                ),
                0.2 * (1.0 - 0.5 * rough),
            ));
        }
    }
    let surface = top + 0.03;
    // Arêtes usées: filets plus clairs sur les bords exposés à la lumière (haut et gauche).
    let edge = lerp_color(tone, 0xffffff, 0.22);
    p.push(block(Vec3::new(center.x, surface, center.z - inner.z + 0.012), Vec3::new(inner.x, 0.004, 0.012), Mat3::IDENTITY, edge));
    p.push(block(Vec3::new(center.x - inner.x + 0.012, surface, center.z), Vec3::new(0.012, 0.004, inner.z), Mat3::IDENTITY, edge));
    // Piqûres de la pierre et tache de crasse.
    for _ in 0..(2.0 + 4.0 * rough * (half.x * half.z * 4.0).min(2.0)) as usize {
        let r = 0.008 + rng.next() * 0.012;
        p.push(ellipsoid(
            Vec3::new(center.x + rng.spread(inner.x), surface, center.z + rng.spread(inner.z)),
            Vec3::new(r, 0.004, r * (0.6 + rng.next() * 0.6)),
            darker(tone, 0.72),
        ));
    }
    // Crevasse irrégulière sur certaines dalles: trois segments qui bifurquent.
    if rough > 0.3 && half.x > 0.5 && rng.next() < 0.3 {
        let mut at = Vec3::new(center.x + rng.spread(inner.x * 0.6), surface + 0.002, center.z + rng.spread(inner.z * 0.6));
        let mut angle = rng.spread(3.0);
        for _ in 0..3 {
            let len = 0.12 + rng.next() * 0.16;
            let next = at + Vec3::new(angle.cos() * len, 0.0, angle.sin() * len);
            p.push(block((at + next) / 2.0, Vec3::new(len / 2.0, 0.004, 0.007), Mat3::from_rotation_y(-angle), darker(tone, 0.6)));
            at = next;
            angle += rng.spread(0.9);
        }
    }
}

/// Bloc de mur taillé à la main, vu de face: socle, face en petites facettes inégales avec quelques bossages, arête
/// supérieure éclairée et arête inférieure dans l'ombre, piqûres et crevasses.
fn rough_block(p: &mut Vec<Part>, rng: &mut Rng, center: Vec3, half: Vec3, tone: u32, mortar: u32) {
    p.push(shiny(block(center, half, Mat3::IDENTITY, darker(tone, 0.78)), 0.1));
    let inner = Vec3::new(half.x - 0.04, half.y - 0.035, half.z);
    let cell = 0.11;
    let (nx, ny) = (((inner.x * 2.0) / cell).ceil().max(1.0) as i32, ((inner.y * 2.0) / cell).ceil().max(1.0) as i32);
    let (sx, sy) = (inner.x * 2.0 / nx as f32, inner.y * 2.0 / ny as f32);
    let front = center.z + half.z;
    for ix in 0..nx {
        for iy in 0..ny {
            let (cx, cy) = (center.x - inner.x + (ix as f32 + 0.5) * sx, center.y - inner.y + (iy as f32 + 0.5) * sy);
            // Quelques bossages bruts qui sortent de la face.
            let boss = if rng.next() < 0.05 { 0.02 } else { 0.0 };
            let lift = 0.012 + rng.next() * 0.007 + boss;
            let shade = if rng.next() < 0.5 { mortar } else { 0xffffff };
            let facet = lerp_color(tone, shade, rng.next() * 0.06);
            p.push(shiny(
                block(
                    Vec3::new(cx, cy, front + lift / 2.0),
                    Vec3::new(sx / 2.0 * 0.985, sy / 2.0 * 0.985, lift / 2.0),
                    Mat3::from_rotation_x(rng.spread(0.02)) * Mat3::from_rotation_y(rng.spread(0.02)),
                    facet,
                ),
                0.2,
            ));
        }
    }
    let surface = front + 0.05;
    // Arête haute éclairée, arête basse dans l'ombre.
    p.push(block(Vec3::new(center.x, center.y + inner.y - 0.012, surface), Vec3::new(inner.x, 0.012, 0.004), Mat3::IDENTITY, lerp_color(tone, 0xffffff, 0.25)));
    p.push(block(Vec3::new(center.x, center.y - inner.y + 0.012, surface), Vec3::new(inner.x, 0.012, 0.004), Mat3::IDENTITY, darker(tone, 0.5)));
    for _ in 0..(2 + (half.x * 3.0) as usize) {
        let r = 0.008 + rng.next() * 0.012;
        p.push(ellipsoid(
            Vec3::new(center.x + rng.spread(inner.x), center.y + rng.spread(inner.y), surface),
            Vec3::new(r, r * (0.6 + rng.next() * 0.6), 0.004),
            darker(tone, 0.72),
        ));
    }
    if rng.next() < 0.3 && half.x > 0.4 {
        let mut at = Vec3::new(center.x + rng.spread(inner.x * 0.6), center.y + rng.spread(inner.y * 0.5), surface + 0.002);
        let mut angle = rng.spread(3.0);
        for _ in 0..3 {
            let len = 0.08 + rng.next() * 0.12;
            let next = at + Vec3::new(angle.cos() * len, angle.sin() * len, 0.0);
            p.push(block((at + next) / 2.0, Vec3::new(len / 2.0, 0.007, 0.004), Mat3::from_rotation_z(angle), darker(tone, 0.4)));
            at = next;
            angle += rng.spread(0.9);
        }
    }
    let _ = mortar;
}

/// Découpe `total` mètres en morceaux de largeur aléatoire comprise entre `lo` et `hi`.
fn split(rng: &mut Rng, total: f32, lo: f32, hi: f32) -> Vec<f32> {
    let mut parts = Vec::new();
    let mut left = total;
    while left > 1e-3 {
        let mut w = lo + rng.next() * (hi - lo);
        if left - w < lo * 0.8 {
            w = left;
        }
        parts.push(w.min(left));
        left -= w.min(left);
    }
    parts
}

/// Sol, vu d'en haut, sur un carré de 4x4 tuiles (4,8 m): de grandes dalles de pierre irrégulières, posées
/// par assises de hauteurs différentes, avec des joints décalés. Les joints tombent sur les bords du carré, donc
/// plusieurs motifs se raccordent sans coupure.
pub fn floor_macro(region: usize, variant: usize, seed: u64, theme: Theme) -> Vec<Part> {
    stone_macro(region, variant, seed, theme, false)
}

/// Dessus des murs épais, vu d'en haut: de très grosses pierres sombres, presque noires, sans détail, comme la masse
/// brute d'un rempart. Même découpage que le sol (4x4 tuiles).
pub fn roof_macro(region: usize, variant: usize, seed: u64, theme: Theme) -> Vec<Part> {
    stone_macro(region, variant, seed, theme, true)
}

fn stone_macro(region: usize, variant: usize, seed: u64, theme: Theme, roof: bool) -> Vec<Part> {
    let gore = theme == Theme::Gore;
    let st = style(region, gore);
    // Forêt et marais: herbe, vase, canopée et eau; les abords gardent le dallage (terre battue, rochers).
    if region == 6 || region == 7 {
        return if roof { crate::outdoor::top(region, variant, seed, &st) } else { crate::outdoor::ground(region, variant, seed, &st, gore) };
    }
    let palette = if roof { st.wall } else { st.slab };
    let mut rng = Rng(seed);
    let side = MACRO as f32 * 1.2;
    let half = side / 2.0;
    let mut p = vec![block(Vec3::new(0.0, -0.05, 0.0), Vec3::new(half + 0.02, 0.05, half + 0.02), Mat3::IDENTITY, st.mortar)];
    // (hauteur min, max, largeur min, max) des dalles: parquet en lattes, marbre en grands carrés, pierre ailleurs.
    let (hlo, hhi, wlo, whi) = match (roof, region) {
        (true, _) => (1.3, 2.4, 1.6, 3.2),
        (_, 2) => (0.3, 0.36, 1.8, 3.6),
        (_, 5) => (2.4, 2.4, 2.4, 2.4),
        _ => (0.95, 1.7, 1.1, 2.5),
    };
    let gap = if region == 2 { 0.03 } else { 0.06 };
    let mut z = -half;
    for (ci, h) in split(&mut rng, side, hlo, hhi).into_iter().enumerate() {
        let mut x = -half;
        for (si, w) in split(&mut rng, side, wlo, whi).into_iter().enumerate() {
            let idx = if region == 5 && !roof { (ci + si) % 2 } else { (ci * 3 + si + variant) % 3 };
            // Chaque dalle a sa propre teinte, un peu plus sombre ou plus claire que sa voisine.
            let tone = lerp_color(palette[idx], if rng.next() < 0.5 { st.mortar } else { 0xffffff }, rng.next() * 0.14);
            // Le dessus des murs est nettement plus sombre que le sol.
            let tone = if roof { darker(tone, 0.6) } else { tone };
            let thick = 0.05 + rng.next() * 0.025;
            // Marbre poli au palais royal, bois lisse en bibliothèque, pierre brute ailleurs.
            let rough = match (roof, region) {
                (true, _) => 0.7,
                (_, 5) => 0.25,
                (_, 2) => 0.5,
                _ => 1.0,
            };
            rough_slab(
                &mut p,
                &mut rng,
                Vec3::new(x + w / 2.0, 0.04, z + h / 2.0),
                Vec3::new((w - gap) / 2.0, thick, (h - gap) / 2.0),
                tone,
                st.mortar,
                rough,
            );
            x += w;
        }
        z += h;
    }
    // Petits détails propres à la région, rares et discrets (jamais sur le dessus des murs).
    if roof {
        return p;
    }
    match region {
        1 => {
            for _ in 0..3 {
                p.push(ellipsoid(Vec3::new(rng.spread(half * 0.9), 0.1, rng.spread(half * 0.9)), Vec3::new(0.3 + rng.next() * 0.2, 0.02, 0.22), 0x3a7a3a));
            }
            for _ in 0..4 {
                let (x, zz) = (rng.spread(half * 0.9), rng.spread(half * 0.9));
                p.push(cone(Vec3::new(x, 0.05, zz), Vec3::new(x, 0.2, zz), 0.035, 0.004, st.accent));
            }
        }
        3 => {
            for _ in 0..2 {
                let (x, zz) = (rng.spread(half * 0.8), rng.spread(half * 0.8));
                p.push(glow(block(Vec3::new(x, 0.08, zz), Vec3::new(0.3 + rng.next() * 0.4, 0.008, 0.02), Mat3::from_rotation_y(rng.spread(1.2)), st.glow)));
            }
        }
        4 => {
            let (x, zz) = (rng.spread(half * 0.7), rng.spread(half * 0.7));
            p.push(glow(block(Vec3::new(x, 0.08, zz), Vec3::new(0.35, 0.006, 0.012), Mat3::from_rotation_y(rng.spread(1.2)), st.glow)));
        }
        _ => {}
    }
    if gore && rng.next() < 0.5 {
        // Quelques os dans un coin, jamais de flaque: les grandes flaques sont posées par le jeu.
        let (x, zz) = (rng.spread(half * 0.8), rng.spread(half * 0.8));
        p.push(shiny(limb(Vec3::new(x, 0.1, zz), Vec3::new(x + 0.35, 0.1, zz + rng.spread(0.15)), 0.03, 0xdcd4bc), 0.4));
        p.push(ball(Vec3::new(x + 0.38, 0.1, zz), 0.045, 0xdcd4bc));
    }
    p
}

/// Face d'un mur, vue de face, sur 4 tuiles de large: de gros blocs de pierre irréguliers par assises de 0,4 m,
/// avec un liseré clair en haut et de l'ombre en bas. En mode sanglant, des coulures de sang.
pub fn wall_face_macro(region: usize, variant: usize, seed: u64, theme: Theme) -> Vec<Part> {
    let gore = theme == Theme::Gore;
    let st = style(region, gore);
    if region == 6 || region == 7 {
        return crate::outdoor::face(region, variant, seed, &st);
    }
    let mut rng = Rng(seed);
    let width = MACRO as f32 * 1.2;
    let half = width / 2.0;
    let mut p = vec![block(Vec3::new(0.0, 0.0, -0.1), Vec3::new(half + 0.02, H + 0.02, 0.05), Mat3::IDENTITY, st.mortar)];
    for row in 0..3 {
        let y = -0.4 + row as f32 * 0.4;
        let mut x = -half;
        for (k, w) in split(&mut rng, width, 0.8, 1.7).into_iter().enumerate() {
            let c = st.wall[(rng.next() * 3.0) as usize % 3];
            let tone = lerp_color(c, if (k + row + variant) % 2 == 0 { 0xffffff } else { st.mortar }, rng.next() * 0.12);
            let depth = 0.06 + rng.next() * 0.03;
            rough_block(&mut p, &mut rng, Vec3::new(x + w / 2.0, y, 0.0), Vec3::new((w - 0.05) / 2.0, 0.175, depth), tone, st.mortar);
            x += w;
        }
    }
    p.push(shiny(block(Vec3::new(0.0, 0.54, 0.07), Vec3::new(half + 0.02, 0.07, 0.11), Mat3::IDENTITY, st.wall[1]), 0.8));
    p.push(shiny(block(Vec3::new(0.0, 0.485, 0.1), Vec3::new(half + 0.02, 0.012, 0.11), Mat3::IDENTITY, st.slab[0]), 0.9));
    p.push(block(Vec3::new(0.0, -0.57, 0.1), Vec3::new(half + 0.02, 0.05, 0.1), Mat3::IDENTITY, st.mortar));
    if gore {
        for _ in 0..4 {
            if rng.next() < 0.6 {
                let x = rng.spread(half * 0.95);
                let len = 0.25 + rng.next() * 0.55;
                p.push(shiny(cone(Vec3::new(x, 0.5, 0.1), Vec3::new(x, 0.5 - len, 0.1), 0.035, 0.012, 0x8a1018), 0.8));
                p.push(shiny(ball(Vec3::new(x, 0.5 - len, 0.1), 0.026, 0x8a1018), 0.9));
            }
        }
    }
    p
}

/// Face de mur décorée (une seule tuile): niche à crâne, lierre, étagères de livres, plaques de forge, runes, pilastre,
/// ou en mode sanglant crâne, chaînes et crochet de boucher.
pub fn wall_face_decor(region: usize, variant: usize, seed: u64, theme: Theme) -> Vec<Part> {
    let gore = theme == Theme::Gore;
    let st = style(region, gore);
    if region == 6 || region == 7 {
        return crate::outdoor::decor(region, seed, &st);
    }
    let mut rng = Rng(seed);
    let mut p = vec![block(Vec3::new(0.0, 0.0, -0.1), Vec3::new(H + 0.02, H + 0.02, 0.05), Mat3::IDENTITY, st.mortar)];
    let decorated = variant == 1;
    // Étagères de bibliothèque ou plaques de forge remplacent les briques sur la variante décorée.
    if decorated && !gore && region == 2 {
        p.push(block(Vec3::new(0.0, 0.0, 0.0), Vec3::new(H, H, 0.05), Mat3::IDENTITY, 0x3a2412));
        for (k, y) in [-0.4f32, -0.02, 0.36].into_iter().enumerate() {
            p.push(shiny(block(Vec3::new(0.0, y - 0.12, 0.06), Vec3::new(H, 0.025, 0.08), Mat3::IDENTITY, 0x8a5a34), 0.3));
            let mut x = -0.52;
            while x < 0.5 {
                let w = 0.04 + rng.next() * 0.035;
                let h = 0.14 + rng.next() * 0.08;
                let colors = [0xc02840, 0x2f5fb0, 0x3a8a40, 0xd8a030, 0x7a3aa8, 0xe8e0d0];
                let c = colors[(rng.next() * colors.len() as f32) as usize % colors.len()];
                p.push(shiny(block(Vec3::new(x + w, y + h / 2.0 - 0.1 + 0.0, 0.07), Vec3::new(w, h / 2.0, 0.045), Mat3::from_rotation_z(rng.spread(0.05)), c), 0.5));
                x += w * 2.0 + 0.01;
            }
            let _ = k;
        }
    } else if decorated && !gore && region == 3 {
        for (x, y) in [(-0.3f32, -0.3f32), (0.3, -0.3), (-0.3, 0.3), (0.3, 0.3)] {
            p.push(shiny(block(Vec3::new(x, y, 0.0), Vec3::new(0.27, 0.27, 0.06), Mat3::IDENTITY, st.wall[((x + y) * 5.0) as i32 as usize % 3]), 0.7));
            for (dx, dy) in [(-0.2f32, -0.2f32), (0.2, -0.2), (-0.2, 0.2), (0.2, 0.2)] {
                p.push(shiny(ball(Vec3::new(x + dx, y + dy, 0.07), 0.02, 0x9aa0b0), 0.8));
            }
        }
        p.push(glow(block(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.4, 0.025, 0.01), Mat3::IDENTITY, st.glow)));
    } else {
        let mut row = 0;
        let mut y = -0.45;
        while y < 0.5 {
            let mut x = if row % 2 == 0 { -0.6 } else { -0.4 };
            while x < 0.6 {
                let c = st.wall[(rng.next() * 3.0) as usize % 3];
                p.push(shiny(
                    block(Vec3::new(x + 0.2, y + 0.15, 0.0), Vec3::new(0.185, 0.135, 0.05 + rng.next() * 0.025), Mat3::from_rotation_z(rng.spread(0.025)), c),
                    0.25,
                ));
                x += 0.4;
            }
            y += 0.3;
            row += 1;
        }
    }
    // Liseré de pierre clair en haut (l'arête du mur) et ombre épaisse en bas, au contact du sol.
    p.push(shiny(block(Vec3::new(0.0, 0.54, 0.07), Vec3::new(H + 0.02, 0.07, 0.11), Mat3::IDENTITY, st.wall[1]), 0.8));
    p.push(shiny(block(Vec3::new(0.0, 0.485, 0.1), Vec3::new(H + 0.02, 0.012, 0.11), Mat3::IDENTITY, st.slab[0]), 0.9));
    p.push(block(Vec3::new(0.0, -0.57, 0.1), Vec3::new(H + 0.02, 0.05, 0.1), Mat3::IDENTITY, st.mortar));
    if decorated && !gore {
        match region {
            0 => {
                p.push(block(Vec3::new(0.0, -0.05, 0.08), Vec3::new(0.22, 0.24, 0.02), Mat3::IDENTITY, 0x0c0e14));
                p.push(shiny(ball(Vec3::new(0.0, -0.03, 0.13), 0.13, st.accent), 0.6));
                for sg in [-1.0f32, 1.0] {
                    p.push(ball(Vec3::new(sg * 0.05, -0.0, 0.25), 0.032, 0x0c0e14));
                    p.push(glow(ball(Vec3::new(sg * 0.05, -0.0, 0.25), 0.012, 0x70a0ff)));
                }
            }
            1 => {
                for k in 0..3 {
                    let x = -0.35 + k as f32 * 0.35;
                    p.push(limb(Vec3::new(x, 0.5, 0.1), Vec3::new(x + rng.spread(0.1), -0.2 - rng.next() * 0.2, 0.1), 0.02, 0x2e6a34));
                    for l in 0..3 {
                        p.push(ellipsoid(Vec3::new(x + rng.spread(0.07), 0.3 - l as f32 * 0.18, 0.12), Vec3::new(0.06, 0.03, 0.015), 0x4ca850));
                    }
                }
            }
            4 => {
                for (x, y) in [(0.0f32, 0.2f32), (-0.15, -0.05), (0.15, -0.05), (0.0, -0.25)] {
                    p.push(glow(block(Vec3::new(x, y, 0.09), Vec3::new(0.05, 0.015, 0.01), Mat3::from_rotation_z(rng.spread(0.8)), st.glow)));
                }
            }
            5 => {
                p.push(shiny(block(Vec3::new(0.0, 0.0, 0.09), Vec3::new(0.1, 0.5, 0.03), Mat3::IDENTITY, st.accent), 0.9));
                p.push(block(Vec3::new(0.0, 0.0, 0.12), Vec3::new(0.06, 0.4, 0.01), Mat3::IDENTITY, 0x8a1c2c));
            }
            _ => {}
        }
    }
    if gore {
        // Coulures de sang le long de la face; sur la variante décorée, un crâne planté, des chaînes et un crochet de boucher.
        for _ in 0..3 {
            if rng.next() < 0.6 {
                let x = rng.spread(0.5);
                let len = 0.2 + rng.next() * 0.5;
                p.push(shiny(cone(Vec3::new(x, 0.5, 0.1), Vec3::new(x, 0.5 - len, 0.1), 0.035, 0.012, 0x8a1018), 0.8));
                p.push(shiny(ball(Vec3::new(x, 0.5 - len, 0.1), 0.026, 0x8a1018), 0.9));
            }
        }
        if decorated {
            p.push(block(Vec3::new(-0.25, 0.0, 0.08), Vec3::new(0.2, 0.22, 0.02), Mat3::IDENTITY, 0x08060a));
            p.push(shiny(ball(Vec3::new(-0.25, 0.0, 0.14), 0.13, 0xdcd4bc), 0.5));
            for sg in [-1.0f32, 1.0] {
                p.push(ball(Vec3::new(-0.25 + sg * 0.05, 0.02, 0.25), 0.032, 0x08060a));
            }
            for k in 0..5 {
                let y = 0.5 - k as f32 * 0.17;
                p.push(shiny(ball(Vec3::new(0.3, y, 0.1), 0.045, 0x6a6e78), 0.7));
            }
            p.push(shiny(limb(Vec3::new(0.3, 0.5, 0.1), Vec3::new(0.3, -0.35, 0.1), 0.02, 0x6a6e78), 0.7));
            p.push(shiny(limb(Vec3::new(0.3, -0.35, 0.1), Vec3::new(0.18, -0.45, 0.1), 0.03, 0x8a8e98), 0.8));
            p.push(shiny(ellipsoid(Vec3::new(0.18, -0.52, 0.11), Vec3::new(0.06, 0.1, 0.03), 0xa83a4c), 0.8));
        }
    }
    p
}

/// Palettes sombres et sales du thème sanglant: pierre grise ou brune, joints noirs, un peu de rouge.
fn style_gore(region: usize) -> Style {
    match region {
        0 => Style { slab: [0x4a4c52, 0x54565e, 0x40424a], mortar: 0x0c0a0c, wall: [0x24222a, 0x2c2a34, 0x1c1a22], accent: 0xd8d0b8, glow: 0x6a8a5a },
        1 => Style { slab: [0x3e4a38, 0x485442, 0x343e30], mortar: 0x0a0e0a, wall: [0x1e281e, 0x263226, 0x161e16], accent: 0x6a8a3a, glow: 0xa8c040 },
        2 => Style { slab: [0x5a3e2a, 0x4e3422, 0x664832], mortar: 0x120a06, wall: [0x2e1c12, 0x241610, 0x3a2418], accent: 0x8a1c1c, glow: 0xd8a040 },
        3 => Style { slab: [0x34303a, 0x3c3844, 0x2a2630], mortar: 0x0a080a, wall: [0x2a1c1c, 0x201414, 0x362424], accent: 0xff4010, glow: 0xff6a20 },
        4 => Style { slab: [0x3a3c58, 0x444664, 0x303250], mortar: 0x0a0a1a, wall: [0x16182e, 0x1e203c, 0x101224], accent: 0x8aa0d8, glow: 0x6a90ff },
        5 => Style { slab: [0x6a5a52, 0x5a4a44, 0x7a6860], mortar: 0x180c0c, wall: [0x3a1018, 0x2e0c14, 0x481420], accent: 0xb89030, glow: 0xff3040 },
        6 => Style { slab: [0x3c4a2a, 0x344024, 0x465632], mortar: 0x10160a, wall: [0x1a2a18, 0x142214, 0x223416], accent: 0x8a1c1c, glow: 0xa8c040 },
        7 => Style { slab: [0x3a4a2e, 0x303e28, 0x445636], mortar: 0x0c120c, wall: [0x14262a, 0x1a3234, 0x0e1c1e], accent: 0x6a8a3a, glow: 0x70e0a0 },
        _ => Style { slab: [0x6a5e48, 0x5c5240, 0x76684e], mortar: 0x140e08, wall: [0x3e3a34, 0x32302a, 0x4a463e], accent: 0x8a1c1c, glow: 0xd8a040 },
    }
}
