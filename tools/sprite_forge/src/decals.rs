//! Marques d'usure posées sur le sol par le jeu: taches douces, traînées et fissures.
//! Ce sont des masques blancs semi-transparents que le jeu teinte (suie, mousse, givre...) et étire sur
//! plusieurs tuiles, pour que l'usure ne se limite pas à une tuile isolée.

use crate::shapes::Rng;

/// Côté d'une case, en pixels.
pub const CELL: usize = 128;
pub const BLOTCHES: usize = 4;
pub const STREAKS: usize = 3;
pub const CRACKS: usize = 3;
pub const SPLATTERS: usize = 3;
pub const COLUMNS: usize = BLOTCHES + STREAKS + CRACKS + SPLATTERS;

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263) ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Bruit de valeur lissé.
fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(fx), s(fy));
    let a = hash(xi, yi, seed);
    let b = hash(xi + 1, yi, seed);
    let c = hash(xi, yi + 1, seed);
    let d = hash(xi + 1, yi + 1, seed);
    (a + (b - a) * u) + ((c + (d - c) * u) - (a + (b - a) * u)) * v
}

fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    0.5 * noise(x, y, seed) + 0.3 * noise(x * 2.0, y * 2.0, seed + 1) + 0.2 * noise(x * 4.0, y * 4.0, seed + 2)
}

fn smooth(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn blank() -> Vec<[f32; 4]> {
    vec![[1.0, 1.0, 1.0, 0.0]; CELL * CELL]
}

fn blotch(seed: u32) -> Vec<[f32; 4]> {
    let mut img = blank();
    for y in 0..CELL {
        for x in 0..CELL {
            let (u, v) = ((x as f32 - 64.0) / 64.0, (y as f32 - 64.0) / 64.0);
            let r = (u * u + v * v).sqrt();
            let n = fbm(x as f32 / 22.0, y as f32 / 22.0, seed);
            img[y * CELL + x][3] = smooth(1.0, 0.25, r + (n - 0.5) * 0.9).powf(1.3);
        }
    }
    img
}

fn streak(seed: u32) -> Vec<[f32; 4]> {
    let mut img = blank();
    for y in 0..CELL {
        for x in 0..CELL {
            let (u, v) = ((x as f32 - 64.0) / 64.0, (y as f32 - 64.0) / 24.0);
            let r = (u * u + v * v).sqrt();
            let n = fbm(x as f32 / 18.0, y as f32 / 8.0, seed);
            img[y * CELL + x][3] = smooth(1.0, 0.15, r + (n - 0.5) * 0.9).powf(1.2);
        }
    }
    img
}

/// Fissure: une marche aléatoire qui traverse la case, avec deux ramifications, légèrement estompée.
fn crack(seed: u32) -> Vec<[f32; 4]> {
    let mut rng = Rng(seed as u64 * 977 + 5);
    let mut img = blank();
    let mut branches = vec![(8.0f32, 64.0 + rng.spread(14.0), rng.spread(0.35), 118)];
    let mut spawned = 0;
    while let Some((mut x, mut y, mut angle, steps)) = branches.pop() {
        for i in 0..steps {
            angle += rng.spread(0.3);
            x += angle.cos() * 1.4;
            y += angle.sin() * 1.4;
            if !(2.0..CELL as f32 - 2.0).contains(&x) || !(2.0..CELL as f32 - 2.0).contains(&y) {
                break;
            }
            for (dx, dy, a) in [(0, 0, 1.0f32), (1, 0, 0.55), (-1, 0, 0.55), (0, 1, 0.55), (0, -1, 0.55)] {
                let (px, py) = ((x as i32 + dx) as usize, (y as i32 + dy) as usize);
                let cell = &mut img[py * CELL + px][3];
                *cell = cell.max(a * (1.0 - i as f32 / steps as f32 * 0.4));
            }
            if spawned < 2 && i > 20 && i % 31 == 0 && rng.next() < 0.7 {
                spawned += 1;
                branches.push((x, y, angle + rng.spread(1.1) + 0.5, 40));
            }
        }
    }
    img
}

/// Éclaboussure: une flaque centrale irrégulière entourée de gouttes projetées et de traînées.
fn splatter(seed: u32) -> Vec<[f32; 4]> {
    let mut rng = Rng(seed as u64 * 131 + 9);
    let mut img = blank();
    let stamp = |img: &mut Vec<[f32; 4]>, cx: f32, cy: f32, radius: f32, stretch: f32, angle: f32| {
        let (sa, ca) = angle.sin_cos();
        let reach = (radius * stretch.max(1.0) + 2.0) as i32;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (x, y) = ((cx as i32 + dx), (cy as i32 + dy));
                if x < 0 || y < 0 || x >= CELL as i32 || y >= CELL as i32 {
                    continue;
                }
                let (u, v) = (dx as f32 * ca + dy as f32 * sa, -(dx as f32) * sa + dy as f32 * ca);
                let d = ((u / (radius * stretch)).powi(2) + (v / radius).powi(2)).sqrt();
                let a = smooth(1.0, 0.7, d);
                let cell = &mut img[y as usize * CELL + x as usize][3];
                *cell = cell.max(a);
            }
        }
    };
    stamp(&mut img, 64.0, 64.0, 22.0 + rng.next() * 6.0, 1.0, 0.0);
    for _ in 0..5 {
        let a = rng.next() * std::f32::consts::TAU;
        stamp(&mut img, 64.0 + a.cos() * 22.0, 64.0 + a.sin() * 22.0, 10.0 + rng.next() * 6.0, 1.0, 0.0);
    }
    for _ in 0..16 {
        let a = rng.next() * std::f32::consts::TAU;
        let dist = 30.0 + rng.next() * 30.0;
        stamp(&mut img, 64.0 + a.cos() * dist, 64.0 + a.sin() * dist, 2.0 + rng.next() * 4.0, 1.0 + rng.next() * 2.5, a);
    }
    img
}

pub fn cells() -> Vec<Vec<[f32; 4]>> {
    let mut cells = Vec::new();
    for k in 0..BLOTCHES {
        cells.push(blotch(11 + k as u32 * 7));
    }
    for k in 0..STREAKS {
        cells.push(streak(31 + k as u32 * 5));
    }
    for k in 0..CRACKS {
        cells.push(crack(51 + k as u32 * 3));
    }
    for k in 0..SPLATTERS {
        cells.push(splatter(71 + k as u32 * 5));
    }
    cells
}
