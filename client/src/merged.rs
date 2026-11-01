//! Fusion des primitives d'un modèle de `sprite_forge` en un seul maillage.
//!
//! Un héros se compose de 90 à 155 primitives (sphères, cubes, capsules, cônes). Les dessiner une à une coûte une entité, un
//! `Transform` et un appel de dessin chacune, à chaque image. Ici toutes les pièces d'une pose sont posées dans un même maillage,
//! avec leur couleur, leur éclat et leur reflet dans la couleur des sommets: un personnage devient une entité et un appel de dessin.
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

/// Poids d'un sommet du maillage fusionné, en octets: position et normale (3 flottants chacune) et couleur (4 flottants).
pub const VERTEX_BYTES: usize = 12 + 12 + 16;

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

/// Les maillages fusionnés déjà construits, par pose.
#[derive(Default)]
pub struct PoseCache {
    poses: HashMap<PoseKey, Handle<Mesh>>,
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

    /// Le maillage d'une pose, construit à la demande si on ne l'a pas encore.
    pub fn get_or_build(&mut self, key: &PoseKey, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        if let Some(found) = self.poses.get(key) {
            return found.clone();
        }
        let mesh = merge(&key.0.parts(key.0.anim(key.1, key.2)));
        self.vertices += mesh.count_vertices();
        let handle = meshes.add(mesh);
        self.poses.insert(key.clone(), handle.clone());
        handle
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

/// Le tampon de sommets d'un maillage fusionné: position, normale et couleur (avec l'éclat et le reflet dans l'alpha).
#[derive(Default)]
pub struct Buffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

/// L'alpha d'une couleur de sommet: l'éclat et le reflet partagent le canal alpha (2 + reflet pour une pièce qui brille, reflet sinon;
/// aucune surface n'est transparente).
pub fn glow_shine(glow: bool, shine: f32) -> f32 {
    (if glow { 2.0 } else { 0.0 }) + shine.clamp(0.0, 1.0)
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
            self.positions.push((place.rotation * (Vec3::from(*position) * place.scale) + place.translation).to_array());
            self.normals.push((place.rotation * (Vec3::from(*normal) * inverse_scale)).normalize_or_zero().to_array());
            self.colors.push([part.color[0], part.color[1], part.color[2], glow_shine(part.glow, part.shine)]);
        }
        self.indices.extend(shape.indices.iter().map(|index| first + index));
    }

    pub fn into_mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// Toutes les pièces dans un seul maillage. Chaque sommet porte la couleur de sa pièce (sRGB, comme dans `sprite_forge`) et, dans son
/// canal alpha, son éclat (la pièce brille, sans ombre) et son reflet.
pub fn merge(parts: &[LivePart]) -> Mesh {
    let mut buffers = Buffers::default();
    for part in parts {
        buffers.push_part(part);
    }
    buffers.into_mesh()
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
        LivePart { shape, center, rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], scale: [1.0; 3], color: [1.0, 0.5, 0.25], glow, shine: 0.3 }
    }

    #[test]
    fn merging_keeps_every_vertex_and_every_triangle() {
        let parts = [part(LiveShape::Sphere, [0.0; 3], false), part(LiveShape::Cube, [2.0, 0.0, 0.0], true)];
        let (sphere, cube) = (base(LiveShape::Sphere), base(LiveShape::Cube));
        let mesh = merge(&parts);
        assert_eq!(mesh.count_vertices(), sphere.positions.len() + cube.positions.len());
        assert_eq!(mesh.indices().map(|i| i.len()), Some(sphere.indices.len() + cube.indices.len()));
    }

    #[test]
    fn parts_are_placed_and_carry_their_own_colour_and_glow() {
        let mesh = merge(&[part(LiveShape::Cube, [10.0, 0.0, 0.0], true)]);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("positions manquantes");
        };
        assert!(positions.iter().all(|p| p[0] >= 9.0 && p[0] <= 11.0), "le cube est centré en x = 10");
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) else {
            panic!("couleurs manquantes");
        };
        // La pièce brille (2) et son reflet vaut 0,3.
        assert!(colors.iter().all(|c| c[..3] == [1.0, 0.5, 0.25] && (c[3] - 2.3).abs() < 1e-5));
    }

    #[test]
    fn indices_never_point_outside_their_part() {
        let parts = [part(LiveShape::Capsule(1.5), [0.0; 3], false), part(LiveShape::Frustum(1.0, 0.5), [0.0, 3.0, 0.0], false)];
        let mesh = merge(&parts);
        let count = mesh.count_vertices() as u32;
        assert!(mesh.indices().is_some_and(|indices| indices.iter().all(|i| (i as u32) < count)));
    }
}
