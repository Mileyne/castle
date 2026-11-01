//! Réglages du joueur, et plateforme de l'appareil.
//!
//! Pour l'instant un seul réglage: afficher ou non les commandes tactiles (boutons et joystick à l'écran). Par défaut ils
//! sont affichés sur iPhone et iPad seulement, jamais sur Apple TV ni sur ordinateur; le joueur peut en décider autrement dans
//! les réglages (écran titre ou menu de pause). Le choix est gardé dans un petit fichier à côté des parties.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

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

/// Volumes réglés par le joueur, de 0 à 10 (dixièmes du volume maximal): musique, voix, bruitages.
static VOLUMES: [AtomicU8; 3] = [AtomicU8::new(10), AtomicU8::new(10), AtomicU8::new(10)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Music,
    Voice,
    Sfx,
}

impl Channel {
    const KEYS: [&'static str; 3] = ["volume_music", "volume_voice", "volume_sfx"];

    fn index(self) -> usize {
        self as usize
    }
}

/// Le volume réglé pour ce canal, de 0.0 à 1.0.
pub fn volume(channel: Channel) -> f32 {
    f32::from(VOLUMES[channel.index()].load(Ordering::Relaxed)) / 10.0
}

/// Le volume réglé, en dixièmes (0 à 10).
pub fn volume_tenths(channel: Channel) -> u8 {
    VOLUMES[channel.index()].load(Ordering::Relaxed)
}

/// Change le volume d'un canal de `delta` dixièmes (borné entre 0 et 10) et l'enregistre.
pub fn adjust_volume(channel: Channel, delta: i8) {
    let now = i16::from(volume_tenths(channel)) + i16::from(delta);
    VOLUMES[channel.index()].store(now.clamp(0, 10) as u8, Ordering::Relaxed);
    save();
}

/// Passe au palier suivant (de 20 % en 20 %, puis retour à 0), pour une validation sans flèches.
pub fn cycle_volume(channel: Channel) {
    let next = (volume_tenths(channel) / 2 * 2 + 2) % 12;
    VOLUMES[channel.index()].store(next.min(10), Ordering::Relaxed);
    save();
}

/// La vue 3D est-elle active? (Le décor en 3D est un prototype: `CASTLE_DECOR3D=1`.)
pub fn view3d_enabled() -> bool {
    std::env::var_os("CASTLE_DECOR3D").is_some()
}

/// Hauteur des murs de la vue 3D, en tuiles.
pub const VIEW_WALL_TILES: f32 = 3.0;
/// Cadrage de la vue 3D: la part de terrain montrée, 1 étant celle de la version 2D.
pub const VIEW_ZOOM: f32 = 1.25;
/// Inclinaison et angle de la vue par défaut, en degrés. Le stick droit de la manette principale les modifie en jouant (voir `turn_view`).
pub const VIEW_TILT_DEFAULT: f32 = 52.0;
pub const VIEW_ANGLE_DEFAULT: f32 = -15.0;
/// Limites de l'inclinaison: de presque à l'horizontale à presque vue d'en haut.
const VIEW_TILT_RANGE: (f32, f32) = (25.0, 80.0);

/// Inclinaison de la caméra au-dessus de l'horizon, en degrés (bits d'un `f32`).
static VIEW_TILT: AtomicU32 = AtomicU32::new(VIEW_TILT_DEFAULT.to_bits());
/// Angle de la caméra autour du décor, en degrés (0: elle est au sud et regarde vers le nord).
static VIEW_ANGLE: AtomicU32 = AtomicU32::new(VIEW_ANGLE_DEFAULT.to_bits());

/// L'inclinaison de la vue, en degrés.
pub fn view_tilt() -> f32 {
    f32::from_bits(VIEW_TILT.load(Ordering::Relaxed))
}

/// L'angle de la vue, en degrés.
pub fn view_angle() -> f32 {
    f32::from_bits(VIEW_ANGLE.load(Ordering::Relaxed))
}

/// Angle de la caméra autour du décor, en radians; zéro sans la vue 3D. Les commandes de direction en tiennent compte.
pub fn view_yaw() -> f32 {
    if view3d_enabled() { view_angle().to_radians() } else { 0.0 }
}

/// Tourne la vue de `angle` degrés et l'incline de `tilt` degrés (l'angle fait le tour, l'inclinaison reste bornée).
pub fn turn_view(angle: f32, tilt: f32) {
    let angle = (view_angle() + angle + 180.0).rem_euclid(360.0) - 180.0;
    VIEW_ANGLE.store(angle.to_bits(), Ordering::Relaxed);
    let tilt = (view_tilt() + tilt).clamp(VIEW_TILT_RANGE.0, VIEW_TILT_RANGE.1);
    VIEW_TILT.store(tilt.to_bits(), Ordering::Relaxed);
}

/// Remet l'angle et l'inclinaison par défaut.
pub fn reset_view() {
    VIEW_ANGLE.store(VIEW_ANGLE_DEFAULT.to_bits(), Ordering::Relaxed);
    VIEW_TILT.store(VIEW_TILT_DEFAULT.to_bits(), Ordering::Relaxed);
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

fn load() {
    let Some(text) = castle_shared::save::read_settings() else {
        return;
    };
    for line in text.lines() {
        if let Some((key, value)) = line.trim().split_once('=') {
            if let (Some(i), Ok(tenths)) = (Channel::KEYS.iter().position(|k| *k == key), value.parse::<u8>()) {
                VOLUMES[i].store(tenths.min(10), Ordering::Relaxed);
                continue;
            }
        }
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
    let volumes: String = Channel::KEYS.iter().zip(&VOLUMES).map(|(k, v)| format!("{k}={}\n", v.load(Ordering::Relaxed))).collect();
    if let Err(err) = castle_shared::save::write_settings(&format!("{touch}{pad}{size}{volumes}")) {
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

/// Mémoire physique de l'appareil, en octets (0 si elle est inconnue).
fn physical_memory() -> u64 {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        unsafe extern "C" {
            fn sysctlbyname(name: *const std::ffi::c_char, old: *mut std::ffi::c_void, old_len: *mut usize, new: *mut std::ffi::c_void, new_len: usize) -> i32;
        }
        let mut bytes: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        // SAFETY: `hw.memsize` est un entier de 64 bits; `len` donne la taille du tampon.
        let status = unsafe { sysctlbyname(c"hw.memsize".as_ptr(), (&raw mut bytes).cast(), &raw mut len, std::ptr::null_mut(), 0) };
        if status == 0 { bytes } else { 0 }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        0
    }
}

/// L'appareil a peu de mémoire (moins de 3 Go: Apple TV HD...): le jeu charge alors des planches de sprites moins grandes.
/// `CASTLE_LITE=1` (ou `0`) force le choix, pour l'essayer sur ordinateur.
pub fn low_memory() -> bool {
    match std::env::var("CASTLE_LITE").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => (1..3 * 1024 * 1024 * 1024).contains(&physical_memory()),
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;

    /// Les deux réglages vivent dans des variables globales: tout se vérifie dans un seul test.
    #[test]
    fn the_view_turns_wraps_clamps_and_resets() {
        reset_view();
        assert_eq!((view_angle(), view_tilt()), (VIEW_ANGLE_DEFAULT, VIEW_TILT_DEFAULT));
        // L'angle fait le tour: -15 + 200 = 185, soit -175.
        turn_view(200.0, 0.0);
        assert!((view_angle() - -175.0).abs() < 1e-3, "{}", view_angle());
        // L'inclinaison reste dans ses bornes.
        turn_view(0.0, 500.0);
        assert_eq!(view_tilt(), 80.0);
        turn_view(0.0, -500.0);
        assert_eq!(view_tilt(), 25.0);
        reset_view();
        assert_eq!((view_angle(), view_tilt()), (VIEW_ANGLE_DEFAULT, VIEW_TILT_DEFAULT));
    }
}
