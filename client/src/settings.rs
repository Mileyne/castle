//! Réglages du joueur, et plateforme de l'appareil.
//!
//! Pour l'instant un seul réglage: afficher ou non les commandes tactiles (boutons et joystick à l'écran). Par défaut ils
//! sont affichés sur iPhone et iPad seulement, jamais sur Apple TV ni sur ordinateur; le joueur peut en décider autrement dans
//! les réglages (écran titre ou menu de pause). Le choix est gardé dans un petit fichier à côté des parties.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use bevy::prelude::*;

/// 0: pas de choix du joueur (valeur par défaut de la plateforme), 1: sans commandes tactiles, 2: avec.
static TOUCH_CHOICE: AtomicU8 = AtomicU8::new(0);

/// Disposition des quatre boutons de la manette. Les boutons sont lus par position (haut, bas, gauche, droite), mais les
/// manettes Nintendo (Switch, 8BitDo SN30...) nomment A, B, X et Y à d'autres places que celles de Xbox.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PadLayout {
    /// A en bas, B à droite, X à gauche, Y en haut.
    Xbox,
    /// B en bas, A à droite, Y à gauche, X en haut.
    Nintendo,
}

/// Taille de l'interface: 0 par défaut de la plateforme (petite sur Apple TV, normale ailleurs), 1 petite, 2 normale, 3 grande.
static UI_SIZE: AtomicU8 = AtomicU8::new(0);

/// Le facteur d'échelle de l'interface (textes, panneaux, menus), selon le choix du joueur. L'Apple TV rapporte une fenêtre de
/// 1920 x 1080 points (3840 x 2160 pixels): l'interface y est un peu trop grande, donc plus petite par défaut.
pub fn ui_scale() -> f32 {
    match UI_SIZE.load(Ordering::Relaxed) {
        1 => 0.75,
        2 => 1.0,
        3 => 1.3,
        _ if is_tv() => 0.8,
        _ => 1.0,
    }
}

/// Le choix du joueur pour la taille de l'interface (0 par défaut, 1 petite, 2 normale, 3 grande).
pub fn ui_size_choice() -> u8 {
    UI_SIZE.load(Ordering::Relaxed)
}

/// Passe à la taille suivante (par défaut, petite, normale, grande) et l'enregistre.
pub fn cycle_ui_size() {
    UI_SIZE.store((UI_SIZE.load(Ordering::Relaxed) + 1) % 4, Ordering::Relaxed);
    save();
}

/// Choix du joueur: 0 automatique (d'après le nom de la manette), 1 Xbox, 2 Nintendo.
static PAD_CHOICE: AtomicU8 = AtomicU8::new(0);
/// Disposition reconnue d'après le nom de la manette branchée: 0 inconnue, 1 Xbox, 2 Nintendo.
static PAD_DETECTED: AtomicU8 = AtomicU8::new(0);

/// La disposition en vigueur: le choix du joueur, sinon celle que le nom de la manette laisse deviner.
pub fn pad_layout() -> PadLayout {
    match PAD_CHOICE.load(Ordering::Relaxed) {
        1 => PadLayout::Xbox,
        2 => PadLayout::Nintendo,
        _ => {
            if PAD_DETECTED.load(Ordering::Relaxed) == 2 {
                PadLayout::Nintendo
            } else {
                PadLayout::Xbox
            }
        }
    }
}

/// Le choix du joueur, pour l'afficher: 0 automatique, 1 Xbox, 2 Nintendo.
pub fn pad_choice() -> u8 {
    PAD_CHOICE.load(Ordering::Relaxed)
}

/// Passe au choix suivant (automatique, Xbox, Nintendo) et l'enregistre.
pub fn cycle_pad_choice() {
    PAD_CHOICE.store((PAD_CHOICE.load(Ordering::Relaxed) + 1) % 3, Ordering::Relaxed);
    save();
}

