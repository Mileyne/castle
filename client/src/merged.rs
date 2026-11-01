//! Fusion des primitives d'un modèle de `sprite_forge` en un seul maillage.
//!
//! Un héros se compose de 90 à 155 primitives (sphères, cubes, capsules, cônes). Les dessiner une à une coûte une entité, un
//! `Transform` et un appel de dessin chacune, à chaque image. Ici toutes les pièces d'une pose sont posées dans un même maillage,
//! avec leur couleur dans les sommets: un personnage devient une entité et un appel de dessin.
//! Le maillage d'une pose se construit une fois, puis se garde: `PoseCache`. Un modèle étant en 3D, une seule pose sert les huit
//! directions du regard (on tourne l'entité); seules les actions comptent (repos, marche, coup), à raison d'une image par case de leur
//! boucle. `Baking` les construit toutes à l'avance, par petits morceaux (écran « Veuillez patienter » du jeu et de la visionneuse).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use sprite_forge::{Anim, LivePart, LiveShape, Theme, live_exit, live_landmark, live_parts};

/// Images de la boucle d'une sortie de niveau (torches et bannières qui vacillent).
pub const EXIT_FRAMES: usize = 10;

/// Poids d'un sommet du maillage fusionné, en octets: position et normale (3 flottants chacune) et couleur (4 flottants); les vêtements
/// ont en plus des coordonnées de texture (2 flottants).
pub const VERTEX_BYTES: usize = 12 + 12 + 16;

/// La matière d'une pièce: de quoi dépend son matériau (rugosité, métal, texture). Un modèle est découpé en un maillage par matière.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    /// Tissu et cuir (tabard, robe, cape, pantalon, bottes): mat, avec une texture de tissage.
    Cloth,
    /// Peau, visage, cheveux, barbe: légèrement lisse.
    Skin,
    /// Cotte de mailles, bois, cuir verni: semi-métallique.
    Mail,
    /// Acier, laiton, fer (casque, épaulières, lames): métal poli.
    Metal,
    /// Les pièces qui brillent (orbe, étincelles, braise): elles ne sont pas éclairées.
    Glow,
}

/// Le nombre de matières.
pub const SURFACES: usize = 5;

impl Surface {
    pub const ALL: [Surface; SURFACES] = [Self::Cloth, Self::Skin, Self::Mail, Self::Metal, Self::Glow];

    pub fn index(self) -> usize {
        self as usize
    }

    /// La matière d'une pièce, d'après son éclat, sa couche et son reflet (`shiny` dans `sprite_forge`: 0,8 pour l'acier et le laiton,
    /// environ 0,3 pour les mailles, 0 pour le tissu).
    pub fn of(part: &LivePart) -> Self {
        if part.glow {
            Self::Glow
        } else if part.layer == sprite_forge::LiveLayer::Body {
            Self::Skin
        } else if part.shine >= 0.6 {
            Self::Metal
        } else if part.shine >= 0.2 {
            Self::Mail
        } else {
            Self::Cloth
        }
    }
}

/// Les maillages d'une pose, un par matière (certaines matières peuvent manquer).
pub type PoseMeshes = [Option<Handle<Mesh>>; SURFACES];

/// Le matériau de Bevy d'une matière. `lit`: éclairé par le soleil et la lumière ambiante, avec la rugosité et le métal de la matière;
/// sinon, les couleurs pures. `tint` multiplie la couleur (et la rend translucide si son alpha est inférieur à 1); `cloth` est la
/// texture de tissage des vêtements (`None`: vêtements unis).
pub fn surface_material(surface: Surface, lit: bool, tint: Color, cloth: Option<&Handle<Image>>) -> StandardMaterial {
    let (roughness, metallic, reflectance) = match surface {
        Surface::Cloth => (0.92, 0.0, 0.3),
        Surface::Skin => (0.6, 0.0, 0.4),
        Surface::Mail => (0.45, 0.5, 0.5),
        Surface::Metal => (0.25, 0.9, 0.6),
        Surface::Glow => (0.5, 0.0, 0.5),
    };
    StandardMaterial {
        base_color: tint,
        base_color_texture: if lit && surface == Surface::Cloth { cloth.cloned() } else { None },
        perceptual_roughness: roughness,
        metallic,
        reflectance,
        unlit: !lit || surface == Surface::Glow,
        alpha_mode: if tint.alpha() < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
        ..default()
    }
}

