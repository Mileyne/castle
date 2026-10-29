//! Réglages des modèles faits dans l'éditeur: proportions du corps et du visage, et, pour chaque matière d'un modèle
//! (toutes les parties de la même couleur d'origine), sa couleur, sa taille, sa brillance et sa lueur.
//!
//! Les réglages sont enregistrés dans `art_overrides/<thème>/models.ron` et appliqués par-dessus ce que le code des
//! modèles produit, sans le modifier. Un modèle sans réglage donne exactement le même rendu qu'avant.

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::render::Part;
use crate::shapes::{Theme, rgb};

/// Réglages de tous les modèles, par clé de modèle (`hero:Knight`, `monster:Bat`, `prop:Coin`...).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tuning {
    pub models: BTreeMap<String, ModelTuning>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelTuning {
    /// Taille de tout le modèle (1 = inchangé), pieds au sol.
    pub size: f32,
    /// Proportions: `dims.<nom>` pour le corps, `face.<nom>` pour le visage; valeur absolue.
    pub params: BTreeMap<String, f32>,
    /// Matières, par couleur d'origine (0xRRGGBB).
    pub materials: BTreeMap<u32, Material>,
}

impl Default for ModelTuning {
    fn default() -> Self {
        Self { size: 1.0, params: BTreeMap::new(), materials: BTreeMap::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Material {
    /// Nouvelle couleur (0xRRGGBB).
    pub color: Option<u32>,
    /// Taille des parties de cette matière (1 = inchangé), autour de leur centre.
    pub size: f32,
    /// Reflet brillant, de 0 (mat) à 1.
    pub shine: Option<f32>,
    /// Partie lumineuse: pas d'ombrage.
    pub glow: Option<bool>,
}

impl Default for Material {
    fn default() -> Self {
        Self { color: None, size: 1.0, shine: None, glow: None }
    }
}

impl Material {
    /// Vrai si ce réglage ne change rien.
    pub fn is_default(&self) -> bool {
        *self == Material::default()
    }
}

/// Les réglages appliqués par les fonctions `build` des modèles.
static ACTIVE: RwLock<Tuning> = RwLock::new(Tuning { models: BTreeMap::new() });

pub fn set_active(tuning: Tuning) {
    *ACTIVE.write().unwrap() = tuning;
}

pub fn active() -> Tuning {
    ACTIVE.read().unwrap().clone()
}

/// Exécute `f` sans aucun réglage (pour connaître les valeurs d'origine), puis remet les réglages en place.
pub fn without_tuning<T>(f: impl FnOnce() -> T) -> T {
    let saved = std::mem::take(&mut *ACTIVE.write().unwrap());
    let result = f();
    *ACTIVE.write().unwrap() = saved;
    result
}

fn file(theme: Theme) -> std::path::PathBuf {
    crate::overrides::root().join(theme.folder()).join("models.ron")
}

/// Réglages enregistrés d'un thème (vides s'il n'y en a pas ou si le fichier est illisible).
pub fn load(theme: Theme) -> Tuning {
    std::fs::read_to_string(file(theme)).ok().and_then(|text| ron::from_str(&text).ok()).unwrap_or_default()
}

pub fn save(theme: Theme, tuning: &Tuning) -> std::io::Result<()> {
    let path = file(theme);
    std::fs::create_dir_all(path.parent().expect("le fichier est dans un dossier"))?;
    let text = ron::ser::to_string_pretty(tuning, ron::ser::PrettyConfig::default()).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

/// Valeur réglée d'un paramètre de proportions, ou `default`.
pub(crate) fn param(model: &str, name: &str, default: f32) -> f32 {
    ACTIVE.read().unwrap().models.get(model).and_then(|m| m.params.get(name)).copied().unwrap_or(default)
}

pub(crate) fn hero_key(class: crate::heroes::Class) -> String {
    format!("hero:{class:?}")
}

pub(crate) fn monster_key(monster: crate::monsters::Monster) -> String {
    format!("monster:{monster:?}")
}

pub(crate) fn prop_key(prop: crate::props::Prop) -> String {
    format!("prop:{prop:?}")
}

/// Couleur d'origine d'une partie, au format 0xRRGGBB.
pub fn hex_of(color: [f32; 3]) -> u32 {
    let [r, g, b] = color.map(|c| (c * 255.0 + 0.5) as u32);
    (r << 16) | (g << 8) | b
}

/// Applique matières et taille aux parties d'un modèle.
pub(crate) fn apply(model: &str, parts: &mut [Part]) {
    let tuning = ACTIVE.read().unwrap();
    let Some(tuned) = tuning.models.get(model) else {
        return;
    };
    for part in parts.iter_mut() {
        let Some(material) = tuned.materials.get(&hex_of(part.color)) else {
            continue;
        };
        if let Some(color) = material.color {
            part.color = rgb(color);
        }
        part.scale *= material.size;
        if let Some(shine) = material.shine {
            part.shine = shine;
        }
        if let Some(glow) = material.glow {
            part.glow = glow;
        }
    }
    if tuned.size != 1.0 {
        for part in parts.iter_mut() {
            part.center *= tuned.size;
            part.scale *= tuned.size;
        }
    }
}

/// Les tests partagent les réglages actifs (un état global): ils passent un par un.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Shape;
    use glam::{Mat3, Vec3};

    fn part(color: u32) -> Part {
        Part { shape: Shape::Sphere, center: Vec3::new(0.0, 1.0, 0.0), rot: Mat3::IDENTITY, scale: Vec3::splat(0.5), color: rgb(color), glow: false, shine: 0.0 }
    }

    #[test]
    fn colors_round_trip_through_hex() {
        for hex in [0x000000, 0xffffff, 0x123456, 0xfad2b4, 0x8a1c28] {
            assert_eq!(hex_of(rgb(hex)), hex);
        }
    }

    #[test]
    fn material_and_size_apply_to_matching_parts_only() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut tuned = ModelTuning { size: 2.0, ..Default::default() };
        tuned.materials.insert(0xff0000, Material { color: Some(0x00ff00), size: 1.5, shine: Some(0.8), glow: Some(true) });
        let mut tuning = Tuning::default();
        tuning.models.insert("test:model".into(), tuned);
        set_active(tuning);
        let mut parts = [part(0xff0000), part(0x0000ff)];
        apply("test:model", &mut parts);
        assert_eq!(hex_of(parts[0].color), 0x00ff00);
        assert!(parts[0].glow && parts[0].shine == 0.8);
        // Matière 1,5 puis modèle 2: 0,5 * 1,5 * 2.
        assert!((parts[0].scale.x - 1.5).abs() < 1e-5, "{}", parts[0].scale.x);
        assert_eq!(hex_of(parts[1].color), 0x0000ff);
        assert!((parts[1].scale.x - 1.0).abs() < 1e-5);
        assert!((parts[1].center.y - 2.0).abs() < 1e-5);
        set_active(Tuning::default());
    }

    #[test]
    fn saved_tuning_reads_back() {
        let mut tuning = Tuning::default();
        let mut m = ModelTuning::default();
        m.params.insert("dims.thigh".into(), 0.5);
        m.materials.insert(0x112233, Material { color: Some(0x445566), ..Default::default() });
        tuning.models.insert("hero:Knight".into(), m);
        let text = ron::ser::to_string_pretty(&tuning, ron::ser::PrettyConfig::default()).unwrap();
        assert_eq!(ron::from_str::<Tuning>(&text).unwrap(), tuning);
    }
}
