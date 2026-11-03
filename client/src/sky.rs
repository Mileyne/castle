//! Le ciel de la vue à la première personne: un dôme autour de la caméra, sans éclairage, avec une texture générée ici. Dans les forêts, une
//! nuit étoilée (étoiles de brillances variées, quelques nuages bleutés qui se fondent dans l'ombre comme de la brume). Les autres lieux
//! gardent le noir pour l'instant.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    camera::visibility::RenderLayers,
};
use castle_shared::{Expedition, Region};

use crate::decor3d::{DecorCamera, LAYER};
use crate::firstperson::FirstPerson;

/// Rayon du dôme, en unités du monde (en deçà de la portée de la caméra, qui est de 6000).
const SKY_RADIUS: f32 = 4500.0;
/// Taille de la texture du ciel (équirectangulaire: 360° en largeur, 180° en hauteur).
const SKY_WIDTH: usize = 3072;
const SKY_HEIGHT: usize = SKY_WIDTH / 2;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_sky.after(crate::decor3d::follow_camera));
    }
}

#[derive(Component)]
struct Sky;

/// Hachage entier vers [0, 1): des variations stables (une même étoile reste toujours à sa place).
fn hash01(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA6B) ^ (z as u32).wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// Bruit de valeur en trois dimensions (continu, sans couture sur la sphère puisqu'il lit la direction et non l'image).
fn value_noise(p: Vec3) -> f32 {
    let cell = p.floor();
    let f = p - cell;
    let f = f * f * (3.0 - 2.0 * f);
    let (x, y, z) = (cell.x as i32, cell.y as i32, cell.z as i32);
    let corner = |dx: i32, dy: i32, dz: i32| hash01(x + dx, y + dy, z + dz);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), f.x);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), f.x);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), f.x);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), f.x);
    lerp(lerp(x00, x10, f.y), lerp(x01, x11, f.y), f.z)
}

fn fbm(p: Vec3) -> f32 {
    let (mut sum, mut amplitude, mut scale) = (0.0, 0.5, 1.0);
    for _ in 0..4 {
        sum += amplitude * value_noise(p * scale);
        amplitude *= 0.5;
        scale *= 2.03;
    }
    sum
}

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// La couleur (RVB, 0 à 255) du ciel étoilé dans la direction `dir` (unitaire, Y vers le haut).
fn night_sky(dir: Vec3, column: i32, row: i32) -> [u8; 3] {
    let up = dir.y.clamp(-1.0, 1.0);
    // Dégradé: bleu nuit profond au zénith, un peu plus clair et plus vert près de l'horizon.
    let near_horizon = (1.0 - up.abs()).powf(2.2);
    let mut color = Vec3::new(5.0, 8.0, 26.0).lerp(Vec3::new(20.0, 34.0, 64.0), near_horizon);
    // Nuages: du bruit en gros plans, plus denses vers l'horizon, très doux (de la brume plus que des formes nettes).
    let cloud = smoothstep(0.52, 0.86, fbm(dir * 2.6 + Vec3::new(3.0, 1.0, 7.0))) * (0.35 + 0.65 * near_horizon);
    // Étoiles: un point par pixel au plus, plus rares aux pôles de l'image (où les pixels sont trop nombreux) pour une densité régulière.
    let density = 1.0 / 520.0 * (1.0 - up * up).sqrt().max(0.05);
    let roll = hash01(column, row, 11);
    if roll < density {
        let brightness = 0.35 + 0.65 * hash01(column, row, 29).powi(2);
        let tint = Vec3::new(0.85 + 0.15 * hash01(column, row, 41), 0.9, 1.0 - 0.18 * hash01(column, row, 53));
        // Les nuages voilent les étoiles.
        color += tint * 255.0 * brightness * (1.0 - 0.9 * cloud);
    }
    // Brume des nuages: une lueur bleutée qui se mêle au ciel.
    color = color.lerp(Vec3::new(78.0, 96.0, 140.0), cloud * 0.55);
    [color.x.clamp(0.0, 255.0) as u8, color.y.clamp(0.0, 255.0) as u8, color.z.clamp(0.0, 255.0) as u8]
}