/// La texture de tissage des vêtements: un tissage simple (fils qui passent dessus puis dessous) un peu irrégulier, en tons clairs qui
/// assombrissent légèrement la couleur de la pièce. Elle se répète, avec ses réductions successives (sans elles, le tissage scintillerait
/// de loin).
pub fn weave_image() -> Image {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIZE: usize = 64;
    const MIPS: usize = 7;
    let noise = |x: usize, y: usize| {
        let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
        h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        ((h ^ (h >> 16)) & 255) as f32 / 255.0
    };
    // Le niveau principal: des cases de 8 pixels (4 cm), un fil horizontal ou vertical selon le damier, renflé au milieu et sombre aux
    // bords, avec un peu de grain.
    let mut level: Vec<f32> = (0..SIZE * SIZE)
        .map(|i| {
            let (x, y) = (i % SIZE, i / SIZE);
            let (cx, cy) = ((x % 8) as f32 - 3.5, (y % 8) as f32 - 3.5);
            let horizontal = ((x / 8) + (y / 8)) % 2 == 0;
            let across = if horizontal { cy } else { cx };
            let thread = 1.0 - 0.5 * (across / 3.5).powi(2);
            (0.62 + 0.38 * thread) * (0.92 + 0.08 * noise(x, y))
        })
        .collect();
    let mut data = Vec::new();
    let mut size = SIZE;
    for _ in 0..MIPS {
        data.extend(level.iter().flat_map(|v| {
            // La texture est lue comme du sRGB.
            let byte = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            [byte, byte, byte, 255]
        }));
        // La réduction suivante: la moyenne de quatre pixels.
        let next = (size / 2).max(1);
        level = (0..next * next)
            .map(|i| {
                let (x, y) = ((i % next) * 2, (i / next) * 2);
                let at = |dx: usize, dy: usize| level[((y + dy).min(size - 1)) * size + (x + dx).min(size - 1)];
                (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4.0
            })
            .collect();
        size = next;
    }
    let mut image = Image::new(Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 }, TextureDimension::D2, vec![255; SIZE * SIZE * 4], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.mip_level_count = MIPS as u32;
    image.data = Some(data);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// D'où vient un modèle dans `sprite_forge`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ModelSource {
    /// Un modèle réglable (héros, monstre, objet): sa clé, voir `sprite_forge::models`.
    Key(String),
    /// Un grand motif du décor: son indice dans `LandmarkKind::ALL`.
    Landmark(usize),
    /// Une sortie de niveau: la ligne de sa planche.
    Exit(usize),
}

/// Une pose à construire: le modèle, la famille d'action (0 repos, 1 marche, 2 coup) et l'image dans sa boucle.
pub type PoseKey = (ModelSource, u8, u16);

impl ModelSource {
    fn is_prop(&self) -> bool {
        matches!(self, Self::Key(key) if key.starts_with("prop:"))
    }

    /// Les familles d'action que le modèle a vraiment: un objet, une sortie ou un motif ne font que se tenir là.
    pub fn families(&self) -> &'static [u8] {
        match self {
            Self::Key(_) if !self.is_prop() => &[0, 1, 2],
            _ => &[0],
        }
    }

    /// Le nombre d'images de la boucle d'une famille (celles des planches de sprites du jeu).
    pub fn steps(&self, family: u8) -> usize {
        match (self, family) {
            (Self::Landmark(_), _) => 1,
            (Self::Exit(_), _) => EXIT_FRAMES,
            (_, _) if self.is_prop() => 20,
            (_, 0) => 25,
            (_, 1) => 16,
            _ => 10,
        }
    }

    /// La pose exacte d'une image de la boucle d'une famille.
    pub fn anim(&self, family: u8, index: u16) -> Anim {
        let t = f32::from(index) / self.steps(family) as f32;
        match (self, family) {
            (Self::Key(_), 1) if !self.is_prop() => Anim::Walk(t),
            (Self::Key(_), 2) if !self.is_prop() => Anim::Attack(t),
            _ => Anim::Idle(t),
        }
    }

    /// La pose du modèle la plus proche de `anim` parmi celles qu'il a vraiment: sa famille et son indice dans la boucle.
    pub fn pose(&self, anim: Anim) -> (u8, u16) {
        let (Anim::Idle(t) | Anim::Walk(t) | Anim::Attack(t)) = anim;
        let family = match anim {
            Anim::Idle(_) => 0,
            Anim::Walk(_) => 1,
            Anim::Attack(_) => 2,
        };
        let family = if self.families().contains(&family) { family } else { 0 };
        let steps = self.steps(family);
        (family, ((t.clamp(0.0, 0.9999) * steps as f32) as usize).min(steps - 1) as u16)
    }

    /// Les pièces du modèle dans une pose.
    pub fn parts(&self, anim: Anim) -> Vec<LivePart> {
        match self {
            Self::Key(key) => live_parts(key, Theme::Medieval, anim),
            Self::Landmark(index) => live_landmark(*index),
            Self::Exit(row) => {
                let (Anim::Idle(t) | Anim::Walk(t) | Anim::Attack(t)) = anim;
                live_exit(*row, t, Theme::Medieval)
            }
        }
    }

    /// Toutes les poses du modèle: ce qu'il faut construire pour qu'il ne se fige jamais en jeu.
    pub fn all_poses(&self) -> Vec<PoseKey> {
        self.families()
            .iter()
            .flat_map(|family| (0..self.steps(*family)).map(move |index| (self.clone(), *family, index as u16)))
            .collect()
    }
}

