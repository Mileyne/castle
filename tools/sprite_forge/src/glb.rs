//! Export des modèles 3D vers glTF (`.glb`), le format que Blender, Godot, Unity et la plupart des outils 3D lisent.
//!
//! Chaque pièce du modèle (sphère, cube, capsule, cône tronqué) devient un objet nommé (`012_Metal_cone`), avec son matériau (couleur,
//! rugosité, métal, émission). Les poses de repos, de marche et de coup deviennent trois animations (`Idle`, `Walk`, `Attack`), qui
//! déplacent ces objets image par image. Il n'y a ni squelette ni peau: c'est le modèle tel que le jeu le fabrique, un point de départ
//! que l'on reprend dans Blender (voir `docs/blender.md`).
//!
//! Repère: le mètre, y vers le haut, le personnage regarde vers +z et a les pieds en y = 0, comme glTF le demande.

use std::collections::HashMap;

use glam::{Mat3, Quat, Vec3};
use serde_json::{Value, json};

use crate::editor_api::{Anim, FaceMap, LiveLayer, LivePart, LiveShape, face_map, live_parts, models};
use crate::shapes::Theme;

/// Les animations d'un personnage: nom, nombre d'images, cadence (images par seconde, celle des planches du jeu), boucle ou non.
const CLIPS: [(&str, usize); 3] = [("Idle", 25), ("Walk", 16), ("Attack", 10)];
const FPS: f32 = 25.0;

/// La matière d'une pièce, d'après son éclat, sa couche et son reflet (la même règle que `client/src/merged.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Surface {
    Cloth,
    Skin,
    Mail,
    Metal,
    Glow,
    Face,
}

impl Surface {
    pub fn of(part: &LivePart) -> Self {
        if part.glow {
            Self::Glow
        } else if part.layer == LiveLayer::Face {
            Self::Face
        } else if part.layer == LiveLayer::Body {
            Self::Skin
        } else if part.shine >= 0.6 {
            Self::Metal
        } else if part.shine >= 0.2 {
            Self::Mail
        } else {
            Self::Cloth
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Cloth => "Cloth",
            Self::Skin => "Skin",
            Self::Mail => "Mail",
            Self::Metal => "Metal",
            Self::Glow => "Glow",
            Self::Face => "Face",
        }
    }

    /// Rugosité, métal (les valeurs du jeu).
    fn pbr(self) -> (f32, f32) {
        match self {
            Self::Cloth => (0.92, 0.0),
            Self::Skin | Self::Face => (0.6, 0.0),
            Self::Mail => (0.45, 0.5),
            Self::Metal => (0.25, 0.9),
            Self::Glow => (0.5, 0.0),
        }
    }
}

fn shape_name(shape: LiveShape) -> &'static str {
    match shape {
        LiveShape::Sphere => "ball",
        LiveShape::Cube => "block",
        LiveShape::Capsule(_) => "limb",
        LiveShape::Frustum(..) => "cone",
    }
}

/// Le dernier octet: sRGB vers linéaire.
fn linear(value: f32) -> f32 {
    if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
}

/// La place d'une pièce: translation, rotation, échelle (la capsule est étirée pour que sa longueur totale soit `rayon x (h + 1)`).
fn transform(part: &LivePart) -> (Vec3, Quat, Vec3) {
    let mut scale = Vec3::from(part.scale);
    if let LiveShape::Capsule(h) = part.shape {
        scale.y = part.scale[0] * (h + 1.0) / 2.0;
    }
    let rotation = Quat::from_mat3(&Mat3::from_cols_array_2d(&part.rot)).normalize();
    (Vec3::from(part.center), rotation, scale)
}

// ----------------------------------------------------------------------------------------------------------------------------------
// Géométrie de base (rayon 1)

