//! Commandes tactiles (iPhone, iPad) et manette.
//!
//! Tout le jeu lit le clavier (`ButtonInput<KeyCode>`): menus, dialogues, pause, héros. Plutôt que de doubler chaque
//! système, les boutons à l'écran et la manette « appuient » sur les touches correspondantes (Entrée, Échap, Espace,
//! E, F, 1 à 4...). Seul le déplacement passe à part, par `Stick`, pour garder la direction fine du joystick.
//!
//! Les boutons affichés dépendent du moment: menus (flèches, OK, Retour), jeu (joystick à gauche, actions à droite,
//! héros, pause), dialogue (bouton OK en plus; pendant un vote, les boutons 1 à 4 votent, comme au clavier). Le pavé
//! d'adresse remplace le clavier pour rejoindre une partie.
//!
//! Les boutons tactiles s'affichent sur iOS et Android, ou sur ordinateur avec `CASTLE_TOUCH=1` (pour les essayer à la
//! souris, il faut un écran tactile). La manette marche partout.

use std::collections::HashMap;
use std::sync::OnceLock;

use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardInput, NativeKeyCode},
    },
    prelude::*,
    window::PrimaryWindow,
};
use bevy_replicon::prelude::ClientState;
use castle_shared::HeroClass;

use crate::dialogue::DialogueState;
use crate::i18n::Tr;
use crate::pause::PauseMenu;
use crate::title::{MenuMode, TitleActive};
use crate::ui::ui_font;

/// Vrai si les commandes tactiles sont affichées (et si les textes d'aide parlent de boutons plutôt que de touches).
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        cfg!(any(target_os = "ios", target_os = "android")) || std::env::var_os("CASTLE_TOUCH").is_some()
    })
}

/// Direction demandée par le joystick tactile ou le stick gauche de la manette (zéro au repos). `send_input` la prend
/// quand aucune touche de direction n'est enfoncée.
#[derive(Resource, Default)]
pub struct Stick(pub Vec2);

/// Rayon du joystick, rayon de sa manette, et part du rayon en dessous de laquelle on reste immobile.
const STICK_RADIUS: f32 = 64.0;
const KNOB_RADIUS: f32 = 26.0;
const STICK_DEAD_ZONE: f32 = 0.25;
/// Zone morte du stick de la manette, et seuil au-delà duquel il sert de flèche dans les menus.
const PAD_DEAD_ZONE: f32 = 0.2;
const PAD_MENU_THRESHOLD: f32 = 0.6;
/// Marge ajoutée autour des boutons pour les toucher sans viser au pixel près.
const HIT_SLOP: f32 = 8.0;
/// Le joystick naît là où le pouce se pose, dans cette part gauche de l'écran.
const STICK_AREA: f32 = 0.45;

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stick>()
            .init_resource::<Controls>()
            // Juste après la lecture du clavier: les touches simulées sont vues par tous les systèmes de la frame.
            .add_systems(PreUpdate, read_controls.after(InputSystems));
        if enabled() {
            app.add_systems(Update, draw_controls);
        }
    }
}

/// Ce que font les boutons: le moment de la partie décide lesquels sont affichés.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Context {
    /// Connexion en cours, rien à toucher.
    #[default]
    Nothing,
    Title {
        address: bool,
        saves: bool,
    },
    Pause,
    Game {
        dialogue: bool,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    /// Touche maintenue tant que le doigt reste sur le bouton.
    Hold(KeyCode),
    /// Caractère tapé une fois (pavé d'adresse).
    Type(char),
    Backspace,
}

