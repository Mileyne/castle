//! Parties sauvegardées: une campagne nommée, écrite à chaque entrée de niveau.
//!
//! La sauvegarde ne contient pas le monde: le château d'un niveau se régénère à l'identique depuis l'état du
//! générateur aléatoire au moment d'y entrer (`entry_rng`). Un retour au début du niveau (équipe à terre) et un
//! chargement reprennent donc exactement la même chose.

use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Version du format; une sauvegarde plus récente que le jeu est ignorée.
pub const SAVE_VERSION: u32 = 1;
/// Longueur maximale du nom d'une partie, en caractères.
pub const SAVE_NAME_MAX: usize = 24;

/// Mode de jeu d'une partie: les parties solo et coopératives sont listées séparément.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    /// Un joueur, serveur local fermé au réseau.
    Solo,
    /// Plusieurs joueurs sur le réseau local: l'hôte détient la sauvegarde.
    Coop,
}

impl GameMode {
    pub fn arg(self) -> &'static str {
        match self {
            GameMode::Solo => "solo",
            GameMode::Coop => "coop",
        }
    }

    pub fn from_arg(arg: &str) -> Option<Self> {
        match arg {
            "solo" => Some(GameMode::Solo),
            "coop" => Some(GameMode::Coop),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveGame {
    pub version: u32,
    pub name: String,
    pub mode: GameMode,
    /// Niveau dans lequel l'équipe est entrée pour la dernière fois.
    pub level: u32,
    /// État du générateur aléatoire du serveur à l'entrée de ce niveau.
    pub entry_rng: u64,
    /// Le dialogue d'introduction a été vu.
    pub intro_done: bool,
    /// Or et clés de chaque équipe (une par joueur, dans l'ordre d'arrivée), tels qu'à l'entrée du niveau.
    pub purses: Vec<(u32, u32)>,
    /// Date de la dernière écriture, en secondes depuis 1970.
    pub saved_at: u64,
}

impl SaveGame {
    /// Une nouvelle campagne, au niveau 1.
    pub fn new(name: &str, mode: GameMode, entry_rng: u64) -> Self {
        Self {
            version: SAVE_VERSION,
            name: clean_name(name),
            mode,
            level: 1,
            entry_rng: entry_rng | 1,
            intro_done: false,
            purses: Vec::new(),
            saved_at: now(),
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Nom d'une partie tel qu'on le garde: sans espaces en bordure, sans caractères de contrôle, de longueur bornée.
pub fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(SAVE_NAME_MAX)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Nom du fichier d'une partie: le nom en minuscules, sans rien qui gêne un système de fichiers.
fn file_stem(name: &str) -> String {
    let stem: String = clean_name(name)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    if stem.is_empty() { "_".into() } else { stem }
}

/// Dossier des sauvegardes (`CASTLE_SAVE_DIR` pour le changer).
pub fn save_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CASTLE_SAVE_DIR") {
        return dir.into();
    }
    let home = || PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(target_os = "ios") {
        // HOME est le conteneur de l'app; Documents est sauvegardé par iCloud et visible dans l'app Fichiers.
        home().join("Documents/saves")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Application Support/Castle/saves")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("Castle/saves")
    } else {
        match std::env::var_os("XDG_DATA_HOME") {
            Some(dir) => PathBuf::from(dir).join("castle/saves"),
            None => home().join(".local/share/castle/saves"),
        }
    }
}

/// Un autre lieu de stockage que les fichiers, pour les appareils où ils ne durent pas (l'Apple TV n'a pas de dossier
/// d'enregistrement durable: le client y branche les préférences du système). Clés: le nom de fichier de la partie, `.bak` pour sa copie.
pub trait Backend: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str);
    fn remove(&self, key: &str);
    fn keys(&self) -> Vec<String>;
}

static BACKEND: std::sync::OnceLock<Box<dyn Backend>> = std::sync::OnceLock::new();

/// Remplace les fichiers par ce lieu de stockage (à appeler une fois, au démarrage).
pub fn use_backend(backend: Box<dyn Backend>) {
    let _ = BACKEND.set(backend);
}

const SETTINGS_KEY: &str = "settings";

/// Les réglages enregistrés (texte), et leur écriture: dans le lieu de stockage s'il y en a un, sinon à côté des parties.
pub fn read_settings() -> Option<String> {
    match BACKEND.get() {
        Some(backend) => backend.get(SETTINGS_KEY),
        None => std::fs::read_to_string(save_dir().parent().map(PathBuf::from).unwrap_or_else(save_dir).join("settings.txt")).ok(),
    }
}

pub fn write_settings(text: &str) -> std::io::Result<()> {
    if let Some(backend) = BACKEND.get() {
        backend.set(SETTINGS_KEY, text);
        return Ok(());
    }
    let dir = save_dir().parent().map(PathBuf::from).unwrap_or_else(save_dir);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("settings.txt"), text)
}

fn path_of(name: &str) -> PathBuf {
    save_dir().join(format!("{}.ron", file_stem(name)))
}

