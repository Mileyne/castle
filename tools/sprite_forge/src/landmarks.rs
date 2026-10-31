//! Grands motifs du décor (arbres, rochers, colonnes, statues, fontaines): dessinés en grand, vus de trois quarts comme les
//! sorties de niveau, et posés au sol par leur pied (l'origine du modèle).

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::render::Part;
use crate::shapes::*;

/// Ordre et noms de fichiers: le même que `LandmarkKind::ALL` dans `castle_shared`.
pub const LANDMARKS: [&str; 4] = ["boulder", "pillar", "statue", "fountain"];

pub fn build(index: usize) -> Vec<Part> {
    let mut p: Vec<Part> = Vec::new();
    match index {
        // Gros rocher couvert de mousse.
        0 => {
            p.push(shiny(ellipsoid(Vec3::new(0.0, 0.62, 0.0), Vec3::new(1.15, 0.8, 0.9), 0x7a7a84), 0.45));
            p.push(shiny(ellipsoid(Vec3::new(0.85, 0.38, 0.35), Vec3::new(0.6, 0.45, 0.5), 0x8e8e98), 0.4));
            p.push(shiny(ellipsoid(Vec3::new(-0.8, 0.32, -0.3), Vec3::new(0.55, 0.4, 0.45), 0x65656f), 0.4));
            p.push(shiny(ellipsoid(Vec3::new(0.2, 1.0, -0.15), Vec3::new(0.7, 0.45, 0.55), 0x8a8a94), 0.4));
            p.push(ellipsoid(Vec3::new(-0.2, 1.25, 0.0), Vec3::new(0.55, 0.14, 0.42), 0x5a8a40));
            p.push(ellipsoid(Vec3::new(0.7, 0.8, 0.45), Vec3::new(0.3, 0.09, 0.22), 0x5a8a40));
            p.push(limb(Vec3::new(-0.3, 0.5, 0.78), Vec3::new(0.1, 0.9, 0.85), 0.018, 0x2c2a30));
        }
        // Colonne cannelée.
        1 => {
            let stone = 0xc2beb0;
            p.push(shiny(block(Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.62, 0.14, 0.62), Mat3::IDENTITY, 0x9c988c), 0.2));
            p.push(shiny(cone(Vec3::new(0.0, 0.28, 0.0), Vec3::new(0.0, 0.5, 0.0), 0.5, 0.42, 0xaaa69a), 0.25));
            p.push(shiny(cone(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 3.0, 0.0), 0.4, 0.34, stone), 0.35));
            for k in 0..6 {
                let a = k as f32 * TAU / 6.0;
                p.push(limb(Vec3::new(a.cos() * 0.37, 0.7, a.sin() * 0.37), Vec3::new(a.cos() * 0.32, 2.8, a.sin() * 0.32), 0.012, 0x8e8a7e));
            }
            p.push(shiny(cone(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, 3.22, 0.0), 0.42, 0.52, 0xaaa69a), 0.25));
            p.push(shiny(block(Vec3::new(0.0, 3.36, 0.0), Vec3::new(0.64, 0.14, 0.64), Mat3::IDENTITY, 0x9c988c), 0.2));
        }
        // Statue de chevalier sur son socle.
        2 => {
            let stone = 0xaaa69c;
            p.push(shiny(block(Vec3::new(0.0, 0.2, 0.0), Vec3::new(0.65, 0.2, 0.65), Mat3::IDENTITY, 0x8e8a80), 0.2));
            p.push(shiny(block(Vec3::new(0.0, 0.62, 0.0), Vec3::new(0.5, 0.22, 0.5), Mat3::IDENTITY, 0x9c988c), 0.25));
            p.push(panel(Vec3::new(0.0, 1.9, -0.18), Vec3::new(0.0, 0.9, -0.3), 0.6, 0.06, 0x8a867c));
            p.push(shiny(cone(Vec3::new(0.0, 0.85, 0.0), Vec3::new(0.0, 1.95, 0.0), 0.3, 0.24, stone), 0.35));
            p.push(shiny(ball(Vec3::new(0.0, 2.15, 0.0), 0.2, stone), 0.35));
            p.push(shiny(ellipsoid(Vec3::new(0.0, 2.3, 0.0), Vec3::new(0.21, 0.12, 0.21), 0x9a968c), 0.3));
            p.push(shiny(limb(Vec3::new(0.28, 1.8, 0.0), Vec3::new(0.5, 2.45, 0.1), 0.07, stone), 0.35));
            p.push(shiny(limb(Vec3::new(0.5, 2.0, 0.1), Vec3::new(0.5, 3.15, 0.1), 0.04, 0xc6c2b6), 0.7));
            p.push(shiny(block(Vec3::new(0.5, 2.05, 0.1), Vec3::new(0.14, 0.03, 0.04), Mat3::IDENTITY, 0xc6c2b6), 0.6));
            p.push(shiny(limb(Vec3::new(-0.28, 1.8, 0.0), Vec3::new(-0.4, 1.2, 0.2), 0.07, stone), 0.35));
            p.push(shiny(ellipsoid(Vec3::new(-0.4, 1.15, 0.3), Vec3::new(0.2, 0.3, 0.06), 0x9a968c), 0.4));
        }
        // Fontaine à deux vasques.
        _ => {
            let stone = 0xb4b0a4;
            p.push(shiny(cone(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.42, 0.0), 1.55, 1.4, stone), 0.3));
            p.push(shiny(cone(Vec3::new(0.0, 0.42, 0.0), Vec3::new(0.0, 0.52, 0.0), 1.5, 1.5, 0xc8c4b8), 0.3));
            p.push(shiny(ellipsoid(Vec3::new(0.0, 0.4, 0.0), Vec3::new(1.3, 0.04, 1.3), 0x3f7aa8), 0.6));
            p.push(shiny(cone(Vec3::new(0.0, 0.4, 0.0), Vec3::new(0.0, 1.35, 0.0), 0.32, 0.22, stone), 0.3));
            p.push(shiny(cone(Vec3::new(0.0, 1.2, 0.0), Vec3::new(0.0, 1.5, 0.0), 0.8, 0.6, 0xc8c4b8), 0.3));
            p.push(shiny(ellipsoid(Vec3::new(0.0, 1.48, 0.0), Vec3::new(0.62, 0.04, 0.62), 0x4a8ab8), 0.6));
            p.push(shiny(cone(Vec3::new(0.0, 1.5, 0.0), Vec3::new(0.0, 2.0, 0.0), 0.12, 0.05, stone), 0.3));
            p.push(glow(ball(Vec3::new(0.0, 2.1, 0.0), 0.1, 0xcfe8ff)));
            for k in 0..8 {
                let a = k as f32 * TAU / 8.0;
                p.push(glow(cone(Vec3::new(a.cos() * 0.6, 1.45, a.sin() * 0.6), Vec3::new(a.cos() * 1.0, 0.45, a.sin() * 1.0), 0.03, 0.02, 0x9ccbf0)));
            }
        }
    }
    // La fontaine est large: on la réduit pour qu'elle tienne dans sa case.
    if index == 3 {
        p = scaled(p, 0.5);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Camera, render};

    /// Avec `FORGE_PREVIEW=<dossier>`, écrit un aperçu de chaque motif (pour le regarder).
    #[test]
    fn every_landmark_draws_something() {
        let camera = Camera { yaw: 0.0, elevation: 0.42, scale: 40.0, size: 192, feet_y: 176.0, outline: true, shadow: true };
        for (index, name) in LANDMARKS.iter().enumerate() {
            let image = render(&build(index), &camera, 3);
            let covered = image.iter().filter(|p| p[3] > 0.5).count();
            assert!(covered > 2000, "{name}: {covered} pixels seulement");
            let n = 192;
            if let Some(dir) = std::env::var_os("FORGE_PREVIEW") {
                let bytes: Vec<u8> = image.iter().flat_map(|p| [p[0], p[1], p[2], p[3]].map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8)).collect();
                let path = std::path::Path::new(&dir).join(format!("{name}.png"));
                let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path).unwrap()), n as u32, n as u32);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&bytes).unwrap();
            }
            // Rien ne doit toucher les bords de la case.
            let edge = (0..n).any(|i| image[i][3] > 0.2 || image[(n - 1) * n + i][3] > 0.2 || image[i * n][3] > 0.2 || image[i * n + n - 1][3] > 0.2);
            assert!(!edge, "{name} dépasse de sa case: {:?}", (0..n).flat_map(|i| [(i, 0), (i, n - 1), (0, i), (n - 1, i)]).filter(|(x, y)| image[y * n + x][3] > 0.2).take(4).collect::<Vec<_>>());
        }
    }
}