/// Les maillages fusionnés déjà construits, par pose (un par matière).
#[derive(Default)]
pub struct PoseCache {
    poses: HashMap<PoseKey, PoseMeshes>,
    /// Sommets de toutes les poses gardées (pour dire ce que ça pèse).
    pub vertices: usize,
}

impl PoseCache {
    pub fn len(&self) -> usize {
        self.poses.len()
    }

    pub fn contains(&self, key: &PoseKey) -> bool {
        self.poses.contains_key(key)
    }

    /// Mémoire graphique prise par les poses gardées, en octets.
    pub fn bytes(&self) -> usize {
        self.vertices * VERTEX_BYTES
    }

    pub fn clear(&mut self) {
        self.poses.clear();
        self.vertices = 0;
    }

    /// Les maillages d'une pose, construits à la demande si on ne les a pas encore.
    pub fn get_or_build(&mut self, key: &PoseKey, meshes: &mut Assets<Mesh>) -> PoseMeshes {
        if let Some(found) = self.poses.get(key) {
            return found.clone();
        }
        let mut handles: PoseMeshes = Default::default();
        for (slot, mesh) in handles.iter_mut().zip(merge(&key.0.parts(key.0.anim(key.1, key.2)))) {
            if let Some(mesh) = mesh {
                self.vertices += mesh.count_vertices();
                *slot = Some(meshes.add(mesh));
            }
        }
        self.poses.insert(key.clone(), handles.clone());
        handles
    }
}

/// Une construction de poses à l'avance, par morceaux pour pouvoir afficher l'avancement entre deux.
#[derive(Default)]
pub struct Baking {
    jobs: Vec<PoseKey>,
    /// Combien de poses étaient à faire au départ, et combien sont faites.
    total: usize,
    done: usize,
}

impl Baking {
    /// Prépare la construction de toutes les poses de ces modèles, hors celles que le cache a déjà.
    pub fn plan(sources: impl IntoIterator<Item = ModelSource>, cache: &PoseCache) -> Self {
        let mut seen = std::collections::HashSet::new();
        let jobs: Vec<PoseKey> = sources
            .into_iter()
            .filter(|source| seen.insert(source.clone()))
            .flat_map(|source| source.all_poses())
            .filter(|key| !cache.contains(key))
            .collect();
        Self { total: jobs.len(), done: 0, jobs }
    }

    pub fn is_finished(&self) -> bool {
        self.jobs.is_empty()
    }

    /// L'avancement, de 0 à 100 (100 quand il n'y a rien à faire).
    pub fn percent(&self) -> u32 {
        if self.total == 0 { 100 } else { (self.done * 100 / self.total) as u32 }
    }

    /// Construit des poses pendant au plus `budget`.
    pub fn run(&mut self, budget: std::time::Duration, cache: &mut PoseCache, meshes: &mut Assets<Mesh>) {
        let started = std::time::Instant::now();
        while let Some(key) = self.jobs.pop() {
            cache.get_or_build(&key, meshes);
            self.done += 1;
            if started.elapsed() >= budget {
                break;
            }
        }
    }
}

