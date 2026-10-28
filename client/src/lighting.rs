//! Lumière du château: les zones loin des héros et des sources lumineuses s'assombrissent.
//!
//! Une couche sombre, attachée à la caméra, est dessinée par un petit shader (`assets/shaders/lighting.wgsl`)
//! qui l'éclaircit autour des lumières: les héros, puis les générateurs, le portail et les tourelles.

use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d},
};
use castle_shared::*;

use crate::{LocalParty, LocalPlayer};
use crate::art::ArtTheme;

const MAX_LIGHTS: usize = 16;
/// Part d'obscurité loin de toute lumière (0 = pas d'effet, 1 = noir complet).
const AMBIENT_DARKNESS: f32 = 0.78;
/// Mode sanglant: la pénombre est plus épaisse, et teintée de rouge.
const GORE_AMBIENT_DARKNESS: f32 = 0.89;
/// Rayon du halo du héros dirigé, des compagnons, et des sources lumineuses du décor.
const HERO_RADIUS: f32 = 135.0;
const COMPANION_RADIUS: f32 = 95.0;
/// Vitesse à laquelle la colorimétrie glisse vers celle d'un nouveau héros.
const GRADE_SPEED: f32 = 4.0;
/// Pourtour de la vue: il s'assombrit à partir de cette distance du centre, avec cette force.
const VIGNETTE_START: f32 = 120.0;
const VIGNETTE_STRENGTH: f32 = 0.55;

/// Colorimétrie d'un héros: couleur du voile posé sur toute l'image, sa force, et le multiplicateur de couleur
/// appliqué au décor (les personnages gardent leurs couleurs). Gris pour le Chevalier, violet pour l'Ensorceleur,
/// vert pour le Rôdeur, orange pour le Forgebraise.
fn grade_of(class: HeroClass) -> (Vec3, f32, Vec3) {
    match class {
        HeroClass::Knight => (Vec3::new(0.58, 0.6, 0.64), 0.2, Vec3::new(0.8, 0.82, 0.86)),
        HeroClass::Sorcerer => (Vec3::new(0.75, 0.4, 1.0), 0.13, Vec3::new(0.84, 0.55, 1.0)),
        HeroClass::Ranger => (Vec3::new(0.35, 0.95, 0.45), 0.12, Vec3::new(0.55, 1.0, 0.6)),
        HeroClass::Forgebraise => (Vec3::new(1.0, 0.55, 0.2), 0.13, Vec3::new(1.0, 0.64, 0.34)),
    }
}

/// Multiplicateur de couleur du décor selon le héros dirigé, lissé; les tuiles le lisent pour se teinter.
#[derive(Resource)]
pub struct Grade(pub Vec3);

impl Default for Grade {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

#[derive(ShaderType, Clone, Debug)]
pub struct LightData {
    pub lights: [Vec4; MAX_LIGHTS],
    pub params: Vec4,
    /// Teinte du héros dirigé (RVB) et force de la colorimétrie.
    pub tint: Vec4,
    /// Centre de la vue (xy), rayon où commence l'assombrissement du pourtour, et sa force.
    pub view: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct LightingMaterial {
    #[uniform(0)]
    pub data: LightData,
}

impl Material2d for LightingMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lighting.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Grade>()
            .add_plugins(Material2dPlugin::<LightingMaterial>::default())
            .add_systems(Update, update_lights);
    }
}

/// Couche sombre qui recouvre toute la vue, à attacher à la caméra.
pub fn overlay(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<LightingMaterial>,
) -> impl Bundle {
    let material = materials.add(LightingMaterial {
        data: LightData { lights: [Vec4::ZERO; MAX_LIGHTS], params: Vec4::new(0.0, AMBIENT_DARKNESS, 0.0, 0.0), tint: Vec4::ZERO, view: Vec4::ZERO },
    });
    (
        Mesh2d(meshes.add(Rectangle::new(2400.0, 1600.0))),
        MeshMaterial2d(material),
        // Devant tout le décor, derrière l'interface.
        Transform::from_xyz(0.0, 0.0, -900.0),
    )
}