#[derive(Default)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl Geometry {
    /// Un quadrilatère (quatre sommets dans l'ordre trigonométrique vus de l'extérieur).
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3) {
        let first = self.positions.len() as u32;
        for (corner, uv) in corners.iter().zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]) {
            self.positions.push(corner.to_array());
            self.normals.push(normal.to_array());
            self.uvs.push(uv);
        }
        self.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    /// Une grille de sommets (`rows` lignes de `columns` + 1 sommets, le dernier répétant le premier) reliée en triangles. `from_top`: les
    /// lignes vont de haut en bas (sphère, capsule), sinon de bas en haut (flanc d'un cône); le sens des triangles en dépend, pour que
    /// l'avant de chacun regarde vers l'extérieur.
    fn grid(&mut self, rows: usize, columns: usize, first: u32, from_top: bool) {
        for r in 0..rows {
            for c in 0..columns {
                let a = first + (r * (columns + 1) + c) as u32;
                let b = a + (columns + 1) as u32;
                if from_top {
                    self.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
                } else {
                    self.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
                }
            }
        }
    }
}

const SLICES: usize = 24;
const STACKS: usize = 16;

/// Une sphère de rayon 1; avec `face`, ses coordonnées de texture sont la projection de face de la photo du héros (pour le crâne), sinon
/// une projection simple de face, de -1 à 1 sur toute la largeur.
fn sphere(frontal: bool, face: Option<FaceMap>) -> Geometry {
    let mut g = Geometry::default();
    for i in 0..=STACKS {
        let theta = std::f32::consts::PI * i as f32 / STACKS as f32;
        for j in 0..=SLICES {
            let phi = std::f32::consts::TAU * j as f32 / SLICES as f32;
            // y vers le haut; phi = 0 regarde vers +z (l'avant du personnage).
            let p = Vec3::new(theta.sin() * phi.sin(), theta.cos(), theta.sin() * phi.cos());
            g.positions.push(p.to_array());
            g.normals.push(p.to_array());
            g.uvs.push(if frontal {
                face.map_or([0.5 + p.x / 2.0, 0.5 - p.y / 2.0], |face| face.uv(p.x, p.y))
            } else {
                [j as f32 / SLICES as f32, i as f32 / STACKS as f32]
            });
        }
    }
    g.grid(STACKS, SLICES, 0, true);
    g
}

/// Un cube de demi-côté 1.
fn cube() -> Geometry {
    let mut g = Geometry::default();
    let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    g.quad([c(-1.0, -1.0, 1.0), c(1.0, -1.0, 1.0), c(1.0, 1.0, 1.0), c(-1.0, 1.0, 1.0)], Vec3::Z);
    g.quad([c(1.0, -1.0, -1.0), c(-1.0, -1.0, -1.0), c(-1.0, 1.0, -1.0), c(1.0, 1.0, -1.0)], Vec3::NEG_Z);
    g.quad([c(1.0, -1.0, 1.0), c(1.0, -1.0, -1.0), c(1.0, 1.0, -1.0), c(1.0, 1.0, 1.0)], Vec3::X);
    g.quad([c(-1.0, -1.0, -1.0), c(-1.0, -1.0, 1.0), c(-1.0, 1.0, 1.0), c(-1.0, 1.0, -1.0)], Vec3::NEG_X);
    g.quad([c(-1.0, 1.0, 1.0), c(1.0, 1.0, 1.0), c(1.0, 1.0, -1.0), c(-1.0, 1.0, -1.0)], Vec3::Y);
    g.quad([c(-1.0, -1.0, -1.0), c(1.0, -1.0, -1.0), c(1.0, -1.0, 1.0), c(-1.0, -1.0, 1.0)], Vec3::NEG_Y);
    g
}