/// L'image du ciel étoilé: RVBA, équirectangulaire (la première ligne est le zénith).
fn night_sky_image() -> Image {
    let mut data = vec![255u8; SKY_WIDTH * SKY_HEIGHT * 4];
    for row in 0..SKY_HEIGHT {
        // Latitude de la ligne, du zénith (+90°) au nadir (-90°).
        let lat = (0.5 - (row as f32 + 0.5) / SKY_HEIGHT as f32) * std::f32::consts::PI;
        for column in 0..SKY_WIDTH {
            let lon = ((column as f32 + 0.5) / SKY_WIDTH as f32) * std::f32::consts::TAU;
            let dir = Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos());
            let [r, g, b] = night_sky(dir, column as i32, row as i32);
            let at = (row * SKY_WIDTH + column) * 4;
            data[at..at + 3].copy_from_slice(&[r, g, b]);
        }
    }
    Image::new(
        Extent3d { width: SKY_WIDTH as u32, height: SKY_HEIGHT as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Montre le ciel dans les forêts à la première personne, centré sur la caméra; le crée à la première demande (la texture se calcule alors).
fn update_sky(
    mut commands: Commands,
    view: Res<FirstPerson>,
    expeditions: Query<&Expedition>,
    camera: Single<&Transform, (With<DecorCamera>, Without<Sky>)>,
    mut sky: Query<(&mut Transform, &mut Visibility), With<Sky>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let wanted = view.on && expeditions.iter().any(|expedition| expedition.region == Region::Forest);
    match sky.single_mut() {
        Ok((mut transform, mut visibility)) => {
            transform.translation = camera.translation;
            let state = if wanted { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != state {
                *visibility = state;
            }
        }
        Err(_) if wanted => {
            let texture = images.add(night_sky_image());
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                unlit: true,
                // On est à l'intérieur du dôme: on en voit la face interne.
                cull_mode: None,
                fog_enabled: false,
                ..default()
            });
            commands.spawn((
                Sky,
                Mesh3d(meshes.add(Sphere::new(SKY_RADIUS).mesh().uv(64, 32))),
                MeshMaterial3d(material),
                Transform::from_translation(camera.translation),
                RenderLayers::layer(LAYER),
            ));
        }
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le ciel de nuit est sombre, avec des étoiles brillantes ici et là, et ses nuages ne sont que de la brume (jamais blancs).
    #[test]
    fn the_night_sky_is_dark_with_stars_and_soft_clouds() {
        let (mut bright, mut total) = (0, 0);
        let (mut cloudy, mut sum) = (0, 0u64);
        for row in (0..SKY_HEIGHT as i32).step_by(7) {
            let lat = (0.5 - (row as f32 + 0.5) / SKY_HEIGHT as f32) * std::f32::consts::PI;
            for column in (0..SKY_WIDTH as i32).step_by(5) {
                let lon = ((column as f32 + 0.5) / SKY_WIDTH as f32) * std::f32::consts::TAU;
                let dir = Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos());
                let [r, g, b] = night_sky(dir, column, row);
                let level = (r as u32 + g as u32 + b as u32) / 3;
                total += 1;
                sum += level as u64;
                if level > 150 {
                    bright += 1;
                }
                if b > 100 && level < 150 {
                    cloudy += 1;
                }
            }
        }
        let average = sum / total as u64;
        assert!(average < 60, "un ciel sombre: {average}");
        assert!(bright > 0 && bright * 20 < total, "quelques étoiles seulement: {bright} sur {total}");
        assert!(cloudy > 0, "des nuages bleutés");
    }

    #[test]
    fn the_sky_image_has_the_expected_size() {
        let image = night_sky_image();
        assert_eq!((image.width() as usize, image.height() as usize), (SKY_WIDTH, SKY_HEIGHT));
    }
}