/// Un maillage de base (une forme de rayon 1), déjà réduit à des tableaux.
struct Base {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

/// Un cône de rayons `bottom` et `top` se distingue de l'autre au centième près.
fn base_key(shape: LiveShape) -> (u8, i32, i32) {
    match shape {
        LiveShape::Sphere => (0, 0, 0),
        LiveShape::Cube => (1, 0, 0),
        LiveShape::Capsule(_) => (2, 0, 0),
        LiveShape::Frustum(a, b) => (3, (a * 50.0).round() as i32, (b * 50.0).round() as i32),
    }
}

/// Les formes de base, avec moins de facettes que pour un affichage isolé: une pièce de personnage est petite à l'écran.
fn build_base(shape: LiveShape) -> Base {
    let mesh: Mesh = match shape {
        LiveShape::Sphere => Sphere::new(1.0).mesh().uv(12, 8),
        LiveShape::Cube => Cuboid::new(2.0, 2.0, 2.0).into(),
        LiveShape::Capsule(_) => Capsule3d::new(1.0, 2.0).mesh().rings(2).latitudes(4).longitudes(10).build(),
        LiveShape::Frustum(a, b) => ConicalFrustum { radius_top: b, radius_bottom: a, height: 2.0 }.mesh().resolution(12).build(),
    };
    let float3 = |attribute| match mesh.attribute(attribute) {
        Some(bevy::mesh::VertexAttributeValues::Float32x3(values)) => values.clone(),
        _ => Vec::new(),
    };
    let indices = match mesh.indices() {
        Some(Indices::U16(indices)) => indices.iter().map(|i| u32::from(*i)).collect(),
        Some(Indices::U32(indices)) => indices.clone(),
        None => Vec::new(),
    };
    Base { positions: float3(Mesh::ATTRIBUTE_POSITION), normals: float3(Mesh::ATTRIBUTE_NORMAL), indices }
}

fn base(shape: LiveShape) -> Arc<Base> {
    static BASES: OnceLock<Mutex<HashMap<(u8, i32, i32), Arc<Base>>>> = OnceLock::new();
    let mut bases = BASES.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    bases.entry(base_key(shape)).or_insert_with(|| Arc::new(build_base(shape))).clone()
}

/// La place d'une pièce dans le modèle.
pub fn part_transform(part: &LivePart) -> Transform {
    let mut scale = Vec3::from(part.scale);
    if let LiveShape::Capsule(h) = part.shape {
        // La capsule de référence a une demi-longueur de 2 pour un rayon de 1: on l'étire pour que le total fasse `rayon * (h + 1)`.
        scale.y = part.scale[0] * (h + 1.0) / 2.0;
    }
    Transform { translation: Vec3::from(part.center), rotation: Quat::from_mat3(&Mat3::from_cols_array_2d(&part.rot)), scale }
}

/// Le tampon de sommets d'un maillage fusionné: position, normale et couleur (linéaire et opaque, comme les matériaux de Bevy la lisent).
#[derive(Default)]
pub struct Buffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Coordonnées de texture (une projection plane selon l'axe dominant de la normale), gardées seulement pour les vêtements.
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Côté, en mètres, d'une répétition de la texture de tissage.
const WEAVE_TILE: f32 = 0.32;

/// Les coordonnées de texture d'un point: la face du cube qui regarde le plus la normale, projetée à plat.
fn box_uv(position: Vec3, normal: Vec3) -> [f32; 2] {
    let n = normal.abs();
    let (u, v) = if n.x >= n.y && n.x >= n.z {
        (position.z, position.y)
    } else if n.y >= n.z {
        (position.x, position.z)
    } else {
        (position.x, position.y)
    };
    [u / WEAVE_TILE, v / WEAVE_TILE]
}

/// La couleur d'un sommet pour les matériaux de Bevy: la couleur sRGB de `sprite_forge` passée en linéaire. Une pièce qui brille (orbe,
/// étincelles) reçoit une couleur plus vive, qui la fait ressortir.
pub fn vertex_color(color: [f32; 3], glow: bool) -> [f32; 4] {
    let linear = Color::srgb(color[0], color[1], color[2]).to_linear();
    let boost = if glow { 3.0 } else { 1.0 };
    [linear.red * boost, linear.green * boost, linear.blue * boost, 1.0]
}

impl Buffers {
    /// Ajoute une primitive entière, à sa place.
    pub fn push_part(&mut self, part: &LivePart) {
        let shape = base(part.shape);
        let place = part_transform(part);
        let first = self.positions.len() as u32;
        // Une normale suit l'inverse de l'échelle (puis la rotation): une sphère aplatie garde des normales justes.
        let inverse_scale = Vec3::new(1.0 / place.scale.x.max(1e-4), 1.0 / place.scale.y.max(1e-4), 1.0 / place.scale.z.max(1e-4));
        for (position, normal) in shape.positions.iter().zip(&shape.normals) {
            let at = place.rotation * (Vec3::from(*position) * place.scale) + place.translation;
            let facing = (place.rotation * (Vec3::from(*normal) * inverse_scale)).normalize_or_zero();
            self.positions.push(at.to_array());
            self.normals.push(facing.to_array());
            self.uvs.push(box_uv(at, facing));
            self.colors.push(vertex_color(part.color, part.glow));
        }
        self.indices.extend(shape.indices.iter().map(|index| first + index));
    }

    pub fn into_mesh(self) -> Mesh {
        self.into_mesh_with(false)
    }

    /// Le maillage, avec ses coordonnées de texture si `with_uv` (les vêtements).
    pub fn into_mesh_with(self, with_uv: bool) -> Mesh {
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices));
        if with_uv { mesh.with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs) } else { mesh }
    }
}

