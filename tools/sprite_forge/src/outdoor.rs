//! Les tuiles des décors extérieurs: la forêt (sol herbeux, murs de grands arbres) et le marais (sol de vase, murs d'eau
//! et de roseaux). Mêmes dimensions que les tuiles du château: un carré de 4x4 tuiles pour le sol et le dessus des
//! murs, une bande de 4 tuiles de large pour les faces. Les motifs se raccordent sur leurs bords: tout ce qui touche un
//! bord est aussi dessiné de l'autre côté.

use glam::{Mat3, Vec3};

use crate::render::Part;
use crate::shapes::*;
use crate::tiles::{MACRO, Style, darker, lerp_color};

const MARSH: usize = 7;

/// Les positions où dessiner un élément centré en `x` de rayon `r`, dans un carré de côté `side` centré sur 0: lui-même,
/// et son double de l'autre côté s'il dépasse d'un bord.
fn wraps(x: f32, r: f32, side: f32) -> Vec<f32> {
    let half = side / 2.0;
    let mut out = vec![x];
    if x - r < -half {
        out.push(x + side);
    }
    if x + r > half {
        out.push(x - side);
    }
    out
}

fn pick(rng: &mut Rng, colors: &[u32]) -> u32 {
    colors[(rng.next() * colors.len() as f32) as usize % colors.len()]
}

/// Un brin d'herbe couché: un trait court, un peu relevé à la pointe.
fn blade(p: &mut Vec<Part>, x: f32, z: f32, angle: f32, len: f32, color: u32) {
    p.push(limb(Vec3::new(x, 0.045, z), Vec3::new(x + angle.cos() * len, 0.062, z + angle.sin() * len), 0.0085, color));
}