/// Une capsule de rayon 1 le long de y: un cylindre de y = -1 à 1, fermé par deux demi-sphères (de y = -2 à 2 en tout).
fn capsule() -> Geometry {
    let mut g = Geometry::default();
    let half = STACKS / 2;
    // De haut en bas: la demi-sphère du haut, puis celle du bas (la jonction cylindrique est le segment entre elles).
    for i in 0..=STACKS {
        let theta = std::f32::consts::PI * i as f32 / STACKS as f32;
        let offset = if i <= half { 1.0 } else { -1.0 };
        for j in 0..=SLICES {
            let phi = std::f32::consts::TAU * j as f32 / SLICES as f32;
            let p = Vec3::new(theta.sin() * phi.sin(), theta.cos(), theta.sin() * phi.cos());
            g.positions.push([p.x, p.y + offset, p.z]);
            g.normals.push(p.to_array());
            g.uvs.push([j as f32 / SLICES as f32, i as f32 / STACKS as f32]);
        }
    }
    g.grid(STACKS, SLICES, 0, true);
    g
}

/// Un cône tronqué le long de y, de y = -1 (rayon `bottom`) à y = 1 (rayon `top`), fermé par deux disques.
fn frustum(bottom: f32, top: f32) -> Geometry {
    let mut g = Geometry::default();
    // Le flanc: la normale penche selon la différence des rayons (une pente de `bottom - top` sur une hauteur de 2).
    for (row, (y, radius)) in [(-1.0f32, bottom), (1.0, top)].into_iter().enumerate() {
        for j in 0..=SLICES {
            let phi = std::f32::consts::TAU * j as f32 / SLICES as f32;
            let n = Vec3::new(phi.sin() * 2.0, bottom - top, phi.cos() * 2.0).normalize();
            g.positions.push([radius * phi.sin(), y, radius * phi.cos()]);
            g.normals.push(n.to_array());
            g.uvs.push([j as f32 / SLICES as f32, row as f32]);
        }
    }
    g.grid(1, SLICES, 0, false);
    // Les disques.
    for (y, radius, normal) in [(-1.0f32, bottom, Vec3::NEG_Y), (1.0, top, Vec3::Y)] {
        let center = g.positions.len() as u32;
        g.positions.push([0.0, y, 0.0]);
        g.normals.push(normal.to_array());
        g.uvs.push([0.5, 0.5]);
        for j in 0..=SLICES {
            let phi = std::f32::consts::TAU * j as f32 / SLICES as f32;
            g.positions.push([radius * phi.sin(), y, radius * phi.cos()]);
            g.normals.push(normal.to_array());
            g.uvs.push([0.5 + phi.sin() / 2.0, 0.5 + phi.cos() / 2.0]);
        }
        for j in 0..SLICES as u32 {
            let (a, b) = (center + 1 + j, center + 2 + j);
            // Vue de l'extérieur, dans le sens trigonométrique (le disque du haut regarde vers +y, celui du bas vers -y).
            if normal.y > 0.0 {
                g.indices.extend([center, a, b]);
            } else {
                g.indices.extend([center, b, a]);
            }
        }
    }
    g
}

// ----------------------------------------------------------------------------------------------------------------------------------
// Écriture du fichier