/// Toutes les pièces d'une pose en un maillage par matière (`None` pour une matière absente). Chaque sommet porte la couleur de sa pièce.
pub fn merge(parts: &[LivePart]) -> [Option<Mesh>; SURFACES] {
    let mut buffers: [Buffers; SURFACES] = Default::default();
    for part in parts {
        buffers[Surface::of(part).index()].push_part(part);
    }
    let mut meshes: [Option<Mesh>; SURFACES] = Default::default();
    for (surface, (slot, buffer)) in Surface::ALL.into_iter().zip(meshes.iter_mut().zip(buffers)) {
        if !buffer.positions.is_empty() {
            *slot = Some(buffer.into_mesh_with(surface == Surface::Cloth));
        }
    }
    meshes
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn a_characters_poses_follow_the_frames_of_its_sprite_loops() {
        let hero = ModelSource::Key("hero:Knight".into());
        assert_eq!(hero.families(), &[0, 1, 2]);
        assert_eq!((hero.steps(0), hero.steps(1), hero.steps(2)), (25, 16, 10));
        assert_eq!(hero.all_poses().len(), 25 + 16 + 10);
        // Un objet, un motif et une sortie ne font que se tenir là.
        let prop = ModelSource::Key("prop:Coin".into());
        assert_eq!((prop.families(), prop.all_poses().len()), (&[0u8][..], 20));
        assert_eq!(ModelSource::Landmark(1).all_poses().len(), 1);
        assert_eq!(ModelSource::Exit(2).all_poses().len(), EXIT_FRAMES);
    }

    #[test]
    fn the_nearest_pose_is_always_one_that_gets_baked() {
        let hero = ModelSource::Key("hero:Ranger".into());
        let baked: std::collections::HashSet<PoseKey> = hero.all_poses().into_iter().collect();
        for step in 0..=100 {
            let t = step as f32 / 100.0;
            for anim in [Anim::Idle(t), Anim::Walk(t), Anim::Attack(t)] {
                let (family, index) = hero.pose(anim);
                assert!(baked.contains(&(hero.clone(), family, index)), "{anim:?} -> {family}/{index}");
            }
        }
        // Un objet ramène toute demande de marche ou de coup à son repos.
        let coin = ModelSource::Key("prop:Coin".into());
        assert_eq!(coin.pose(Anim::Walk(0.5)).0, 0);
    }

    #[test]
    fn baking_reports_its_progress_and_skips_what_is_already_built() {
        let mut meshes = Assets::<Mesh>::default();
        let mut cache = PoseCache::default();
        let sources = [ModelSource::Landmark(0), ModelSource::Exit(0), ModelSource::Landmark(0)];
        let mut baking = Baking::plan(sources.clone(), &cache);
        assert_eq!((baking.percent(), baking.is_finished()), (0, false));
        while !baking.is_finished() {
            baking.run(std::time::Duration::ZERO, &mut cache, &mut meshes);
        }
        // Le doublon n'est construit qu'une fois: un motif (1 pose) plus une sortie (dix).
        assert_eq!((baking.percent(), cache.len()), (100, 1 + EXIT_FRAMES));
        assert!(cache.bytes() > 0);
        assert!(Baking::plan(sources, &cache).is_finished(), "tout est déjà construit");
    }

    fn part(shape: LiveShape, center: [f32; 3], glow: bool) -> LivePart {
        LivePart { shape, center, rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], scale: [1.0; 3], color: [1.0, 0.5, 0.25], glow, shine: 0.3, layer: sprite_forge::LiveLayer::Outfit }
    }

    fn all_vertices(meshes: &[Option<Mesh>; SURFACES]) -> usize {
        meshes.iter().flatten().map(Mesh::count_vertices).sum()
    }

    #[test]
    fn merging_keeps_every_vertex_and_every_triangle() {
        let parts = [part(LiveShape::Sphere, [0.0; 3], false), part(LiveShape::Cube, [2.0, 0.0, 0.0], true)];
        let (sphere, cube) = (base(LiveShape::Sphere), base(LiveShape::Cube));
        let meshes = merge(&parts);
        assert_eq!(all_vertices(&meshes), sphere.positions.len() + cube.positions.len());
        let triangles: usize = meshes.iter().flatten().filter_map(|mesh| mesh.indices().map(|i| i.len())).sum();
        assert_eq!(triangles, sphere.indices.len() + cube.indices.len());
    }

    #[test]
    fn parts_are_split_by_material_and_carry_their_own_colour() {
        // Un cube qui brille: la matière « lumière »; sa couleur est linéaire, trois fois plus vive.
        let meshes = merge(&[part(LiveShape::Cube, [10.0, 0.0, 0.0], true)]);
        assert!(meshes.iter().enumerate().all(|(i, mesh)| mesh.is_some() == (i == Surface::Glow.index())));
        let mesh = meshes[Surface::Glow.index()].as_ref().expect("la matière lumière");
        let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("positions manquantes");
        };
        assert!(positions.iter().all(|p| p[0] >= 9.0 && p[0] <= 11.0), "le cube est centré en x = 10");
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) else {
            panic!("couleurs manquantes");
        };
        let linear = Color::srgb(1.0, 0.5, 0.25).to_linear();
        assert!(colors.iter().all(|c| (c[0] - linear.red * 3.0).abs() < 1e-4 && (c[1] - linear.green * 3.0).abs() < 1e-4 && c[3] == 1.0));
    }

    #[test]
    fn a_pieces_material_follows_its_shine_and_layer() {
        let mut piece = part(LiveShape::Sphere, [0.0; 3], false);
        piece.shine = 0.0;
        assert_eq!(Surface::of(&piece), Surface::Cloth);
        piece.shine = 0.3;
        assert_eq!(Surface::of(&piece), Surface::Mail);
        piece.shine = 0.8;
        assert_eq!(Surface::of(&piece), Surface::Metal);
        piece.layer = sprite_forge::LiveLayer::Body;
        assert_eq!(Surface::of(&piece), Surface::Skin, "la peau reste de la peau, même brillante");
        piece.glow = true;
        assert_eq!(Surface::of(&piece), Surface::Glow);
    }

    #[test]
    fn only_clothes_carry_texture_coordinates() {
        let mut cloth = part(LiveShape::Sphere, [0.0; 3], false);
        cloth.shine = 0.0;
        let mut metal = part(LiveShape::Sphere, [1.0; 3], false);
        metal.shine = 0.9;
        let meshes = merge(&[cloth, metal]);
        assert!(meshes[Surface::Cloth.index()].as_ref().is_some_and(|mesh| mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some()));
        assert!(meshes[Surface::Metal.index()].as_ref().is_some_and(|mesh| mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_none()));
    }

    #[test]
    fn the_weave_texture_has_all_its_reductions() {
        let image = weave_image();
        assert_eq!(image.texture_descriptor.mip_level_count, 7);
        // 64x64 + 32x32 + ... + 1x1, quatre octets par pixel.
        assert_eq!(image.data.as_ref().map(Vec::len), Some((64 * 64 + 32 * 32 + 16 * 16 + 8 * 8 + 4 * 4 + 2 * 2 + 1) * 4));
    }

    #[test]
    fn indices_never_point_outside_their_part() {
        let parts = [part(LiveShape::Capsule(1.5), [0.0; 3], false), part(LiveShape::Frustum(1.0, 0.5), [0.0, 3.0, 0.0], false)];
        for mesh in merge(&parts).iter().flatten() {
            let count = mesh.count_vertices() as u32;
            assert!(mesh.indices().is_some_and(|indices| indices.iter().all(|i| (i as u32) < count)));
        }
    }
}
