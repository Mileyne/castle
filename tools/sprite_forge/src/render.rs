//! Petit moteur de rendu: des volumes (sphères, capsules, boîtes, cônes) vus en projection orthographique,
//! éclairés par paliers (toon) et cernés d'un contour, pour obtenir un rendu pixel-art détaillé.

use glam::{Mat3, Vec3};

const EPS: f32 = 1e-4;

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// Sphère de rayon 1 (déformée par `scale` en ellipsoïde).
    Sphere,
    /// Cube de demi-côté 1.
    Cube,
    /// Capsule le long de Y: rayon 1 et demi-longueur de cylindre `h`.
    Capsule(f32),
    /// Tronc de cône le long de Y, de y = -1 (rayon r0) à y = 1 (rayon r1), fermé par deux disques.
    Frustum(f32, f32),
}

#[derive(Clone, Debug)]
pub struct Part {
    pub shape: Shape,
    pub center: Vec3,
    pub rot: Mat3,
    pub scale: Vec3,
    pub color: [f32; 3],
    /// Les parties lumineuses (orbe, étincelles) ne sont pas ombrées.
    pub glow: bool,
    /// Force du reflet brillant (cheveux, métal), de 0 (mat) à 1.
    pub shine: f32,
}

type Hit = Option<(f32, Vec3)>;

fn nearest(a: Hit, b: Hit) -> Hit {
    match (a, b) {
        (Some(x), Some(y)) => Some(if x.0 <= y.0 { x } else { y }),
        (x, None) => x,
        (None, y) => y,
    }
}

fn hit_sphere(o: Vec3, d: Vec3, c: Vec3, r: f32) -> Hit {
    let oc = o - c;
    let a = d.dot(d);
    let b = oc.dot(d);
    let cc = oc.dot(oc) - r * r;
    let disc = b * b - a * cc;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    [(-b - sq) / a, (-b + sq) / a]
        .into_iter()
        .find(|t| *t > EPS)
        .map(|t| (t, o + d * t - c))
}

fn hit_cube(o: Vec3, d: Vec3) -> Hit {
    let (mut tmin, mut tmax) = (f32::NEG_INFINITY, f32::INFINITY);
    let mut normal = Vec3::ZERO;
    for axis in 0..3 {
        if d[axis].abs() < 1e-9 {
            if o[axis].abs() > 1.0 {
                return None;
            }
            continue;
        }
        let (t1, t2) = ((-1.0 - o[axis]) / d[axis], (1.0 - o[axis]) / d[axis]);
        let (near, far, sign) = if t1 < t2 { (t1, t2, -1.0) } else { (t2, t1, 1.0) };
        if near > tmin {
            tmin = near;
            normal = Vec3::ZERO;
            normal[axis] = sign;
        }
        tmax = tmax.min(far);
    }
    (tmin <= tmax && tmin > EPS).then_some((tmin, normal))
}

fn hit_capsule(o: Vec3, d: Vec3, h: f32) -> Hit {
    let mut best: Hit = None;
    let a = d.x * d.x + d.z * d.z;
    if a > 1e-12 {
        let b = o.x * d.x + o.z * d.z;
        let c = o.x * o.x + o.z * o.z - 1.0;
        let disc = b * b - a * c;
        if disc >= 0.0 {
            let sq = disc.sqrt();
            for t in [(-b - sq) / a, (-b + sq) / a] {
                let p = o + d * t;
                if t > EPS && p.y.abs() <= h {
                    best = nearest(best, Some((t, Vec3::new(p.x, 0.0, p.z))));
                    break;
                }
            }
        }
    }
    let top = hit_sphere(o, d, Vec3::new(0.0, h, 0.0), 1.0).filter(|(t, _)| (o + d * *t).y >= h);
    let bottom = hit_sphere(o, d, Vec3::new(0.0, -h, 0.0), 1.0).filter(|(t, _)| (o + d * *t).y <= -h);
    nearest(best, nearest(top, bottom))
}