#[derive(Clone, Copy, PartialEq)]
enum Label {
    Key(&'static str),
    Text(&'static str),
}

#[derive(Clone, Copy, PartialEq)]
struct Button {
    action: Action,
    label: Label,
    /// Centre et demi-taille, en points de la fenêtre (origine en haut à gauche).
    center: Vec2,
    half: Vec2,
    round: bool,
    color: Color,
}

impl Button {
    fn round(action: Action, label: Label, center: Vec2, radius: f32, color: Color) -> Self {
        Self { action, label, center, half: Vec2::splat(radius), round: true, color }
    }

    fn rect(action: Action, label: Label, center: Vec2, half: Vec2, color: Color) -> Self {
        Self { action, label, center, half, round: false, color }
    }

    fn hit(&self, point: Vec2) -> bool {
        let d = point - self.center;
        if self.round {
            d.length() <= self.half.x + HIT_SLOP
        } else {
            d.x.abs() <= self.half.x + HIT_SLOP && d.y.abs() <= self.half.y + HIT_SLOP
        }
    }
}

struct StickTouch {
    id: u64,
    origin: Vec2,
    offset: Vec2,
}

#[derive(Resource, Default)]
struct Controls {
    context: Context,
    size: Vec2,
    buttons: Vec<Button>,
    /// Doigt posé → bouton qu'il tient.
    held_buttons: HashMap<u64, usize>,
    stick: Option<StickTouch>,
    /// Touches que nous tenons enfoncées en ce moment (à relâcher quand plus rien ne les demande).
    pressed: Vec<KeyCode>,
    /// La disposition a changé: l'affichage doit être reconstruit.
    rebuild: bool,
}

fn context(
    title: &TitleActive,
    mode: &MenuMode,
    state: &ClientState,
    pause: &PauseMenu,
    dialogue: &DialogueState,
) -> Context {
    if title.0 {
        Context::Title { address: mode.address, saves: mode.saves }
    } else if *state != ClientState::Connected {
        Context::Nothing
    } else if pause.open {
        Context::Pause
    } else {
        Context::Game { dialogue: dialogue.showing() }
    }
}

/// Position des boutons pour ce moment et cette taille de fenêtre.
fn layout(context: Context, size: Vec2) -> Vec<Button> {
    let (w, h) = (size.x, size.y);
    // En paysage, l'encoche et les bords arrondis mangent les côtés.
    let side = if w > h { 48.0 } else { 16.0 };
    let bottom = 18.0;
    let top = 10.0;
    let white = Color::WHITE;
    // Bouton principal, en bas à droite sous le pouce.
    let main = Vec2::new(w - side - 58.0, h - bottom - 58.0);
    let up = Vec2::new(side + 44.0, h - bottom - 124.0);
    let down = Vec2::new(side + 44.0, h - bottom - 44.0);
    let hold = Action::Hold;
    let mut out = Vec::new();

    let menu_arrows = |out: &mut Vec<Button>| {
        out.push(Button::round(hold(KeyCode::ArrowUp), Label::Text("^"), up, 34.0, white));
        out.push(Button::round(hold(KeyCode::ArrowDown), Label::Text("v"), down, 34.0, white));
    };
    let ok_back = |out: &mut Vec<Button>| {
        out.push(Button::round(hold(KeyCode::Enter), Label::Key("touch-ok"), main, 42.0, Color::srgb(0.9, 0.75, 0.2)));
        out.push(Button::round(hold(KeyCode::Escape), Label::Key("touch-back"), main + Vec2::new(-100.0, 12.0), 32.0, white));
    };

    match context {
        Context::Nothing => {}
        Context::Title { address, saves } => {
            ok_back(&mut out);
            out.push(Button::rect(
                hold(KeyCode::F1),
                Label::Key("touch-language"),
                Vec2::new(w - side - 56.0, top + 18.0),
                Vec2::new(56.0, 18.0),
                white,
            ));
            if saves {
                out.push(Button::round(
                    hold(KeyCode::Delete),
                    Label::Key("touch-delete"),
                    main + Vec2::new(-28.0, -100.0),
                    32.0,
                    Color::srgb(0.9, 0.3, 0.25),
                ));
            }
            if address {
                // Pavé pour l'adresse de l'hôte, à la place des flèches (inutiles sur cet écran).
                const KEYS: [&str; 13] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", ".", ":", "<-"];
                let (key, gap) = (40.0, 6.0);
                let origin = Vec2::new(side + key / 2.0, h - bottom - 3.0 * (key + gap) + key / 2.0);
                for (i, text) in KEYS.into_iter().enumerate() {
                    let center = origin + Vec2::new((i % 5) as f32, (i / 5) as f32) * (key + gap);
                    let action = match text {
                        "<-" => Action::Backspace,
                        _ => Action::Type(text.chars().next().unwrap_or('0')),
                    };
                    out.push(Button::rect(action, Label::Text(text), center, Vec2::splat(key / 2.0), white));
                }
            } else {
                menu_arrows(&mut out);
            }
        }
        Context::Pause => {
            menu_arrows(&mut out);
            ok_back(&mut out);
        }
        Context::Game { dialogue } => {
            out.push(Button::round(hold(KeyCode::Space), Label::Key("touch-attack"), main, 44.0, Color::srgb(0.85, 0.25, 0.2)));
            out.push(Button::round(
                hold(KeyCode::KeyE),
                Label::Key("touch-special"),
                main + Vec2::new(-100.0, 14.0),
                34.0,
                Color::srgb(0.65, 0.35, 0.85),
            ));
            out.push(Button::round(
                hold(KeyCode::KeyF),
                Label::Key("touch-use"),
                main + Vec2::new(-22.0, -100.0),
                34.0,
                Color::srgb(0.3, 0.7, 0.35),
            ));
            if dialogue {
                // Entrée: continuer (maintenir: tout passer).
                out.push(Button::round(
                    hold(KeyCode::Enter),
                    Label::Key("touch-ok"),
                    main + Vec2::new(-112.0, -96.0),
                    30.0,
                    Color::srgb(0.9, 0.75, 0.2),
                ));
            }
            let pause = Vec2::new(w - side - 34.0, top + 18.0);
            out.push(Button::rect(hold(KeyCode::Escape), Label::Key("touch-pause"), pause, Vec2::new(34.0, 18.0), white));
            // Héros (ou votes pendant un dialogue à choix): 1 à 4, de gauche à droite sous le bouton pause.
            let heroes = [
                (KeyCode::Digit1, "1", HeroClass::Knight),
                (KeyCode::Digit2, "2", HeroClass::Sorcerer),
                (KeyCode::Digit3, "3", HeroClass::Ranger),
                (KeyCode::Digit4, "4", HeroClass::Forgebraise),
            ];
            for (i, (key, text, class)) in heroes.into_iter().enumerate() {
                let center = Vec2::new(w - side - 21.0 - (3 - i) as f32 * 50.0, top + 69.0);
                out.push(Button::rect(hold(key), Label::Text(text), center, Vec2::splat(21.0), crate::class_color(class)));
            }
        }
    }
    out
}

/// Où dessiner le joystick au repos.
fn stick_home(size: Vec2) -> Vec2 {
    let side = if size.x > size.y { 48.0 } else { 16.0 };
    Vec2::new(side + 84.0, size.y - 18.0 - 84.0)
}

/// Lit les doigts posés et la manette, et en déduit les touches tenues et la direction.
#[allow(clippy::too_many_arguments)]
fn read_controls(
    mut controls: ResMut<Controls>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut typed: MessageWriter<KeyboardInput>,
    mut stick: ResMut<Stick>,
    touches: Res<Touches>,
    gamepads: Query<&Gamepad>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    title: Res<TitleActive>,
    mode: Res<MenuMode>,
    state: Res<State<ClientState>>,
    pause: Res<PauseMenu>,
    dialogue: Res<DialogueState>,
) {
    let Ok((window_entity, window)) = windows.single() else {
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    let context = context(&title, &mode, state.get(), &pause, &dialogue);
    let controls = &mut *controls;

    if enabled() && (context != controls.context || size != controls.size) {
        controls.context = context;
        controls.size = size;
        controls.buttons = layout(context, size);
        // Les doigts déjà posés ne tiennent plus rien: les boutons ont changé sous eux.
        controls.held_buttons.clear();
        if !matches!(context, Context::Game { .. }) {
            controls.stick = None;
        }
        controls.rebuild = true;
    }
    controls.context = context;

    // Nouveaux doigts: joystick à gauche (hors boutons), sinon le bouton touché.
    if enabled() {
        for touch in touches.iter_just_pressed() {
            let point = touch.position();
            let button = controls.buttons.iter().position(|b| b.hit(point));
            match button {
                Some(index) => match controls.buttons[index].action {
                    Action::Hold(_) => {
                        controls.held_buttons.insert(touch.id(), index);
                    }
                    Action::Type(c) => type_key(&mut typed, window_entity, Key::Character(c.to_string().into())),
                    Action::Backspace => type_key(&mut typed, window_entity, Key::Backspace),
                },
                None => {
                    let in_game = matches!(context, Context::Game { .. });
                    if in_game && controls.stick.is_none() && point.x < size.x * STICK_AREA {
                        controls.stick = Some(StickTouch { id: touch.id(), origin: point, offset: Vec2::ZERO });
                    }
                }
            }
        }
        if let Some(active) = &mut controls.stick {
            if let Some(touch) = touches.get_pressed(active.id) {
                active.offset = (touch.position() - active.origin).clamp_length_max(STICK_RADIUS);
            }
        }
    }

    // Touches demandées: boutons tenus, puis manette.
    let mut wanted: Vec<KeyCode> = controls
        .held_buttons
        .values()
        .filter_map(|&i| match controls.buttons.get(i)?.action {
            Action::Hold(key) => Some(key),
            _ => None,
        })
        .collect();
    let mut pad_stick = Vec2::ZERO;
    for gamepad in &gamepads {
        let left = gamepad.left_stick();
        if left.length() > pad_stick.length() {
            pad_stick = left;
        }
        gamepad_keys(gamepad, context, left, &mut wanted);
    }
    wanted.sort_unstable();
    wanted.dedup();

    for key in &wanted {
        if !controls.pressed.contains(key) {
            keys.press(*key);
        }
    }
    for key in &controls.pressed {
        if !wanted.contains(key) {
            keys.release(*key);
        }
    }
    controls.pressed = wanted;

    // Direction: le joystick tactile d'abord, sinon le stick de la manette.
    let touch_dir = controls
        .stick
        .as_ref()
        .map(|s| s.offset / STICK_RADIUS)
        .filter(|d| d.length() > STICK_DEAD_ZONE);
    stick.0 = match touch_dir {
        Some(dir) => dir,
        None if pad_stick.length() > PAD_DEAD_ZONE && matches!(context, Context::Game { .. }) => pad_stick,
        None => Vec2::ZERO,
    };

    // Doigts levés: ils lâchent leur bouton ou le joystick (à la frame suivante, pour qu'une tape brève compte).
    for touch in touches.iter_just_released().chain(touches.iter_just_canceled()) {
        controls.held_buttons.remove(&touch.id());
        if controls.stick.as_ref().is_some_and(|s| s.id == touch.id()) {
            controls.stick = None;
        }
    }
}

/// Une frappe complète (appui puis relâche) d'une touche de texte, comme le clavier la donnerait.
fn type_key(typed: &mut MessageWriter<KeyboardInput>, window: Entity, logical_key: Key) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        typed.write(KeyboardInput {
            // Pas de code physique: seul le texte compte, et aucune touche du jeu ne réagit.
            key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
            logical_key: logical_key.clone(),
            state,
            text: None,
            repeat: false,
            window,
        });
    }
}

/// Boutons de la manette → touches du jeu, selon le moment.
fn gamepad_keys(gamepad: &Gamepad, context: Context, left: Vec2, wanted: &mut Vec<KeyCode>) {
    use GamepadButton as B;
    let mut map = |button: B, key: KeyCode| {
        if gamepad.pressed(button) {
            wanted.push(key);
        }
    };
    match context {
        Context::Nothing => {}
        Context::Title { .. } | Context::Pause => {
            map(B::DPadUp, KeyCode::ArrowUp);
            map(B::DPadDown, KeyCode::ArrowDown);
            map(B::South, KeyCode::Enter);
            map(B::East, KeyCode::Escape);
            map(B::North, KeyCode::Delete);
            map(B::Select, KeyCode::F1);
            map(B::Start, KeyCode::Escape);
            // Le stick sert aussi de flèches.
            if left.y > PAD_MENU_THRESHOLD {
                wanted.push(KeyCode::ArrowUp);
            } else if left.y < -PAD_MENU_THRESHOLD {
                wanted.push(KeyCode::ArrowDown);
            }
        }
        Context::Game { dialogue } => {
            map(B::South, KeyCode::Space);
            map(B::West, KeyCode::KeyE);
            map(B::North, KeyCode::KeyF);
            map(B::East, KeyCode::Enter);
            if dialogue {
                map(B::South, KeyCode::Enter);
            }
            map(B::Start, KeyCode::Escape);
            map(B::DPadUp, KeyCode::Digit1);
            map(B::DPadRight, KeyCode::Digit2);
            map(B::DPadDown, KeyCode::Digit3);
            map(B::DPadLeft, KeyCode::Digit4);
        }
    }
}

#[derive(Component)]
struct ControlsRoot;

#[derive(Component)]
struct ButtonNode(usize);

#[derive(Component)]
struct StickBase;

#[derive(Component)]
struct StickKnob;

/// Fond d'un bouton: plus opaque quand il est tenu.
fn button_fill(color: Color, held: bool) -> Color {
    color.with_alpha(if held { 0.6 } else { 0.25 })
}

fn circle(center: Vec2, radius: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(center.x - radius),
        top: Val::Px(center.y - radius),
        width: Val::Px(radius * 2.0),
        height: Val::Px(radius * 2.0),
        border: UiRect::all(Val::Px(2.0)),
        border_radius: BorderRadius::all(Val::Px(radius)),
        ..default()
    }
}