/// Sol vu d'en haut: un tapis dense de brins d'herbe de verts voisins, sans grandes taches (les trois variantes se
/// ressemblent assez pour que les raccords entre motifs ne se voient pas). Variante 1: trèfles et petites fleurs;
/// variante 2: feuilles mortes et brindilles. Dans le marais: de la vase moussue, des flaques irrégulières et des
/// touffes de roseaux.
pub fn ground(region: usize, variant: usize, seed: u64, st: &Style, gore: bool) -> Vec<Part> {
    let mut rng = Rng(seed);
    let side = MACRO as f32 * 1.2;
    let half = side / 2.0;
    let marsh = region == MARSH;
    let base = lerp_color(st.slab[1], st.mortar, 0.12);
    let mut p = vec![block(Vec3::new(0.0, -0.05, 0.0), Vec3::new(half + 0.02, 0.05, half + 0.02), Mat3::IDENTITY, base)];
    // Mouchetures fines de la terre qui affleure: de petites taches, de très peu de contraste.
    for i in 0..220 {
        let (x, z) = (rng.spread(half), rng.spread(half));
        let r = 0.07 + rng.next() * 0.16;
        let color = lerp_color(st.slab[i % 3], if rng.next() < 0.5 { st.mortar } else { 0xffffff }, rng.next() * 0.1);
        for dx in wraps(x, r, side) {
            for dz in wraps(z, r, side) {
                p.push(ellipsoid(Vec3::new(dx, 0.02 + (i % 7) as f32 * 0.0004, dz), Vec3::new(r, 0.01, r * (0.6 + rng.next() * 0.6)), color));
            }
        }
    }
    if marsh {
        // Flaques d'eau trouble aux contours irréguliers (trois ellipses qui se chevauchent), avec un reflet.
        for _ in 0..7 {
            let (x, z) = (rng.spread(half), rng.spread(half));
            let r = 0.18 + rng.next() * 0.25;
            for k in 0..3 {
                let (ox, oz) = (rng.spread(r * 0.7), rng.spread(r * 0.7));
                let rr = r * (0.55 + rng.next() * 0.5);
                for dx in wraps(x + ox, rr, side) {
                    for dz in wraps(z + oz, rr, side) {
                        p.push(shiny(ellipsoid(Vec3::new(dx, 0.036 + k as f32 * 0.0005, dz), Vec3::new(rr, 0.01, rr * 0.7), 0x2c5a56), 0.8));
                    }
                }
            }
            for dx in wraps(x - r * 0.15, r * 0.4, side) {
                for dz in wraps(z - r * 0.1, r * 0.3, side) {
                    p.push(shiny(ellipsoid(Vec3::new(dx, 0.045, dz), Vec3::new(r * 0.4, 0.008, r * 0.18), 0x6aa6a0), 0.9));
                }
            }
        }
    }
    // Le tapis d'herbe: beaucoup de brins, de verts proches.
    let greens = [
        lerp_color(st.slab[0], st.mortar, 0.4),
        lerp_color(st.slab[1], st.mortar, 0.15),
        st.slab[2],
        lerp_color(st.slab[0], 0xffffff, 0.2),
        lerp_color(st.slab[2], 0xe0d870, if marsh { 0.1 } else { 0.22 }),
    ];
    let blades = if marsh { 520 } else { 1250 };
    for _ in 0..blades {
        let (x, z) = (rng.spread(half), rng.spread(half));
        let angle = rng.next() * std::f32::consts::TAU;
        let len = 0.06 + rng.next() * 0.1;
        let color = pick(&mut rng, &greens);
        for dx in wraps(x, len, side) {
            for dz in wraps(z, len, side) {
                blade(&mut p, dx, dz, angle, len, color);
            }
        }
    }
    // Touffes: quelques brins qui partent d'un même point.
    for _ in 0..(if marsh { 20 } else { 46 }) {
        let (x, z) = (rng.spread(half), rng.spread(half));
        let color = pick(&mut rng, &greens[2..]);
        for dx in wraps(x, 0.2, side) {
            for dz in wraps(z, 0.2, side) {
                for k in 0..6 {
                    blade(&mut p, dx, dz, k as f32 * 1.05 + rng.next() * 0.5, 0.12 + rng.next() * 0.08, color);
                }
            }
        }
    }
    if marsh {
        // Touffes de roseaux et nénuphars des flaques.
        for _ in 0..16 {
            let (x, z) = (rng.spread(half), rng.spread(half));
            for dx in wraps(x, 0.25, side) {
                for dz in wraps(z, 0.25, side) {
                    for k in 0..5 {
                        let a = k as f32 * 1.26 + rng.next();
                        p.push(limb(Vec3::new(dx, 0.05, dz), Vec3::new(dx + a.cos() * 0.2, 0.08, dz + a.sin() * 0.2), 0.012, pick(&mut rng, &[0x4a6a2c, 0x6a7a34, 0x3a5a28])));
                    }
                }
            }
        }
    } else {
        match variant {
            1 => {
                // Trèfles et petites fleurs.
                for _ in 0..36 {
                    let (x, z) = (rng.spread(half), rng.spread(half));
                    let petal = pick(&mut rng, &[0xffffff, 0xf4f0d0, 0xe880b0, 0xf0d040, 0xa890e8]);
                    for dx in wraps(x, 0.05, side) {
                        for dz in wraps(z, 0.05, side) {
                            for k in 0..4 {
                                let a = k as f32 * 1.57;
                                p.push(ball(Vec3::new(dx + a.cos() * 0.022, 0.07, dz + a.sin() * 0.022), 0.014, petal));
                            }
                            p.push(ball(Vec3::new(dx, 0.075, dz), 0.012, 0xe8b830));
                        }
                    }
                }
            }
            2 => {
                // Feuilles mortes et brindilles.
                for _ in 0..40 {
                    let (x, z) = (rng.spread(half), rng.spread(half));
                    let color = pick(&mut rng, &[0xb8702a, 0x8a5a20, 0xc89030, 0x6a4a1c]);
                    let a = rng.next() * 3.14;
                    for dx in wraps(x, 0.08, side) {
                        for dz in wraps(z, 0.08, side) {
                            p.push(ellipsoid(Vec3::new(dx, 0.06, dz), Vec3::new(0.05, 0.007, 0.028), color));
                            let _ = a;
                        }
                    }
                }
                for _ in 0..8 {
                    let (x, z) = (rng.spread(half), rng.spread(half));
                    let a = rng.next() * 3.14;
                    for dx in wraps(x, 0.2, side) {
                        for dz in wraps(z, 0.2, side) {
                            p.push(limb(Vec3::new(dx, 0.06, dz), Vec3::new(dx + a.cos() * 0.3, 0.065, dz + a.sin() * 0.3), 0.01, 0x4a3822));
                        }
                    }
                }
            }
            _ => {
                for _ in 0..6 {
                    let (x, z) = (rng.spread(half), rng.spread(half));
                    let r = 0.03 + rng.next() * 0.03;
                    for dx in wraps(x, r, side) {
                        for dz in wraps(z, r, side) {
                            p.push(shiny(ellipsoid(Vec3::new(dx, 0.055, dz), Vec3::new(r, 0.02, r * 0.8), pick(&mut rng, &[0x8a8478, 0x6a665c])), 0.15));
                        }
                    }
                }
            }
        }
    }
    if gore && rng.next() < 0.6 {
        // Quelques os dans l'herbe, comme dans les salles du château.
        let (x, z) = (rng.spread(half * 0.8), rng.spread(half * 0.8));
        p.push(shiny(limb(Vec3::new(x, 0.08, z), Vec3::new(x + 0.35, 0.08, z + rng.spread(0.15)), 0.03, 0xdcd4bc), 0.4));
        p.push(ball(Vec3::new(x + 0.38, 0.08, z), 0.045, 0xdcd4bc));
    }
    p
}

