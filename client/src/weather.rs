//! Météo des deux premiers extérieurs (forêt et marais): une pluie battante, un ciel plus sombre (un voile translucide sur
//! toute la vue), et de temps en temps un éclair qui éclaire tout d'un coup, très brièvement, deux fois de suite.

use bevy::{
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d},
};
use castle_shared::{Expedition, Region};

/// Vitesse de chute (unités du monde par seconde) et dérive du vent. Le trait mesure 0,35 x 7 unités (voir `rain.wgsl`).
const FALL_SPEED: f32 = 300.0;
const WIND: f32 = 70.0;
/// Côté, en unités du monde, de la cellule qui contient une goutte (une couche): deux couches donnent environ une goutte pour
/// 290 unités carrées. Plus grand sur les petits appareils, pour moins de gouttes.
const RAIN_CELL: f32 = 24.0;
const RAIN_CELL_LITE: f32 = 34.0;
/// Profondeur de la pluie: devant tout le décor (la caméra est en z = 0, le décor entre 0 et 2).
const RAIN_Z: f32 = 950.0;
/// Opacité du voile sombre sous la pluie (forêt, marais), et force maximale d'un éclair (1 = il efface tout le voile et
/// blanchit l'image).
const GLOOM: [(Region, f32); 2] = [(Region::Forest, 0.28), (Region::Marsh, 0.36)];
const FLASH_STRENGTH: f32 = 1.0;
const STRIKE_GAP: (f32, f32) = (7.0, 16.0);

/// La région est-elle sous la pluie ?
pub fn is_rainy(region: Region) -> bool {
    matches!(region, Region::Forest | Region::Marsh)
}

/// État de l'orage; `flash` (0 à 1) dit à quel point un éclair éclaire la scène en ce moment.
#[derive(Resource, Default)]
pub struct Weather {
    pub flash: f32,
    until_strike: f32,
    strike: Option<f32>,
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Weather>()
            .add_plugins(Material2dPlugin::<RainMaterial>::default())
            .add_systems(Update, (update_storm, update_rain).chain());
    }
}

/// Intensité d'un éclair `t` secondes après son début: un premier éclat bref, une brève accalmie, un second éclat plus
/// long qui s'éteint.
fn envelope(t: f32) -> f32 {
    let pulse = |start: f32, rise: f32, fall: f32, peak: f32| {
        if t < start || t > start + rise + fall {
            0.0
        } else if t < start + rise {
            peak * (t - start) / rise
        } else {
            peak * (1.0 - (t - start - rise) / fall)
        }
    };
    pulse(0.0, 0.02, 0.07, 1.0).max(pulse(0.17, 0.03, 0.2, 0.8))
}

fn update_storm(
    time: Res<Time>,
    expeditions: Query<&Expedition>,
    mut weather: ResMut<Weather>,
    mut seed: Local<u32>,
) {
    let rainy = expeditions.iter().last().is_some_and(|e| is_rainy(e.region));
    if !rainy {
        *weather = Weather::default();
        return;
    }
    let dt = time.delta_secs();
    // Un tirage pseudo-aléatoire simple pour espacer les éclairs.
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223).wrapping_add((time.elapsed_secs() * 1000.0) as u32);
    let roll = ((*seed >> 8) & 0xffff) as f32 / 65535.0;
    if weather.until_strike <= 0.0 {
        weather.until_strike = STRIKE_GAP.0 + (STRIKE_GAP.1 - STRIKE_GAP.0) * roll;
    }
    weather.until_strike -= dt;
    if weather.until_strike <= 0.0 && weather.strike.is_none() {
        weather.strike = Some(0.0);
        weather.until_strike = STRIKE_GAP.0 + (STRIKE_GAP.1 - STRIKE_GAP.0) * roll;
    }
    if let Some(t) = weather.strike.as_mut() {
        *t += dt;
    }
    let t = weather.strike.unwrap_or(0.0);
    weather.flash = if weather.strike.is_some() { envelope(t) * FLASH_STRENGTH } else { 0.0 };
    if weather.strike.is_some() && t > 0.6 {
        weather.strike = None;
        weather.flash = 0.0;
    }
}

/// Le voile sombre de l'orage: sa couleur suit la force de l'éclair en cours.
#[derive(Component)]
struct Gloom;

/// La pluie: un seul rectangle qui couvre la vue, dessiné par `assets/shaders/rain.wgsl`.
#[derive(Component)]
struct Rain;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct RainMaterial {
    /// (côté d'une cellule, vent, vitesse de chute, opacité).
    #[uniform(0)]
    data: Vec4,
}

impl Material2d for RainMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rain.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Pose la pluie et le voile sombre de l'orage sur la vue, et les retire hors des régions pluvieuses.
fn update_rain(
    mut commands: Commands,
    expeditions: Query<&Expedition>,
    camera: Single<Entity, With<Camera2d>>,
    rain: Query<Entity, With<Rain>>,
    mut gloom: Query<(Entity, &mut Sprite), With<Gloom>>,
    weather: Res<Weather>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RainMaterial>>,
) {
    let region = expeditions.iter().last().map(|e| e.region).filter(|r| is_rainy(*r));
    let Some(region) = region else {
        for entity in rain.iter().chain(gloom.iter().map(|(entity, _)| entity)) {
            commands.entity(entity).despawn();
        }
        return;
    };
    // Le voile: sombre et bleuté; un éclair le dissipe et blanchit brièvement la vue.
    let base = GLOOM.iter().find(|(r, _)| *r == region).map_or(0.4, |(_, a)| *a);
    let flash = weather.flash;
    let veil = Color::srgba(0.02 + 0.98 * flash, 0.04 + 0.96 * flash, 0.1 + 0.9 * flash, base * (1.0 - flash) + 0.3 * flash);
    if gloom.is_empty() {
        commands.entity(*camera).with_children(|camera| {
            camera.spawn((Gloom, Sprite::from_color(veil, Vec2::new(2400.0, 1600.0)), Transform::from_xyz(0.0, 0.0, RAIN_Z - 10.0)));
        });
    }
    for (_, mut sprite) in &mut gloom {
        if sprite.color != veil {
            sprite.color = veil;
        }
    }
    if rain.is_empty() {
        let cell = if crate::settings::low_memory() { RAIN_CELL_LITE } else { RAIN_CELL };
        commands.entity(*camera).with_children(|camera| {
            camera.spawn((
                Rain,
                Mesh2d(meshes.add(Rectangle::new(2400.0, 1600.0))),
                MeshMaterial2d(materials.add(RainMaterial { data: Vec4::new(cell, WIND, FALL_SPEED, 1.0) })),
                Transform::from_xyz(0.0, 0.0, RAIN_Z),
            ));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lightning_strike_flashes_twice_then_goes_out() {
        assert_eq!(envelope(0.0), 0.0);
        assert!(envelope(0.02) > 0.95, "premier éclat");
        assert!(envelope(0.12) < 0.1, "accalmie");
        assert!(envelope(0.21) > 0.6, "second éclat");
        assert_eq!(envelope(0.5), 0.0);
    }

    #[test]
    fn only_the_forest_and_the_marsh_get_rain() {
        assert!(is_rainy(Region::Forest) && is_rainy(Region::Marsh));
        assert!(!is_rainy(Region::Approach) && !is_rainy(Region::Crypts));
    }
}