fn draw_controls(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut controls: ResMut<Controls>,
    roots: Query<Entity, With<ControlsRoot>>,
    mut buttons: Query<(&ButtonNode, &mut BackgroundColor), Without<StickBase>>,
    mut base: Query<&mut Node, (With<StickBase>, Without<StickKnob>)>,
    mut knob: Query<&mut Node, (With<StickKnob>, Without<StickBase>)>,
) {
    if std::mem::take(&mut controls.rebuild) {
        for root in &roots {
            commands.entity(root).despawn();
        }
        let in_game = matches!(controls.context, Context::Game { .. });
        let home = stick_home(controls.size);
        commands
            .spawn((
                ControlsRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                // Au-dessus du jeu et du menu de pause.
                GlobalZIndex(90),
            ))
            .with_children(|ui| {
                if in_game {
                    ui.spawn((
                        StickBase,
                        circle(home, STICK_RADIUS),
                        BackgroundColor(Color::WHITE.with_alpha(0.08)),
                        BorderColor::all(Color::WHITE.with_alpha(0.35)),
                    ));
                    ui.spawn((
                        StickKnob,
                        circle(home, KNOB_RADIUS),
                        BackgroundColor(Color::WHITE.with_alpha(0.3)),
                        BorderColor::all(Color::WHITE.with_alpha(0.6)),
                    ));
                }
                for (index, button) in controls.buttons.iter().enumerate() {
                    let node = Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(button.center.x - button.half.x),
                        top: Val::Px(button.center.y - button.half.y),
                        width: Val::Px(button.half.x * 2.0),
                        height: Val::Px(button.half.y * 2.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(if button.round { button.half.x } else { 6.0 })),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    };
                    let font = ui_font(&assets, if button.half.x < 36.0 { 7.0 } else { 8.0 });
                    ui.spawn((
                        ButtonNode(index),
                        node,
                        BackgroundColor(button_fill(button.color, false)),
                        BorderColor::all(Color::WHITE.with_alpha(0.55)),
                    ))
                    .with_children(|b| match button.label {
                        Label::Key(key) => {
                            b.spawn((Text::default(), Tr::new(key), font, TextColor(Color::WHITE)));
                        }
                        Label::Text(text) => {
                            b.spawn((Text::new(text), font, TextColor(Color::WHITE)));
                        }
                    });
                }
            });
        return;
    }

    // Boutons tenus en surbrillance.
    for (node, mut fill) in &mut buttons {
        let Some(button) = controls.buttons.get(node.0) else {
            continue;
        };
        let held = controls.held_buttons.values().any(|&i| i == node.0);
        let wanted = button_fill(button.color, held);
        if fill.0 != wanted {
            fill.0 = wanted;
        }
    }

    // Joystick: il suit le pouce, et revient à sa place au repos.
    let (center, offset) = match &controls.stick {
        Some(s) => (s.origin, s.offset),
        None => (stick_home(controls.size), Vec2::ZERO),
    };
    let place = |node: &mut Node, at: Vec2, radius: f32| {
        let (left, top) = (Val::Px(at.x - radius), Val::Px(at.y - radius));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    };
    for mut node in &mut base {
        place(&mut node, center, STICK_RADIUS);
    }
    for mut node in &mut knob {
        place(&mut node, center + offset, KNOB_RADIUS);
    }
}