/// Dessus des « murs », vu d'en haut: la canopée de grands arbres serrés (forêt) ou une eau profonde et noire avec des
/// nénuphars (marais). Plus sombre que le sol: on ne peut pas y marcher.
pub fn top(region: usize, variant: usize, seed: u64, st: &Style) -> Vec<Part> {
    let mut rng = Rng(seed);
    let side = MACRO as f32 * 1.2;
    let half = side / 2.0;
    let mut p = vec![block(Vec3::new(0.0, -0.05, 0.0), Vec3::new(half + 0.02, 0.05, half + 0.02), Mat3::IDENTITY, darker(st.wall[1], 0.45))];
    if region == MARSH {
        for i in 0..34 {
            let (x, z) = (rng.spread(half), rng.spread(half));
            let r = 0.4 + rng.next() * 0.6;
            let color = lerp_color(st.wall[i % 3], 0x6ab0a8, rng.next() * 0.3);
            for dx in wraps(x, r, side) {
                for dz in wraps(z, r, side) {
                    p.push(shiny(ellipsoid(Vec3::new(dx, 0.02 + i as f32 * 0.001, dz), Vec3::new(r, 0.012, r * (0.6 + rng.next() * 0.5)), color), 0.9));
                }
            }
        }
        for _ in 0..(10 + variant * 4) {
            let (x, z) = (rng.spread(half), rng.spread(half));
            let r = 0.14 + rng.next() * 0.12;
            for dx in wraps(x, r, side) {
                for dz in wraps(z, r, side) {
                    p.push(shiny(ellipsoid(Vec3::new(dx, 0.07, dz), Vec3::new(r, 0.012, r), pick(&mut rng, &[0x3a6a2c, 0x4a7a34, 0x2c5a28])), 0.5));
                }
            }
        }
        return p;
    }
    // Forêt: un feuillage dense et lumpeux, en plusieurs couches de grappes de petites feuilles, de plus en plus claires vers
    // le haut. Pas de reflets: de la verdure mate, avec des trouées sombres entre les grappes.
    for (layer, count) in [60usize, 52, 44, 34].into_iter().enumerate() {
        for _ in 0..count {
            let (cx, cz) = (rng.spread(half), rng.spread(half));
            let reach = 0.34 + rng.next() * 0.14;
            let y = 0.08 + layer as f32 * 0.16;
            let tone = lerp_color(pick(&mut rng, &st.wall), 0xb8e070, layer as f32 * 0.07 + rng.next() * 0.05);
            // Une grappe: sept boules de feuilles autour d'un centre.
            for k in 0..7 {
                let a = k as f32 * 0.9 + rng.next() * 0.6;
                let d = if k == 0 { 0.0 } else { reach * (0.35 + rng.next() * 0.55) };
                let r = 0.1 + rng.next() * 0.09;
                let shade = lerp_color(tone, 0x0c2410, rng.next() * 0.3);
                let (x, z) = (cx + a.cos() * d, cz + a.sin() * d);
                for dx in wraps(x, r, side) {
                    for dz in wraps(z, r, side) {
                        p.push(ellipsoid(Vec3::new(dx, y + rng.next() * 0.05, dz), Vec3::new(r, 0.1, r * (0.8 + rng.next() * 0.3)), shade));
                    }
                }
            }
        }
    }
    // Quelques feuilles claires tout en haut, éclairées.
    for _ in 0..70 {
        let (x, z) = (rng.spread(half), rng.spread(half));
        let r = 0.05 + rng.next() * 0.05;
        for dx in wraps(x, r, side) {
            for dz in wraps(z, r, side) {
                p.push(ellipsoid(Vec3::new(dx, 0.8, dz), Vec3::new(r, 0.04, r * 0.7), lerp_color(st.wall[2], 0xe8ffa0, 0.35 + rng.next() * 0.2)));
            }
        }
    }
    p
}

