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
    if cfg!(target_os = "macos") {
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

fn path_of(name: &str) -> PathBuf {
    save_dir().join(format!("{}.ron", file_stem(name)))
}

fn read(path: &std::path::Path) -> Option<SaveGame> {
    let text = std::fs::read_to_string(path).ok()?;
    let save: SaveGame = ron::from_str(&text).ok()?;
    (save.version <= SAVE_VERSION).then_some(save)
}

/// Charge la partie de ce nom, si elle existe et si elle est lisible.
pub fn load(name: &str) -> Option<SaveGame> {
    read(&path_of(name))
}

/// Écrit la partie. Le fichier précédent est gardé en `.bak`, et l'écriture passe par un fichier temporaire
/// pour qu'une interruption ne laisse jamais une sauvegarde à moitié écrite.
pub fn write(save: &SaveGame) -> std::io::Result<()> {
    let mut save = save.clone();
    save.saved_at = now();
    let path = path_of(&save.name);
    std::fs::create_dir_all(save_dir())?;
    let text = ron::ser::to_string_pretty(&save, ron::ser::PrettyConfig::default())
        .map_err(std::io::Error::other)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    if path.exists() {
        let _ = std::fs::copy(&path, path.with_extension("bak"));
    }
    std::fs::rename(&tmp, &path)
}

/// Supprime une partie (et sa copie de secours).
pub fn delete(name: &str) {
    let path = path_of(name);
    let _ = std::fs::remove_file(path.with_extension("bak"));
    let _ = std::fs::remove_file(path);
}

/// Les parties de ce mode, la plus récente d'abord.
pub fn list(mode: GameMode) -> Vec<SaveGame> {
    let Ok(entries) = std::fs::read_dir(save_dir()) else {
        return Vec::new();
    };
    let mut saves: Vec<SaveGame> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .filter_map(|p| read(&p))
        .filter(|s| s.mode == mode)
        .collect();
    saves.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
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
    }
}