/// Une manette est branchée: son nom dit si c'est une Nintendo ou une 8BitDo (disposition Nintendo) ou non.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn note_controller(name: &str) {
    let name = name.to_lowercase();
    let nintendo = ["8bitdo", "nintendo", "switch", "joy-con", "pro controller", "sn30"].iter().any(|n| name.contains(n));
    PAD_DETECTED.store(if nintendo { 2 } else { 1 }, Ordering::Relaxed);
}

/// L'appareil est-il une Apple TV ? (L'app tvOS est compilée comme une app iOS: on interroge le système.)
pub fn is_tv() -> bool {
    static TV: OnceLock<bool> = OnceLock::new();
    *TV.get_or_init(detect_tv)
}

#[cfg(target_os = "ios")]
fn detect_tv() -> bool {
    use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
    // UIUserInterfaceIdiomTV vaut 2 (3 est CarPlay).
    let idiom: isize = unsafe {
        let device: Option<Retained<AnyObject>> = msg_send![class!(UIDevice), currentDevice];
        match device {
            Some(device) => msg_send![&*device, userInterfaceIdiom],
            None => 0,
        }
    };
    idiom == 2
}

/// Sur ordinateur: `CASTLE_TV=1` fait comme si c'était une Apple TV (pour essayer l'interface sans le téléviseur).
#[cfg(not(target_os = "ios"))]
fn detect_tv() -> bool {
    std::env::var_os("CASTLE_TV").is_some()
}

/// Les commandes tactiles sont-elles affichées par défaut sur cet appareil ?
pub fn touch_default() -> bool {
    (cfg!(target_os = "ios") && !is_tv()) || cfg!(target_os = "android") || std::env::var_os("CASTLE_TOUCH").is_some()
}

/// Vrai si les commandes tactiles sont affichées (choix du joueur, sinon valeur par défaut de la plateforme).
pub fn touch_ui() -> bool {
    match TOUCH_CHOICE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => touch_default(),
    }
}

/// Change le choix du joueur et l'enregistre.
pub fn set_touch_ui(on: bool) {
    TOUCH_CHOICE.store(if on { 2 } else { 1 }, Ordering::Relaxed);
    save();
}

fn path() -> PathBuf {
    let saves = castle_shared::save::save_dir();
    saves.parent().map(PathBuf::from).unwrap_or(saves).join("settings.txt")
}

fn load() {
    let Ok(text) = std::fs::read_to_string(path()) else {
        return;
    };
    for line in text.lines() {
        match line.trim() {
            "touch_ui=on" => TOUCH_CHOICE.store(2, Ordering::Relaxed),
            "touch_ui=off" => TOUCH_CHOICE.store(1, Ordering::Relaxed),
            "pad=xbox" => PAD_CHOICE.store(1, Ordering::Relaxed),
            "pad=nintendo" => PAD_CHOICE.store(2, Ordering::Relaxed),
            "ui_size=small" => UI_SIZE.store(1, Ordering::Relaxed),
            "ui_size=normal" => UI_SIZE.store(2, Ordering::Relaxed),
            "ui_size=large" => UI_SIZE.store(3, Ordering::Relaxed),
            _ => {}
        }
    }
}

fn save() {
    let touch = match TOUCH_CHOICE.load(Ordering::Relaxed) {
        1 => "touch_ui=off\n",
        2 => "touch_ui=on\n",
        _ => "",
    };
    let pad = match PAD_CHOICE.load(Ordering::Relaxed) {
        1 => "pad=xbox\n",
        2 => "pad=nintendo\n",
        _ => "",
    };
    let size = match UI_SIZE.load(Ordering::Relaxed) {
        1 => "ui_size=small\n",
        2 => "ui_size=normal\n",
        3 => "ui_size=large\n",
        _ => "",
    };
    let path = path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(err) = std::fs::write(&path, format!("{touch}{pad}{size}")) {
        warn!("cannot write settings: {err}");
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        load();
        app.add_systems(Update, apply_ui_scale);
    }
}

/// Applique la taille d'interface choisie à tout l'affichage (textes, menus, panneaux).
fn apply_ui_scale(mut scale: ResMut<UiScale>) {
    let wanted = ui_scale();
    if scale.0 != wanted {
        scale.0 = wanted;
    }
}
