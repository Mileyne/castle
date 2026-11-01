//! Réglages du joueur, et plateforme de l'appareil.
//!
//! Pour l'instant un seul réglage: afficher ou non les commandes tactiles (boutons et joystick à l'écran). Par défaut ils
//! sont affichés sur iPhone et iPad seulement, jamais sur Apple TV ni sur ordinateur; le joueur peut en décider autrement dans
//! les réglages (écran titre ou menu de pause). Le choix est gardé dans un petit fichier à côté des parties.

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

/// Les réglages de la vue 3D sont-ils proposés? (Le décor en 3D est un prototype: `CASTLE_DECOR3D=1`.)
pub fn view3d_enabled() -> bool {
    std::env::var_os("CASTLE_DECOR3D").is_some()
}

/// Angle de la caméra autour du décor, en radians; zéro sans la vue 3D. Les commandes de direction en tiennent compte.
pub fn view_yaw() -> f32 {
    if view3d_enabled() { view_value(View3d::Angle).to_radians() } else { 0.0 }
}

/// Un réglage de la vue 3D.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View3d {
    /// Hauteur des murs, en tuiles.
    Wall,
    /// Inclinaison de la caméra au-dessus de l'horizon, en degrés.
    Tilt,
    /// Angle de la caméra autour du décor, en degrés (0: elle regarde vers le nord).
    Angle,
    /// Cadrage: part de terrain montrée, en pour cent (plus petit: plus près).
    Zoom,
}

impl View3d {
    pub const ALL: [View3d; 4] = [Self::Wall, Self::Tilt, Self::Angle, Self::Zoom];

    const KEYS: [&'static str; 4] = ["view_wall", "view_tilt", "view_angle", "view_zoom"];

    /// Les valeurs proposées, et l'indice de celle par défaut.
    fn steps(self) -> (&'static [f32], u8) {
        match self {
            Self::Wall => (&[0.5, 1.0, 1.5, 2.0, 3.0, 4.0], 4),
            Self::Tilt => (&[25.0, 35.0, 45.0, 52.0, 60.0, 70.0, 80.0], 3),
            Self::Angle => (&[-60.0, -45.0, -30.0, -15.0, 0.0, 15.0, 30.0, 45.0, 60.0], 3),
            Self::Zoom => (&[50.0, 65.0, 80.0, 100.0, 125.0, 160.0, 200.0], 3),
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Indice choisi pour chaque réglage de la vue 3D (255: pas de choix, valeur par défaut).
static VIEW3D: [AtomicU8; 4] = [AtomicU8::new(255), AtomicU8::new(255), AtomicU8::new(255), AtomicU8::new(255)];

fn view_index(view: View3d) -> usize {
    let (steps, default) = view.steps();
    match VIEW3D[view.index()].load(Ordering::Relaxed) {
        255 => default as usize,
        chosen => (chosen as usize).min(steps.len() - 1),
    }
}

/// La valeur du réglage: tuiles, degrés ou pour cent selon le réglage.
pub fn view_value(view: View3d) -> f32 {
    view.steps().0[view_index(view)]
}

/// La valeur du réglage, à afficher.
pub fn view_label(view: View3d) -> String {
    let value = view_value(view);
    match view {
        View3d::Wall => format!("{value}").replace('.', ","),
        View3d::Tilt | View3d::Angle => format!("{value:.0}°"),
        View3d::Zoom => format!("{value:.0} %"),
    }
}

/// Passe à la valeur suivante (`delta` = 1) ou précédente (-1) et l'enregistre; au bout, la liste reprend de l'autre côté si `wrap`.
pub fn view_step(view: View3d, delta: i8, wrap: bool) {
    let count = view.steps().0.len() as i16;
    let next = view_index(view) as i16 + i16::from(delta);
    let next = if wrap { next.rem_euclid(count) } else { next.clamp(0, count - 1) };
    VIEW3D[view.index()].store(next as u8, Ordering::Relaxed);
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
            if let (Some(i), Ok(chosen)) = (View3d::KEYS.iter().position(|k| *k == key), value.parse::<u8>()) {
                VIEW3D[i].store(chosen, Ordering::Relaxed);
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
    let views: String = View3d::KEYS
        .iter()
        .zip(&VIEW3D)
        .filter(|(_, v)| v.load(Ordering::Relaxed) != 255)
        .map(|(k, v)| format!("{k}={}\n", v.load(Ordering::Relaxed)))
        .collect();
    if let Err(err) = castle_shared::save::write_settings(&format!("{touch}{pad}{size}{volumes}{views}")) {
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