fn hit_frustum(o: Vec3, d: Vec3, r0: f32, r1: f32) -> Hit {
    let (rm, k) = ((r0 + r1) / 2.0, (r1 - r0) / 2.0);
    let mut best: Hit = None;
    // Surface latérale: x² + z² = (rm + k·y)²
    let a = d.x * d.x + d.z * d.z - k * k * d.y * d.y;
    let r_o = rm + k * o.y;
    let b = o.x * d.x + o.z * d.z - k * d.y * r_o;
    let c = o.x * o.x + o.z * o.z - r_o * r_o;
    if a.abs() > 1e-12 {
        let disc = b * b - a * c;
        if disc >= 0.0 {
            let sq = disc.sqrt();
            let mut ts = [(-b - sq) / a, (-b + sq) / a];
            ts.sort_by(|x, y| x.total_cmp(y));
            for t in ts {
                let p = o + d * t;
                if t > EPS && p.y.abs() <= 1.0 && rm + k * p.y >= 0.0 {
                    best = Some((t, Vec3::new(p.x, -k * (rm + k * p.y), p.z)));
                    break;
                }
            }
        }
    }
    // Disques du haut et du bas.
    if d.y.abs() > 1e-9 {
        for (y, r) in [(-1.0, r0), (1.0, r1)] {
            let t = (y - o.y) / d.y;
            let p = o + d * t;
            if t > EPS && p.x * p.x + p.z * p.z <= r * r {
                best = nearest(best, Some((t, Vec3::new(0.0, y, 0.0))));
            }
        }
    }
    best
}

impl Part {
    /// Rayon de la sphère qui contient la pièce, pour écarter vite les pixels sans rapport.
    fn bound(&self) -> f32 {
        let s = self.scale;
        match self.shape {
            Shape::Sphere => s.max_element(),
            Shape::Cube => s.length(),
            Shape::Capsule(h) => (s.y * h + s.x.max(s.z)).max(s.max_element()) + s.max_element(),
            Shape::Frustum(r0, r1) => Vec3::new(s.x * r0.max(r1), s.y, s.z * r0.max(r1)).length(),
        }
    }

    /// Intersection d'un rayon (origine, direction, en repère du modèle) avec la pièce: (t, normale monde).
    fn hit(&self, o: Vec3, d: Vec3) -> Hit {
        let inv = self.rot.transpose();
        let lo = (inv * (o - self.center)) / self.scale;
        let ld = (inv * d) / self.scale;
        let (t, n) = match self.shape {
            Shape::Sphere => hit_sphere(lo, ld, Vec3::ZERO, 1.0),
            Shape::Cube => hit_cube(lo, ld),
            Shape::Capsule(h) => hit_capsule(lo, ld, h),
            Shape::Frustum(r0, r1) => hit_frustum(lo, ld, r0, r1),
        }?;
        // La normale se transforme par l'inverse transposée: on divise par l'échelle.
        Some((t, (self.rot * (n / self.scale)).normalize_or_zero()))
    }
}

pub struct Camera {
    /// Rotation du héros autour de l'axe vertical (0 = de face, vers le spectateur).
    pub yaw: f32,
    /// Inclinaison de la vue (vue légèrement plongeante).
    pub elevation: f32,
    /// Pixels par unité du modèle.
    pub scale: f32,
    /// Côté de l'image carrée en pixels.
    pub size: usize,
    /// Position de l'origine du modèle (le pied du héros) dans l'image, en pixels depuis le haut.
    pub feet_y: f32,
    /// Contour sombre autour de la silhouette, et ombre portée au sol (inutiles pour les tuiles).
    pub outline: bool,
    pub shadow: bool,
}

const OUTLINE: [f32; 3] = [0.08, 0.06, 0.12];