/// Face d'un « mur », vue de face, sur 4 tuiles de large: des troncs d'arbres et un feuillage sombre (forêt), ou une berge
/// de roseaux au-dessus de l'eau (marais).
pub fn face(region: usize, _variant: usize, seed: u64, st: &Style) -> Vec<Part> {
    let mut rng = Rng(seed);
    let width = MACRO as f32 * 1.2;
    let half = width / 2.0;
    let mut p = vec![block(Vec3::new(0.0, 0.0, -0.1), Vec3::new(half + 0.02, 0.62, 0.05), Mat3::IDENTITY, darker(st.wall[1], 0.55))];
    if region == MARSH {
        p.push(shiny(block(Vec3::new(0.0, -0.45, 0.0), Vec3::new(half + 0.02, 0.17, 0.04), Mat3::IDENTITY, lerp_color(st.wall[1], 0x2c5a56, 0.5)), 0.9));
        for _ in 0..70 {
            let x = rng.spread(half);
            let top = -0.2 + rng.next() * 0.75;
            let lean = rng.spread(0.07);
            let color = pick(&mut rng, &[0x4a6a2c, 0x6a7a34, 0x3a5a28, 0x7a6a38]);
            for dx in wraps(x, 0.1, width) {
                p.push(cone(Vec3::new(dx, -0.55, 0.05), Vec3::new(dx + lean, top, 0.05), 0.022, 0.004, color));
                if top > 0.4 && rng.next() < 0.35 {
                    p.push(shiny(ellipsoid(Vec3::new(dx + lean, top - 0.05, 0.06), Vec3::new(0.022, 0.07, 0.022), 0x5a3a20), 0.3));
                }
            }
        }
        p.push(block(Vec3::new(0.0, -0.57, 0.1), Vec3::new(half + 0.02, 0.05, 0.1), Mat3::IDENTITY, st.mortar));
        return p;
    }
    // Forêt: la lisière vue de face. Au fond, des troncs lointains et sombres; devant, de gros troncs effilés à l'écorce
    // sillonnée, mousseux d'un côté et empâtés de racines; en haut, la masse du feuillage; en bas, fougères et buissons.
    for k in 0..9 {
        let x = -half + (k as f32 + 0.5) * width / 9.0 + rng.spread(0.12);
        for dx in wraps(x, 0.1, width) {
            p.push(limb(Vec3::new(dx, -0.6, -0.04), Vec3::new(dx + rng.spread(0.03), 0.62, -0.04), 0.045 + rng.next() * 0.02, darker(st.wall[1], 0.4)));
        }
    }
    let trunks = 5;
    for k in 0..trunks {
        let x = -half + (k as f32 + 0.5) * width / trunks as f32 + rng.spread(0.2);
        let (r0, r1) = (0.16 + rng.next() * 0.05, 0.095 + rng.next() * 0.02);
        let lean = rng.spread(0.04);
        let bark = lerp_color(pick(&mut rng, &[0x4a3220, 0x3e2a1a, 0x56402a]), st.wall[0], 0.12);
        let bark_d = darker(bark, 0.6);
        let bark_l = lerp_color(bark, 0xb09a78, 0.28);
        for dx in wraps(x, r0 + 0.25, width) {
            // Tronc effilé.
            p.push(cone(Vec3::new(dx, -0.6, 0.02), Vec3::new(dx + lean, 0.64, 0.02), r0, r1, bark));
            // Sillons et arêtes de l'écorce: de longs traits sombres et clairs qui suivent le tronc.
            for g in 0..9 {
                let f = (g as f32 / 8.0) * 2.0 - 1.0;
                let t0 = rng.next() * 0.3;
                let (ra, rb) = (r0 * f.abs().sqrt().max(0.0) * f.signum() * 0.92, r1 * f.signum() * f.abs().sqrt() * 0.92);
                let (ya, yb) = (-0.6 + t0, 0.64 - rng.next() * 0.2);
                let color = if g % 2 == 0 { bark_d } else { bark_l };
                let zf = |r: f32, rad: f32| 0.02 + (rad * rad - r * r).max(0.0).sqrt() * 0.95;
                p.push(limb(Vec3::new(dx + ra * (1.0 - t0 * 0.4), ya, zf(ra, r0)), Vec3::new(dx + lean + rb, yb, zf(rb, r1)), 0.011, color));
            }
            // Mousse sur le côté de l'ombre, nœud et trou.
            for _ in 0..4 {
                let y = -0.5 + rng.next() * 0.9;
                p.push(ellipsoid(Vec3::new(dx - r0 * 0.55, y, 0.02 + r0 * 0.6), Vec3::new(0.05 + rng.next() * 0.05, 0.05 + rng.next() * 0.08, 0.03), lerp_color(0x4a6a2c, st.wall[0], 0.3)));
            }
            p.push(ellipsoid(Vec3::new(dx + r0 * 0.2, 0.05 + rng.spread(0.25), 0.02 + r0 * 0.9), Vec3::new(0.035, 0.05, 0.02), darker(bark, 0.3)));
            // Racines qui s'évasent au pied.
            for sg in [-1.0f32, 1.0] {
                p.push(cone(Vec3::new(dx + sg * r0 * 1.9, -0.6, 0.05), Vec3::new(dx + sg * r0 * 0.55, -0.28, 0.04), 0.05, r0 * 0.5, bark));
            }
            // Une branche coupée.
            let side = if rng.next() < 0.5 { -1.0 } else { 1.0 };
            let by = -0.05 + rng.next() * 0.5;
            p.push(limb(Vec3::new(dx + side * r1 * 0.7, by, 0.03), Vec3::new(dx + side * (r1 + 0.16), by + 0.12, 0.06), 0.018, bark));
        }
    }
    // Feuillage qui retombe d'en haut: de petites grappes mates qui se chevauchent.
    for _ in 0..70 {
        let x = rng.spread(half);
        let y = 0.22 + rng.next() * 0.42;
        let rad = 0.08 + rng.next() * 0.12;
        let tone = lerp_color(pick(&mut rng, &st.wall), 0xa8d060, rng.next() * 0.22);
        let shade = lerp_color(tone, 0x0c2410, rng.next() * 0.3);
        for dx in wraps(x, rad, width) {
            p.push(ellipsoid(Vec3::new(dx, y, 0.11 + rng.next() * 0.04), Vec3::new(rad, rad * 0.85, 0.07), shade));
        }
    }
    // Sous-bois: fougères (éventails de feuilles) et buissons sombres au pied des troncs.
    for _ in 0..16 {
        let x = rng.spread(half);
        for dx in wraps(x, 0.25, width) {
            for k in 0..7 {
                let a = (k as f32 / 6.0 - 0.5) * 2.2;
                p.push(limb(Vec3::new(dx, -0.58, 0.14), Vec3::new(dx + a * 0.18, -0.58 + 0.2 + 0.08 * (1.0 - a.abs()), 0.16), 0.014, lerp_color(0x2c5a22, st.wall[2], rng.next() * 0.6)));
            }
        }
    }
    for _ in 0..16 {
        let x = rng.spread(half);
        let rad = 0.08 + rng.next() * 0.08;
        for dx in wraps(x, rad, width) {
            p.push(ellipsoid(Vec3::new(dx, -0.5 + rng.next() * 0.1, 0.16), Vec3::new(rad, rad * 0.7, 0.07), lerp_color(st.wall[1], 0x0c2410, 0.2 + rng.next() * 0.2)));
        }
    }
    p.push(block(Vec3::new(0.0, -0.57, 0.1), Vec3::new(half + 0.02, 0.05, 0.1), Mat3::IDENTITY, st.mortar));
    p
}