/// Le tampon binaire du fichier et ses descriptions (vues et accesseurs).
#[derive(Default)]
struct Binary {
    data: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Binary {
    /// Ajoute un tableau de flottants (`width` par élément) et rend l'indice de son accesseur.
    fn floats(&mut self, values: &[f32], width: usize, kind: &str, bounds: bool) -> usize {
        let offset = self.data.len();
        self.data.extend(values.iter().flat_map(|v| v.to_le_bytes()));
        self.pad();
        self.views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": values.len() * 4 }));
        let mut accessor = json!({ "bufferView": self.views.len() - 1, "componentType": 5126, "count": values.len() / width, "type": kind });
        if bounds && !values.is_empty() {
            let mut min = vec![f32::MAX; width];
            let mut max = vec![f32::MIN; width];
            for chunk in values.chunks(width) {
                for (k, v) in chunk.iter().enumerate() {
                    min[k] = min[k].min(*v);
                    max[k] = max[k].max(*v);
                }
            }
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn indices(&mut self, values: &[u32]) -> usize {
        let offset = self.data.len();
        self.data.extend(values.iter().flat_map(|v| v.to_le_bytes()));
        self.pad();
        self.views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": values.len() * 4, "target": 34963 }));
        self.accessors.push(json!({ "bufferView": self.views.len() - 1, "componentType": 5125, "count": values.len(), "type": "SCALAR" }));
        self.accessors.len() - 1
    }

    /// Des octets quelconques (une image) dans une vue; rend l'indice de la vue.
    fn bytes(&mut self, bytes: &[u8]) -> usize {
        let offset = self.data.len();
        self.data.extend_from_slice(bytes);
        self.pad();
        self.views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": bytes.len() }));
        self.views.len() - 1
    }

    fn pad(&mut self) {
        while self.data.len() % 4 != 0 {
            self.data.push(0);
        }
    }
}

/// Les accesseurs d'une géométrie écrite dans le tampon: position, normale, coordonnées de texture, indices.
fn add_geometry(binary: &mut Binary, geometry: &Geometry) -> [usize; 4] {
    let flat = |v: &[[f32; 3]]| v.iter().flatten().copied().collect::<Vec<f32>>();
    let position = binary.floats(&flat(&geometry.positions), 3, "VEC3", true);
    let normal = binary.floats(&flat(&geometry.normals), 3, "VEC3", false);
    let uv = binary.floats(&geometry.uvs.iter().flatten().copied().collect::<Vec<f32>>(), 2, "VEC2", false);
    let indices = binary.indices(&geometry.indices);
    [position, normal, uv, indices]
}

/// Le modèle `key` (voir `models()`) en glTF binaire. `face_png`: la peau tirée de la photo du héros, plaquée sur son crâne.
/// `None` si la clé est inconnue.
pub fn export_glb(key: &str, face_png: Option<&[u8]>) -> Option<Vec<u8>> {
    let theme = Theme::Medieval;
    let rest = live_parts(key, theme, Anim::Idle(0.0));
    if rest.is_empty() {
        return None;
    }
    let oriented = key.starts_with("hero:") || key.starts_with("monster:");
    let mut binary = Binary::default();
    let mut meshes: Vec<Value> = Vec::new();
    // Les géométries (une par forme) et les maillages (une géométrie et un matériau).
    type ShapeKey = (u8, i32, i32, bool);
    let mut geometry_of: HashMap<ShapeKey, [usize; 4]> = HashMap::new();
    let mut mesh_of: HashMap<(ShapeKey, usize), usize> = HashMap::new();
    let mut materials: Vec<Value> = Vec::new();
    let mut material_of: HashMap<(Surface, [u8; 3], bool, u8), usize> = HashMap::new();
    let (mut textures, mut images, mut samplers) = (Vec::new(), Vec::new(), Vec::new());
    let mut face_texture = None;
    if let Some(png) = face_png {
        let view = binary.bytes(png);
        images.push(json!({ "bufferView": view, "mimeType": "image/png", "name": "face" }));
        samplers.push(json!({ "magFilter": 9729, "minFilter": 9729, "wrapS": 33071, "wrapT": 33071 }));
        textures.push(json!({ "source": 0, "sampler": 0 }));
        face_texture = Some(0);
    }
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;

    // Les nœuds: un par pièce du repos, dans leur pose de repos.
    let mut nodes: Vec<Value> = Vec::new();
    for (index, part) in rest.iter().enumerate() {
        let surface = Surface::of(part);
        let textured = surface == Surface::Face && face_texture.is_some();
        let shape_key = match part.shape {
            LiveShape::Sphere => (0u8, 0, 0),
            LiveShape::Cube => (1, 0, 0),
            LiveShape::Capsule(_) => (2, 0, 0),
            LiveShape::Frustum(a, b) => (3, (a * 100.0).round() as i32, (b * 100.0).round() as i32),
        };
        let shape_key: ShapeKey = (shape_key.0, shape_key.1, shape_key.2, textured);
        let accessors = *geometry_of.entry(shape_key).or_insert_with(|| {
            let geometry = match part.shape {
                LiveShape::Sphere => sphere(textured, face_map(key)),
                LiveShape::Cube => cube(),
                LiveShape::Capsule(_) => capsule(),
                LiveShape::Frustum(a, b) => frustum(a, b),
            };
            add_geometry(&mut binary, &geometry)
        });
        let color = [byte(part.color[0]), byte(part.color[1]), byte(part.color[2])];
        let material = *material_of.entry((surface, color, part.glow, byte(part.shine))).or_insert_with(|| {
            let (roughness, metallic) = surface.pbr();
            let base = [linear(part.color[0]), linear(part.color[1]), linear(part.color[2]), 1.0];
            let mut material = json!({
                "name": format!("{}_{:02x}{:02x}{:02x}", surface.name(), color[0], color[1], color[2]),
                "pbrMetallicRoughness": { "baseColorFactor": if textured { json!([1.0, 1.0, 1.0, 1.0]) } else { json!(base) }, "metallicFactor": metallic, "roughnessFactor": roughness },
                "doubleSided": false,
            });
            if textured {
                material["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": face_texture.unwrap_or(0) });
            }
            if part.glow {
                material["emissiveFactor"] = json!([base[0], base[1], base[2]]);
            }
            materials.push(material);
            materials.len() - 1
        });
        let mesh = *mesh_of.entry((shape_key, material)).or_insert_with(|| {
            meshes.push(json!({
                "name": format!("{}_{}", shape_name(part.shape), meshes.len()),
                "primitives": [{
                    "attributes": { "POSITION": accessors[0], "NORMAL": accessors[1], "TEXCOORD_0": accessors[2] },
                    "indices": accessors[3],
                    "material": material,
                }]
            }));
            meshes.len() - 1
        });
        let (translation, rotation, scale) = transform(part);
        nodes.push(json!({
            "name": format!("{index:03}_{}_{}", surface.name(), shape_name(part.shape)),
            "mesh": mesh,
            "translation": translation.to_array(),
            "rotation": rotation.to_array(),
            "scale": scale.to_array(),
            "extras": { "surface": surface.name(), "shine": part.shine, "glow": part.glow },
        }));
    }
    // Les animations.
    let clips: Vec<(&str, usize)> = if oriented { CLIPS.to_vec() } else { vec![("Idle", 20)] };
    let mut animations: Vec<Value> = Vec::new();
    for (name, frames) in clips {
        let anim = |i: usize| {
            let t = i as f32 / frames as f32;
            match name {
                "Walk" => Anim::Walk(t),
                "Attack" => Anim::Attack(t),
                _ => Anim::Idle(t),
            }
        };
        // Les pièces de chaque image, alignées sur celles du repos (le coup en ajoute: les traînées de l'arme).
        let poses: Vec<Vec<Option<(Vec3, Quat, Vec3)>>> = (0..=frames)
            .map(|i| {
                let parts = live_parts(key, theme, anim(i % frames));
                align(&rest, &parts).into_iter().map(|found| found.map(|j| transform(&parts[j]))).collect()
            })
            .collect();
        let times: Vec<f32> = (0..=frames).map(|i| i as f32 / FPS).collect();
        let input = binary.floats(&times, 1, "SCALAR", true);
        let (mut samplers_json, mut channels) = (Vec::new(), Vec::new());
        for node in 0..nodes.len() {
            let mut translations = Vec::new();
            let mut rotations: Vec<f32> = Vec::new();
            let mut scales = Vec::new();
            let mut previous: Option<Quat> = None;
            for pose in &poses {
                let (t, r, s) = pose[node].unwrap_or_else(|| (Vec3::from(rest[node].center), previous.unwrap_or(Quat::IDENTITY), Vec3::ZERO));
                // Le signe d'un quaternion est arbitraire: on garde le plus proche du précédent pour ne pas tourner par le long chemin.
                let r = match previous {
                    Some(p) if p.dot(r) < 0.0 => -r,
                    _ => r,
                };
                previous = Some(r);
                translations.extend(t.to_array());
                rotations.extend(r.to_array());
                scales.extend(s.to_array());
            }
            for (path, values, width, kind) in [("translation", translations, 3, "VEC3"), ("rotation", rotations, 4, "VEC4"), ("scale", scales, 3, "VEC3")] {
                let output = binary.floats(&values, width, kind, false);
                samplers_json.push(json!({ "input": input, "output": output, "interpolation": "LINEAR" }));
                channels.push(json!({ "sampler": samplers_json.len() - 1, "target": { "node": node, "path": path } }));
            }
        }
        animations.push(json!({ "name": name, "samplers": samplers_json, "channels": channels }));
    }

    // La racine: le modèle, avec ses pièces pour enfants.
    let root = nodes.len();
    let children: Vec<usize> = (0..nodes.len()).collect();
    nodes.push(json!({ "name": key.replace(':', "_"), "children": children }));

    let mut gltf = json!({
        "asset": { "version": "2.0", "generator": "Castle sprite_forge" },
        "scene": 0,
        "scenes": [{ "name": key, "nodes": [root] }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "buffers": [{ "byteLength": binary.data.len() }],
        "bufferViews": binary.views,
        "accessors": binary.accessors,
        "animations": animations,
    });
    if !images.is_empty() {
        gltf["images"] = json!(images);
        gltf["samplers"] = json!(samplers);
        gltf["textures"] = json!(textures);
    }
    Some(pack(&gltf, &binary.data))
}

/// Aligne les pièces d'une image sur celles du repos: pour chaque pièce du repos, l'indice de la pièce correspondante (même forme et même
/// couleur) dans `parts`. Quand l'image a des pièces en plus (traînées de l'arme), elles sont écartées.
fn align(rest: &[LivePart], parts: &[LivePart]) -> Vec<Option<usize>> {
    if rest.len() == parts.len() {
        return (0..rest.len()).map(Some).collect();
    }
    let same = |a: &LivePart, b: &LivePart| {
        std::mem::discriminant(&a.shape) == std::mem::discriminant(&b.shape) && a.color == b.color && a.glow == b.glow
    };
    let mut found = vec![None; rest.len()];
    let (mut i, mut j) = (0, 0);
    while i < rest.len() && j < parts.len() {
        if same(&rest[i], &parts[j]) {
            found[i] = Some(j);
            i += 1;
        }
        j += 1;
    }
    found
}

/// Le conteneur `.glb`: l'en-tête, le JSON, puis le tampon binaire (chaque morceau aligné sur 4 octets).
fn pack(gltf: &Value, binary: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(gltf).expect("JSON");
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut bin = binary.to_vec();
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend(b"glTF");
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((json.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(json);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    out
}

/// Exporte les modèles demandés (tous si `keys` est vide) dans `dir`, un `.glb` chacun. Rend le nombre de fichiers écrits.
pub fn export_all(dir: &std::path::Path, keys: &[String]) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    let all: Vec<String> = models().into_iter().map(|model| model.key).collect();
    let wanted: Vec<&String> = if keys.is_empty() { all.iter().collect() } else { keys.iter().collect() };
    let mut written = 0;
    for key in wanted {
        // La peau du crâne d'un héros, si sa photo réduite est là (voir `client/assets/art/medieval/faces`).
        let face = key.strip_prefix("hero:").and_then(|class| std::fs::read(format!("client/assets/art/medieval/faces/{}.png", class.to_lowercase())).ok());
        match export_glb(key, face.as_deref()) {
            Some(bytes) => {
                let path = dir.join(format!("{}.glb", key.replace(':', "_")));
                std::fs::write(&path, bytes)?;
                println!("{}", path.display());
                written += 1;
            }
            None => eprintln!("modèle inconnu: {key}"),
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(key: &str) -> (gltf::Document, Vec<gltf::buffer::Data>) {
        let bytes = export_glb(key, None).expect("modèle connu");
        let (document, buffers, _) = gltf::import_slice(&bytes).unwrap_or_else(|e| panic!("{key}: fichier invalide: {e}"));
        (document, buffers)
    }

    #[test]
    fn every_model_exports_a_valid_file() {
        for model in models() {
            let (document, _) = read(&model.key);
            assert!(document.nodes().len() > 1, "{}: aucune pièce", model.key);
        }
    }

    #[test]
    fn a_hero_has_one_node_per_part_and_three_animations() {
        let (document, _) = read("hero:Knight");
        let parts = live_parts("hero:Knight", Theme::Medieval, Anim::Idle(0.0)).len();
        // Les pièces, plus la racine qui les regroupe.
        assert_eq!(document.nodes().len(), parts + 1);
        let names: Vec<&str> = document.animations().filter_map(|a| a.name()).collect();
        assert_eq!(names, ["Idle", "Walk", "Attack"]);
        // Une animation déplace chaque pièce: translation, rotation et échelle.
        assert!(document.animations().all(|a| a.channels().count() == parts * 3));
    }

    #[test]
    fn animations_have_a_closed_loop_and_the_right_length() {
        let (document, buffers) = read("hero:Ranger");
        for (animation, frames) in document.animations().zip([25usize, 16, 10]) {
            let channel = animation.channels().next().expect("un canal");
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            let times: Vec<f32> = reader.read_inputs().expect("temps").collect();
            // Une image de plus que la boucle: la dernière reprend la première, pour qu'elle se referme.
            assert_eq!(times.len(), frames + 1);
            assert!((times.last().copied().unwrap_or(0.0) - frames as f32 / FPS).abs() < 1e-6);
        }
    }

    #[test]
    fn the_attack_ignores_the_weapon_trails_that_the_rest_pose_does_not_have() {
        // Pendant un coup, le Forgebraise a jusqu'à 24 pièces de plus (traînée et onde de choc): elles ne deviennent pas des nœuds.
        let rest = live_parts("hero:Forgebraise", Theme::Medieval, Anim::Idle(0.0));
        let swing = live_parts("hero:Forgebraise", Theme::Medieval, Anim::Attack(0.5));
        assert!(swing.len() > rest.len());
        let found = align(&rest, &swing);
        assert_eq!(found.len(), rest.len());
        // Toutes les pièces du repos retrouvent leur correspondante, dans l'ordre.
        assert!(found.iter().all(Option::is_some));
        let indices: Vec<usize> = found.iter().flatten().copied().collect();
        assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn an_animation_frame_is_the_pose_the_game_draws() {
        let (document, buffers) = read("hero:Knight");
        let walk = document.animations().nth(1).expect("la marche");
        // Une pièce quelconque, à la quatrième image de la marche (seize images par boucle).
        let node = 40;
        let frame = 3;
        let game = live_parts("hero:Knight", Theme::Medieval, Anim::Walk(frame as f32 / 16.0));
        let (expected_translation, _, expected_scale) = transform(&game[node]);
        for channel in walk.channels().filter(|c| c.target().node().index() == node) {
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            match reader.read_outputs().expect("valeurs") {
                gltf::animation::util::ReadOutputs::Translations(values) => {
                    let found = Vec3::from(values.collect::<Vec<_>>()[frame]);
                    assert!((found - expected_translation).length() < 1e-5, "{found} != {expected_translation}");
                }
                gltf::animation::util::ReadOutputs::Scales(values) => {
                    let found = Vec3::from(values.collect::<Vec<_>>()[frame]);
                    assert!((found - expected_scale).length() < 1e-5);
                }
                gltf::animation::util::ReadOutputs::Rotations(_) => {}
                _ => panic!("sortie inattendue"),
            }
        }
    }

    #[test]
    fn parts_keep_the_game_placement_and_colours() {
        let (document, _) = read("hero:Knight");
        let first = live_parts("hero:Knight", Theme::Medieval, Anim::Idle(0.0));
        let node = document.nodes().next().expect("une pièce");
        let (translation, _, _) = node.transform().decomposed();
        assert!((Vec3::from(translation) - Vec3::from(first[0].center)).length() < 1e-5);
        // Les noms disent la matière et la forme, pour s'y retrouver dans Blender.
        assert!(node.name().is_some_and(|name| name.starts_with("000_")));
        let metal = document.materials().find(|m| m.name().is_some_and(|n| n.starts_with("Metal_"))).expect("de l'acier");
        assert!(metal.pbr_metallic_roughness().metallic_factor() > 0.8);
    }

    #[test]
    fn the_skull_carries_the_photo_skin_when_it_is_given() {
        let png = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../client/assets/art/medieval/faces/knight.png")).expect("peau du Chevalier");
        let bytes = export_glb("hero:Knight", Some(&png)).expect("modèle connu");
        let (document, _, images) = gltf::import_slice(&bytes).expect("fichier valide");
        assert_eq!(images.len(), 1);
        assert_eq!((images[0].width, images[0].height), (64, 64));
        let skin = document.materials().find(|m| m.name().is_some_and(|n| n.starts_with("Face_"))).expect("le crâne");
        assert!(skin.pbr_metallic_roughness().base_color_texture().is_some());
    }

    #[test]
    fn a_prop_has_one_loop_and_an_unknown_key_exports_nothing() {
        let (document, _) = read("prop:Coin");
        assert_eq!(document.animations().count(), 1);
        assert!(export_glb("hero:Nobody", None).is_none());
    }
}

#[cfg(test)]
mod winding {
    use super::*;

    /// Chaque triangle d'une forme convexe centrée à l'origine doit regarder vers l'extérieur: sa normale (le sens des sommets, dans glTF
    /// l'avant d'un triangle est le sens trigonométrique) va dans le sens de son centre. Sinon le logiciel 3D supprime la face et on voit
    /// l'intérieur de la pièce.
    fn inward_triangles(g: &Geometry) -> usize {
        g.indices
            .chunks(3)
            .filter(|t| {
                let p = |i: u32| Vec3::from(g.positions[i as usize]);
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                let normal = (b - a).cross(c - a);
                // Les triangles quasi nuls des pôles (de l'ordre de 1e-9) ne comptent pas.
                normal.length() > 1e-6 && normal.dot((a + b + c) / 3.0) < 0.0
            })
            .count()
    }

    #[test]
    fn every_shape_faces_outward() {
        for (name, geometry) in [
            ("sphère", sphere(false, None)),
            ("cube", cube()),
            ("capsule", capsule()),
            ("cône", frustum(1.0, 0.5)),
            ("cylindre", frustum(1.0, 1.0)),
            ("cône inversé", frustum(0.4, 1.0)),
        ] {
            assert_eq!(inward_triangles(&geometry), 0, "{name}: des triangles regardent vers l'intérieur");
        }
    }

    /// La normale écrite dans le fichier doit aussi aller dans le sens où le triangle regarde.
    #[test]
    fn the_written_normals_agree_with_the_triangles() {
        for geometry in [sphere(false, None), cube(), capsule(), frustum(1.0, 0.5)] {
            let disagreeing = geometry
                .indices
                .chunks(3)
                .filter(|t| {
                    let p = |i: u32| Vec3::from(geometry.positions[i as usize]);
                    let n = |i: u32| Vec3::from(geometry.normals[i as usize]);
                    let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
                    face.length() > 1e-6 && face.dot(n(t[0]) + n(t[1]) + n(t[2])) < 0.0
                })
                .count();
            assert_eq!(disagreeing, 0);
        }
    }
}