/// Rend les pièces et renvoie une image RGBA (alpha droit, composantes de 0 à 1) de `cam.size` pixels de côté.
pub fn render(parts: &[Part], cam: &Camera, supersample: usize) -> Vec<[f32; 4]> {
    let ss = supersample;
    let w = cam.size * ss;
    let rot = Mat3::from_rotation_x(cam.elevation) * Mat3::from_rotation_y(cam.yaw);
    let inv = rot.transpose();
    let light = Vec3::new(-0.45, 0.7, 0.55).normalize();
    let ray_dir = inv * Vec3::new(0.0, 0.0, -1.0);
    let s = cam.scale * ss as f32;
    let (cx, cy) = (w as f32 / 2.0, cam.feet_y * ss as f32);

    let mut depth = vec![f32::INFINITY; w * w];
    let mut id = vec![u16::MAX; w * w];
    let mut color = vec![[0.0f32; 3]; w * w];

    for (index, part) in parts.iter().enumerate() {
        let c = rot * part.center;
        let r = part.bound();
        let (x0, x1) = (((cx + (c.x - r) * s).floor().max(0.0)) as usize, ((cx + (c.x + r) * s).ceil().min(w as f32)) as usize);
        let (y0, y1) = (((cy - (c.y + r) * s).floor().max(0.0)) as usize, ((cy - (c.y - r) * s).ceil().min(w as f32)) as usize);
        for py in y0..y1 {
            for px in x0..x1 {
                let u = (px as f32 + 0.5 - cx) / s;
                let v = (cy - (py as f32 + 0.5)) / s;
                let origin = inv * Vec3::new(u, v, 20.0);
                let Some((t, n_model)) = part.hit(origin, ray_dir) else {
                    continue;
                };
                let i = py * w + px;
                if t >= depth[i] {
                    continue;
                }
                depth[i] = t;
                id[i] = index as u16;
                let n = rot * n_model;
                let tone = if part.glow {
                    1.15
                } else {
                    let ndl = n.dot(light);
                    let band = if ndl > 0.5 {
                        1.08
                    } else if ndl > 0.1 {
                        0.9
                    } else if ndl > -0.25 {
                        // Ombre moyenne, hachurée en diagonale comme une trame de manga.
                        if (px + py) / ss.max(1) / 2 % 2 == 0 { 0.7 } else { 0.78 }
                    } else if (px + py) / ss.max(1) / 2 % 2 == 0 {
                        0.5
                    } else {
                        0.6
                    };
                    // Les bords qui fuient vers l'arrière s'assombrissent un peu.
                    if n.z < 0.2 { band * 0.92 } else { band }
                };
                let mut rgb = [
                    (part.color[0] * tone).min(1.0),
                    (part.color[1] * tone).min(1.0),
                    (part.color[2] * tone).min(1.0),
                ];
                // Reflet blanc net (cheveux, métal): petite tache là où la normale fait face à la lumière.
                if part.shine > 0.0 && !part.glow {
                    let half = (light + Vec3::Z).normalize();
                    if n.dot(half) > 0.93 {
                        for ch in &mut rgb {
                            *ch += (1.0 - *ch) * part.shine;
                        }
                    }
                }
                color[i] = rgb;
            }
        }
    }

    // Lignes de séparation entre pièces: on assombrit le côté éloigné d'une frontière.
    let mut shaded = color.clone();
    for py in 1..w - 1 {
        for px in 1..w - 1 {
            let i = py * w + px;
            if id[i] == u16::MAX {
                continue;
            }
            let nearer = [i - 1, i + 1, i - w, i + w]
                .into_iter()
                .any(|j| id[j] != u16::MAX && id[j] != id[i] && depth[j] < depth[i] - 0.04);
            if nearer {
                for ch in &mut shaded[i] {
                    *ch *= 0.45;
                }
            }
        }
    }

    // Réduction (moyenne des sous-pixels) et couverture en alpha.
    let n = cam.size;
    let mut out = vec![[0.0f32; 4]; n * n];
    for y in 0..n {
        for x in 0..n {
            let (mut sum, mut cover) = ([0.0f32; 3], 0.0f32);
            for sy in 0..ss {
                for sx in 0..ss {
                    let i = (y * ss + sy) * w + x * ss + sx;
                    if id[i] != u16::MAX {
                        cover += 1.0;
                        for ch in 0..3 {
                            sum[ch] += shaded[i][ch];
                        }
                    }
                }
            }
            if cover > 0.0 {
                out[y * n + x] = [sum[0] / cover, sum[1] / cover, sum[2] / cover, cover / (ss * ss) as f32];
            }
        }
    }

    // Contour sombre autour de la silhouette.
    let outline = cam.outline;
    let solid: Vec<bool> = out.iter().map(|p| p[3] > 0.5).collect();
    for y in 0..n {
        for x in 0..n {
            let i = y * n + x;
            if !outline || out[i][3] > 0.5 {
                continue;
            }
            let near = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].into_iter().any(|(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                nx >= 0 && ny >= 0 && (nx as usize) < n && (ny as usize) < n && solid[ny as usize * n + nx as usize]
            });
            if near {
                // Trait d'encre teinté: la couleur du voisin, très assombrie.
                let neighbor = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].into_iter().find_map(|(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    (nx >= 0 && ny >= 0 && (nx as usize) < n && (ny as usize) < n && solid[ny as usize * n + nx as usize])
                        .then(|| out[ny as usize * n + nx as usize])
                });
                let base = neighbor.unwrap_or([OUTLINE[0], OUTLINE[1], OUTLINE[2], 1.0]);
                out[i] = [base[0] * 0.22 + 0.03, base[1] * 0.22 + 0.02, base[2] * 0.25 + 0.05, 1.0];
            }
        }
    }

    // Ombre portée douce sous les pieds.
    if !cam.shadow {
        return out;
    }
    let (ex, ey) = (n as f32 / 2.0, cam.feet_y + 1.0);
    for y in 0..n {
        for x in 0..n {
            let i = y * n + x;
            if out[i][3] > 0.0 {
                continue;
            }
            let (dx, dy) = ((x as f32 + 0.5 - ex) / (cam.scale * 0.5), (y as f32 + 0.5 - ey) / (cam.scale * 0.17));
            let d = dx * dx + dy * dy;
            if d < 1.0 {
                out[i] = [0.0, 0.0, 0.0, 0.32 * (1.0 - d)];
            }
        }
    }
    out
}