/// Face décorée (une seule tuile): un gros tronc creux avec des champignons (forêt), ou des roseaux et un feu follet
/// (marais).
pub fn decor(region: usize, seed: u64, st: &Style) -> Vec<Part> {
    let mut rng = Rng(seed);
    let mut p = vec![block(Vec3::new(0.0, 0.0, -0.1), Vec3::new(0.62, 0.62, 0.05), Mat3::IDENTITY, darker(st.wall[1], 0.55))];
    if region == MARSH {
        p.push(shiny(block(Vec3::new(0.0, -0.45, 0.0), Vec3::new(0.62, 0.17, 0.04), Mat3::IDENTITY, lerp_color(st.wall[1], 0x2c5a56, 0.5)), 0.9));
        for k in 0..14 {
            let x = -0.55 + k as f32 * 0.085 + rng.spread(0.02);
            let top = -0.1 + rng.next() * 0.6;
            p.push(cone(Vec3::new(x, -0.55, 0.05), Vec3::new(x + rng.spread(0.05), top, 0.05), 0.02, 0.004, pick(&mut rng, &[0x4a6a2c, 0x6a7a34, 0x3a5a28])));
        }
        // Feu follet entre les roseaux.
        p.push(glow(ball(Vec3::new(0.12, 0.18, 0.14), 0.07, 0x9affd0)));
        p.push(glow(ball(Vec3::new(0.12, 0.18, 0.15), 0.035, 0xffffff)));
        return p;
    }
    let bark = lerp_color(0x4a3220, st.wall[0], 0.12);
    p.push(cone(Vec3::new(0.0, -0.6, 0.02), Vec3::new(0.02, 0.64, 0.02), 0.27, 0.2, bark));
    for g in 0..11 {
        let f = (g as f32 / 10.0) * 2.0 - 1.0;
        let x = f * 0.22;
        let z = 0.02 + (0.24f32 * 0.24 - x * x).max(0.0).sqrt() * 0.95;
        p.push(limb(Vec3::new(x * 1.05, -0.58, z), Vec3::new(x * 0.8, 0.6, z), 0.012, if g % 2 == 0 { darker(bark, 0.6) } else { lerp_color(bark, 0xb09a78, 0.28) }));
    }
    // Racines évasées et gros nœud creux.
    for sg in [-1.0f32, 1.0] {
        p.push(cone(Vec3::new(sg * 0.4, -0.6, 0.06), Vec3::new(sg * 0.2, -0.3, 0.05), 0.06, 0.12, bark));
    }
    p.push(ellipsoid(Vec3::new(0.0, 0.02, 0.24), Vec3::new(0.1, 0.16, 0.05), 0x0c0a08));
    p.push(ellipsoid(Vec3::new(-0.12, 0.3, 0.24), Vec3::new(0.06, 0.05, 0.03), lerp_color(0x4a6a2c, st.wall[0], 0.3)));
    for (x, r) in [(-0.34f32, 0.07f32), (0.3, 0.06), (0.4, 0.05)] {
        p.push(cone(Vec3::new(x, -0.5, 0.1), Vec3::new(x, -0.42, 0.1), 0.02, 0.016, 0xe8e0c8));
        p.push(shiny(ellipsoid(Vec3::new(x, -0.4, 0.1), Vec3::new(r, r * 0.55, r), 0xc83030), 0.5));
        p.push(ball(Vec3::new(x + 0.02, -0.37, 0.14), 0.012, 0xffffff));
    }
    p
}
