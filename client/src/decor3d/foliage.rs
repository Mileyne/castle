//! Le motif du feuillage, cuit une fois pour toutes dans une texture en trois dimensions.
//!
//! Le shader calculait à chaque pixel un diagramme cellulaire (27 cases, trois nombres au hasard par case): c'était près de la moitié
//! du coût d'image sur les forêts. Le motif ne dépend que de la position dans le monde, au pixel de l'art: on le calcule ici une fois
//! (même formules que l'ancien `leaves` du shader), et le shader n'a plus qu'à lire un texel. Le motif se répète tous les 64 unités
//! (4 tuiles) en largeur et en profondeur, 32 en hauteur (la hauteur d'un mur); les cases sont tirées modulo cette période.
//!
//! Deux octets par texel: la position dans le dégradé feuillage (couleur claire / sombre) et l'ombrage (÷ `SHADE_MAX`).

use std::sync::OnceLock;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use castle_shared::{TILE_SIZE, tileart::TILE_CELL};

/// Taille d'une case du diagramme, en unités du monde.
const CELL: f32 = 8.0;
/// Période en cases: largeur, hauteur, profondeur.
const PERIOD: [i32; 3] = [8, 4, 8];
/// Les plus grands ombrages possibles (ombre, feuilles claires, grain): la valeur 255 de l'octet d'ombrage.
const SHADE_MAX: f32 = 1.5;

/// Côté d'un pixel de l'art, en unités du monde.
fn pixel() -> f32 {
    TILE_SIZE / TILE_CELL as f32
}

/// Dimensions de la texture, en texels (un texel = un pixel de l'art). Le shader (`FOLIAGE_DIMS`) doit avoir les mêmes.
fn dims() -> [i32; 3] {
    let per_cell = (CELL / pixel()).round() as i32;
    PERIOD.map(|cells| cells * per_cell)
}

fn fract(x: f32) -> f32 {
    x - x.floor()
}

fn hash31(p: Vec3) -> f32 {
    let mut q = Vec3::new(fract(p.x * 0.1031), fract(p.y * 0.1030), fract(p.z * 0.0973));
    q += q.dot(Vec3::new(q.y, q.z, q.x) + Vec3::splat(33.33));
    fract((q.x + q.y) * q.z)
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Le centre le plus proche (distance, numéro propre à la case, vecteur vers le centre).
fn cell_of(p: Vec3) -> (f32, f32, Vec3) {
    let g = p / CELL;
    let i = g.floor();
    let f = g - i;
    let wrap = |v: Vec3| Vec3::new(v.x.rem_euclid(PERIOD[0] as f32), v.y.rem_euclid(PERIOD[1] as f32), v.z.rem_euclid(PERIOD[2] as f32));
    let mut best = (64.0f32, Vec3::ZERO, Vec3::ZERO);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = Vec3::new(dx as f32, dy as f32, dz as f32);
                let at = wrap(i + c);
                let r = Vec3::new(hash31(at), hash31(at + Vec3::new(31.7, 17.3, 5.9)), hash31(at + Vec3::new(7.1, 53.3, 23.7)));
                let to = c + r - f;
                let d = to.length_squared();
                if d < best.0 {
                    best = (d, c, to);
                }
            }
        }
    }
    (best.0.sqrt(), hash31(wrap(i + best.1) + Vec3::new(5.1, 9.7, 41.3)), best.2)
}

/// Le texel (x, hauteur, y du monde 2D), en indices de pixel de l'art.
fn texel(k: [i32; 3]) -> [u8; 2] {
    let q = Vec3::new(k[0] as f32, k[1] as f32, k[2] as f32) * pixel();
    let (dist, id, toward) = cell_of(q);
    // Le soleil vient du nord-ouest et d'en haut: la touffe est éclairée du côté qui lui fait face.
    let sun = Vec3::new(-0.45, 0.75, 0.5).normalize();
    let lit = (0.5 - toward.dot(sun)).clamp(0.0, 1.0);
    let t = (id * 0.55 + lit * 0.6).clamp(0.0, 1.0);
    // Entre deux touffes, c'est l'ombre; quelques feuilles claires sur la partie éclairée; un peu de grain.
    let mut shade = 1.0 - 0.55 * smoothstep(0.38, 0.72, dist);
    let at = Vec3::new(k[0] as f32, k[1] as f32, k[2] as f32);
    if lit > 0.55 && hash31(at) > 0.93 {
        shade *= 1.35;
    }
    shade *= 0.94 + 0.12 * hash31(at + Vec3::new(3.0, 1.0, 2.0));
    [(t * 255.0).round() as u8, (shade / SHADE_MAX * 255.0).round().clamp(0.0, 255.0) as u8]
}

fn bake() -> Vec<u8> {
    let [w, h, d] = dims();
    let mut data = vec![0u8; (w * h * d * 2) as usize];
    let layer = (w * h * 2) as usize;
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    // Une tranche de profondeur par tâche, réparties entre les fils.
    let slices: Vec<(i32, &mut [u8])> = data.chunks_mut(layer).enumerate().map(|(z, s)| (z as i32, s)).collect();
    let mut groups: Vec<Vec<(i32, &mut [u8])>> = (0..workers).map(|_| Vec::new()).collect();
    for (n, slice) in slices.into_iter().enumerate() {
        groups[n % workers].push(slice);
    }
    std::thread::scope(|scope| {
        for group in groups {
            scope.spawn(move || {
                for (z, slice) in group {
                    for y in 0..h {
                        for x in 0..w {
                            let at = ((y * w + x) * 2) as usize;
                            slice[at..at + 2].copy_from_slice(&texel([x, y, z]));
                        }
                    }
                }
            });
        }
    });
    data
}

static PATTERN: OnceLock<Handle<Image>> = OnceLock::new();

/// La texture du motif de feuillage (une seule pour tout le jeu, cuite à la première demande).
pub(super) fn pattern(images: &mut Assets<Image>) -> Handle<Image> {
    PATTERN
        .get_or_init(|| {
            let [w, h, d] = dims();
            images.add(Image::new(
                Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: d as u32 },
                TextureDimension::D3,
                bake(),
                TextureFormat::Rg8Unorm,
                RenderAssetUsages::RENDER_WORLD,
            ))
        })
        .clone()
}