fn update_lights(
    time: Res<Time>,
    party: Option<Res<LocalParty>>,
    theme: Res<ArtTheme>,
    active: Option<Single<&HeroClass, With<LocalPlayer>>>,
    mut smooth: Local<Option<(Vec3, f32, Vec3)>>,
    mut environment: ResMut<Grade>,
    heroes: Query<(&Transform, &PartyId, &HeroClass), With<Player>>,
    sources: Query<(&Transform, Option<&Generator>, Option<&Portal>), Or<(With<Generator>, With<Portal>, With<Turret>)>>,
    camera: Single<&Transform, With<Camera2d>>,
    overlay: Single<&MeshMaterial2d<LightingMaterial>>,
    mut materials: ResMut<Assets<LightingMaterial>>,
) {
    let Some(mut material) = materials.get_mut(&overlay.0) else {
        return;
    };
    let mine = party.map(|p| p.0);
    let mut lights: Vec<Vec4> = Vec::new();
    // Les héros de l'équipe du joueur d'abord (plus lumineux), puis ceux des autres joueurs.
    let mut hero_lights: Vec<(bool, Vec4)> = heroes
        .iter()
        .map(|(t, id, _)| {
            let own = Some(id.0) == mine;
            let radius = if own { HERO_RADIUS } else { COMPANION_RADIUS };
            (own, Vec4::new(t.translation.x, t.translation.y + 6.0, radius, if own { 1.0 } else { 0.85 }))
        })
        .collect();
    hero_lights.sort_by_key(|(own, _)| !own);
    lights.extend(hero_lights.into_iter().take(10).map(|(_, l)| l));

    // Puis les sources du décor les plus proches de la caméra.
    let view = camera.translation.truncate();
    let mut near: Vec<(f32, Vec4)> = sources
        .iter()
        .map(|(t, generator, portal)| {
            let (radius, intensity) = if portal.is_some() {
                (95.0, 0.95)
            } else if generator.is_some() {
                (70.0, 0.8)
            } else {
                (55.0, 0.7)
            };
            let pos = t.translation.truncate();
            (pos.distance(view), Vec4::new(pos.x, pos.y + 8.0, radius, intensity))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    lights.extend(near.into_iter().map(|(_, l)| l).take(MAX_LIGHTS - lights.len().min(MAX_LIGHTS)));

    let count = lights.len().min(MAX_LIGHTS);
    material.data.lights = [Vec4::ZERO; MAX_LIGHTS];
    for (slot, light) in material.data.lights.iter_mut().zip(lights.iter().take(count)) {
        *slot = *light;
    }
    let gore = *theme == ArtTheme::Gore;
    let ambient = if gore { GORE_AMBIENT_DARKNESS } else { AMBIENT_DARKNESS };
    material.data.params = Vec4::new(count as f32, ambient, if gore { 1.0 } else { 0.0 }, 0.0);

    // La colorimétrie glisse doucement vers celle de la tenue du héros dirigé.
    let target = active.map_or((Vec3::splat(0.8), 0.0, Vec3::ONE), |class| grade_of(**class));
    let k = 1.0 - (-GRADE_SPEED * time.delta_secs()).exp();
    let (wash, strength, tiles) = smooth.get_or_insert(target);
    *wash = wash.lerp(target.0, k);
    *strength += (target.1 - *strength) * k;
    *tiles = tiles.lerp(target.2, k);
    // Le mode sanglant garde une colorimétrie un peu plus discrète: le rouge domine déjà.
    let (wash_strength, environment_mix) = if gore { (*strength * 0.8, 0.5) } else { (*strength, 0.6) };
    material.data.tint = wash.extend(wash_strength);
    material.data.view = Vec4::new(camera.translation.x, camera.translation.y, VIGNETTE_START, VIGNETTE_STRENGTH);
    environment.0 = Vec3::ONE.lerp(*tiles, environment_mix);
}
