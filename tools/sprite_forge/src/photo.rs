//! Outils d'étude des photos de référence: découper et agrandir une zone, mesurer une couleur moyenne.
//!
//! Usage: sprite_forge crop <photo.png> x y largeur hauteur échelle <sortie.png>
//!        sprite_forge avg  <photo.png> x y largeur hauteur

use std::fs::File;
use std::path::Path;

/// Image RGB 8 bits décodée.
pub struct Photo {
    pub width: usize,
    pub height: usize,
    rgb: Vec<[u8; 3]>,
}

impl Photo {
    pub fn load(path: &Path) -> Self {
        let mut decoder = png::Decoder::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().expect("PNG invalide");
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).expect("décodage");
        let channels = match info.color_type {
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Indexed => panic!("PNG indexé non géré"),
        };
        let rgb = buffer
            .chunks(channels)
            .take(info.width as usize * info.height as usize)
            .map(|px| if channels < 3 { [px[0]; 3] } else { [px[0], px[1], px[2]] })
            .collect();
        Self { width: info.width as usize, height: info.height as usize, rgb }
    }

    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        self.rgb[y.min(self.height - 1) * self.width + x.min(self.width - 1)]
    }

    /// Couleur moyenne d'un rectangle.
    pub fn average(&self, x: usize, y: usize, w: usize, h: usize) -> [u8; 3] {
        let mut sum = [0u64; 3];
        let mut n = 0u64;
        for yy in y..(y + h).min(self.height) {
            for xx in x..(x + w).min(self.width) {
                let p = self.at(xx, yy);
                for c in 0..3 {
                    sum[c] += p[c] as u64;
                }
                n += 1;
            }
        }
        sum.map(|s| (s / n.max(1)) as u8)
    }

    /// Rectangle agrandi `scale` fois (au plus proche), en RGBA.
    pub fn crop(&self, x: usize, y: usize, w: usize, h: usize, scale: usize) -> (usize, usize, Vec<u8>) {
        let (ow, oh) = (w * scale, h * scale);
        let mut out = Vec::with_capacity(ow * oh * 4);
        for yy in 0..oh {
            for xx in 0..ow {
                let p = self.at(x + xx / scale, y + yy / scale);
                out.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        (ow, oh, out)
    }

    /// Carré de côté `side` ramené à `size` pixels par moyenne des pixels couverts, en RGBA.
    pub fn square(&self, x: usize, y: usize, side: usize, size: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(size * size * 4);
        for oy in 0..size {
            for ox in 0..size {
                let (x0, x1) = (x + ox * side / size, x + (ox + 1) * side / size);
                let (y0, y1) = (y + oy * side / size, y + (oy + 1) * side / size);
                let c = self.average(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1));
                out.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        out
    }
}

/// Détoure un portrait RGBA carré: le fond, lisse et flou sur les photos, est repéré par remplissage depuis les bords
/// (on s'arrête dès qu'un contour franchit `step`), puis tout ce qui sort d'un ovale est effacé, avec un bord adouci.
pub fn cutout(rgba: &mut [u8], size: usize, step: i32) {
    let diff = |a: usize, b: usize| (0..3).map(|c| (rgba[a * 4 + c] as i32 - rgba[b * 4 + c] as i32).abs()).sum::<i32>();
    let mut background = vec![false; size * size];
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..size {
        for e in [i, (size - 1) * size + i, i * size, i * size + size - 1] {
            if !background[e] {
                background[e] = true;
                stack.push(e);
            }
        }
    }
    while let Some(i) = stack.pop() {
        let (x, y) = (i % size, i / size);
        let neighbours = [(x > 0).then(|| i - 1), (x + 1 < size).then(|| i + 1), (y > 0).then(|| i - size), (y + 1 < size).then(|| i + size)];
        for n in neighbours.into_iter().flatten() {
            if !background[n] && diff(i, n) <= step {
                background[n] = true;
                stack.push(n);
            }
        }
    }
    let s = size as f32;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = ((x as f32 + 0.5) / s - 0.5, (y as f32 + 0.5) / s - 0.5);
            let d = ((dx / 0.5).powi(2) + (dy / 0.52).powi(2)).sqrt();
            let oval = ((0.98 - d) / 0.1).clamp(0.0, 1.0);
            let keep = if background[y * size + x] && d > 0.62 { 0.0 } else { 1.0 };
            let a = &mut rgba[(y * size + x) * 4 + 3];
            *a = (255.0 * oval.min(keep)) as u8;
        }
    }
}