fn parse(text: &str) -> Option<SaveGame> {
    let save: SaveGame = ron::from_str(text).ok()?;
    (save.version <= SAVE_VERSION).then_some(save)
}

fn read(path: &std::path::Path) -> Option<SaveGame> {
    parse(&std::fs::read_to_string(path).ok()?)
}

fn key_of(name: &str) -> String {
    format!("save.{}", file_stem(name))
}

/// Charge la partie de ce nom, si elle existe et si elle est lisible.
pub fn load(name: &str) -> Option<SaveGame> {
    if let Some(backend) = BACKEND.get() {
        return parse(&backend.get(&key_of(name))?);
    }
    read(&path_of(name))
}

/// Écrit la partie. Le fichier précédent est gardé en `.bak`, et l'écriture passe par un fichier temporaire
/// pour qu'une interruption ne laisse jamais une sauvegarde à moitié écrite.
pub fn write(save: &SaveGame) -> std::io::Result<()> {
    let mut save = save.clone();
    save.saved_at = now();
    let text = ron::ser::to_string_pretty(&save, ron::ser::PrettyConfig::default())
        .map_err(std::io::Error::other)?;
    if let Some(backend) = BACKEND.get() {
        let key = key_of(&save.name);
        if let Some(previous) = backend.get(&key) {
            backend.set(&format!("{key}.bak"), &previous);
        }
        backend.set(&key, &text);
        return Ok(());
    }
    let path = path_of(&save.name);
    std::fs::create_dir_all(save_dir())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    if path.exists() {
        let _ = std::fs::copy(&path, path.with_extension("bak"));
    }
    std::fs::rename(&tmp, &path)
}

/// Supprime une partie (et sa copie de secours).
pub fn delete(name: &str) {
    if let Some(backend) = BACKEND.get() {
        let key = key_of(name);
        backend.remove(&format!("{key}.bak"));
        backend.remove(&key);
        return;
    }
    let path = path_of(name);
    let _ = std::fs::remove_file(path.with_extension("bak"));
    let _ = std::fs::remove_file(path);
}

/// Les parties de ce mode, la plus récente d'abord.
pub fn list(mode: GameMode) -> Vec<SaveGame> {
    let mut saves: Vec<SaveGame> = if let Some(backend) = BACKEND.get() {
        backend
            .keys()
            .iter()
            .filter(|k| k.starts_with("save.") && !k.ends_with(".bak"))
            .filter_map(|k| parse(&backend.get(k)?))
            .filter(|s| s.mode == mode)
            .collect()
    } else {
        let Ok(entries) = std::fs::read_dir(save_dir()) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "ron"))
            .filter_map(|p| read(&p))
            .filter(|s| s.mode == mode)
            .collect()
    };
    saves.sort_by_key(|save| std::cmp::Reverse(save.saved_at));
    saves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("castle_save_test_{}", std::process::id()));
        // Un seul test touche à l'environnement, pour ne pas se disputer le dossier avec un autre.
        unsafe { std::env::set_var("CASTLE_SAVE_DIR", &dir) };
        let mut save = SaveGame::new("  Ma Partie / 1  ", GameMode::Coop, 42);
        save.level = 3;
        save.purses = vec![(12, 1), (0, 0)];
        write(&save).unwrap();
        write(&save).unwrap();
        let loaded = load("ma partie / 1").expect("la sauvegarde se relit, quelle que soit la casse");
        assert_eq!(loaded.level, 3);
        assert_eq!(loaded.purses, save.purses);
        assert_eq!(list(GameMode::Coop).len(), 1);
        assert!(list(GameMode::Solo).is_empty());
        delete("Ma Partie / 1");
        assert!(load("Ma Partie / 1").is_none());
        let _ = std::fs::remove_dir_all(dir);

        // Avec un autre lieu de stockage, les parties et les réglages y vont (pas de fichier).
        struct Memory(std::sync::Mutex<std::collections::HashMap<String, String>>);
        impl Backend for Memory {
            fn get(&self, key: &str) -> Option<String> {
                self.0.lock().unwrap().get(key).cloned()
            }
            fn set(&self, key: &str, value: &str) {
                self.0.lock().unwrap().insert(key.into(), value.into());
            }
            fn remove(&self, key: &str) {
                self.0.lock().unwrap().remove(key);
            }
            fn keys(&self) -> Vec<String> {
                self.0.lock().unwrap().keys().cloned().collect()
            }
        }
        use_backend(Box::new(Memory(Default::default())));
        write(&save).unwrap();
        write(&save).unwrap();
        assert_eq!(load("MA PARTIE / 1").map(|s| s.level), Some(3));
        assert_eq!(list(GameMode::Coop).len(), 1, "la copie .bak n'est pas une partie");
        write_settings("pad=xbox\n").unwrap();
        assert_eq!(read_settings().as_deref(), Some("pad=xbox\n"));
        delete("Ma Partie / 1");
        assert!(load("Ma Partie / 1").is_none());
    }
}
