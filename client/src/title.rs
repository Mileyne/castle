//! Écran titre et menu des parties: solo, coopératif (héberger ou rejoindre), parties nommées.
//!
//! L'illustration couvre la fenêtre jusqu'à la connexion au serveur. Le menu choisit la partie, lance le serveur
//! local si besoin (`host.rs`) et ouvre la connexion; le jeu démarre quand le client est connecté.

use std::net::{IpAddr, SocketAddr};

use bevy::{
    color::palettes::css,
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{RenetClient, netcode::NetcodeClientTransport};
use castle_shared::DEFAULT_PORT;
use castle_shared::save::{self, GameMode, SAVE_NAME_MAX, SaveGame};

use crate::host::{self, ConnectAt, LocalServer};
use crate::i18n::Tr;
use crate::ui::ui_font;

/// Vrai tant que l'écran titre est affiché: le jeu ignore alors les touches (dialogue d'intro compris).
#[derive(Resource)]
pub struct TitleActive(pub bool);

/// Adresse à donner aux autres joueurs, quand on héberge une partie.
#[derive(Resource)]
pub struct HostAddress(pub String);

/// Retour à l'écran titre: coupe la connexion, arrête le serveur local et efface la partie affichée. Le message,
/// s'il y en a un, est la clé d'une erreur à montrer (connexion perdue...).
#[derive(Event)]
pub struct ReturnToTitle(pub Option<&'static str>);

/// Secondes accordées à une connexion avant d'y renoncer.
const CONNECT_TIMEOUT: f32 = 8.0;

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TitleActive(true))
            .init_resource::<Menu>()
            .add_systems(Startup, spawn_title)
            .add_observer(return_to_title)
            .add_systems(
                Update,
                (
                    (menu_input, rebuild_menu).chain().run_if(|t: Res<TitleActive>| t.0),
                    watch_connection,
                    show_host_address,
                ),
            );
    }
}

#[derive(Component)]
struct TitleScreen;

/// Conteneur reconstruit à chaque changement du menu.
#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct HostBadge;

/// Texte d'une entrée de menu qui prend la couleur de sélection.
#[derive(Component)]
struct RowText(usize);

/// Marqueur « > » devant une entrée.
#[derive(Component)]
struct RowMarker(usize);

/// Ligne de saisie (nom de partie ou adresse).
#[derive(Component)]
struct InputLine;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Screen {
    Main,
    /// Liste des parties d'un mode, avec « nouvelle partie » en tête.
    Saves(GameMode),
    NewName(GameMode),
    JoinAddress,
    Connecting,
}

#[derive(Resource)]
struct Menu {
    screen: Screen,
    cursor: usize,
    /// Texte en cours de saisie (nom de partie ou adresse).
    input: String,
    saves: Vec<SaveGame>,
    confirm_delete: bool,
    /// Clé du message d'erreur affiché.
    error: Option<&'static str>,
    /// Le menu doit être entièrement redessiné (changement d'écran ou de contenu).
    dirty: bool,
    /// Seule la sélection a bougé: on recolore sans rien reconstruire (sinon le menu clignote).
    cursor_dirty: bool,
    /// Seul le texte saisi a changé.
    input_dirty: bool,
    /// Depuis combien de temps on attend la connexion.
    waiting: f32,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            screen: Screen::Main,
            cursor: 0,
            input: String::new(),
            saves: Vec::new(),
            confirm_delete: false,
            error: None,
            dirty: true,
            cursor_dirty: false,
            input_dirty: false,
            waiting: 0.0,
        }
    }
}

impl Menu {
    fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.cursor = 0;
        self.input.clear();
        self.confirm_delete = false;
        self.dirty = true;
        if let Screen::Saves(mode) = screen {
            self.saves = save::list(mode);
        }
    }

    fn item_count(&self) -> usize {
        match self.screen {
            Screen::Main => 4,
            Screen::Saves(_) => 1 + self.saves.len(),
            _ => 0,
        }
    }
}

fn spawn_title(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            TitleScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(100),
        ))
        .with_children(|screen| {
            // L'illustration garde ses proportions (1376 x 768) et se centre.
            screen
                .spawn((
                    ImageNode::new(assets.load("title.png")),
                    Node {
                        height: Val::Percent(100.0),
                        max_width: Val::Percent(100.0),
                        aspect_ratio: Some(1376.0 / 768.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::FlexEnd,
                        padding: UiRect::bottom(Val::Px(18.0)),
                        ..default()
                    },
                ))
                .with_children(|image| {
                    image.spawn((
                        MenuRoot,
                        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(10.0), ..default() },
                    ));
                });
        });
}

/// Police, couleur et ombre d'un texte de menu; le texte lui-même s'ajoute à côté (`Text::new` ou `Tr`).
fn menu_style(assets: &AssetServer, size: f32, color: Color) -> impl Bundle {
    (
        ui_font(assets, size),
        TextColor(color),
        TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK },
    )
}

/// Texte de menu dont le contenu vient d'une clé de traduction (`Tr`).
fn menu_text(assets: &AssetServer, size: f32, color: Color) -> impl Bundle {
    (Text::default(), menu_style(assets, size, color))
}

/// Une entrée du menu: un marqueur de sélection, puis son contenu. Les textes à recolorer portent `RowText(index)`.
fn spawn_row(
    menu: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    index: usize,
    selected: bool,
    content: impl FnOnce(&mut ChildSpawnerCommands, Color, RowText),
) {
    let color = row_color(selected);
    menu.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .with_children(|row| {
            row.spawn((
                menu_style(assets, 14.0, color),
                Text::new(if selected { ">" } else { " " }),
                RowMarker(index),
                RowText(index),
            ));
            content(row, color, RowText(index));
        });
}

fn row_color(selected: bool) -> Color {
    if selected { css::GOLD.into() } else { css::WHITE.into() }
}

fn rebuild_menu(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut menu: ResMut<Menu>,
    host: Option<Res<HostAddress>>,
    roots: Query<Entity, With<MenuRoot>>,
    mut markers: Query<(&RowMarker, &mut Text), Without<InputLine>>,
    mut colors: Query<(&RowText, &mut TextColor)>,
    mut input_lines: Query<&mut Text, With<InputLine>>,
) {
    if !menu.dirty {
        // Mises à jour légères: les textes existent déjà, on ne touche qu'à ce qui a changé.
        if menu.cursor_dirty {
            menu.cursor_dirty = false;
            for (marker, mut text) in &mut markers {
                text.0 = if marker.0 == menu.cursor { ">" } else { " " }.into();
            }
            for (row, mut color) in &mut colors {
                color.0 = row_color(row.0 == menu.cursor);
            }
        }
        if menu.input_dirty {
            menu.input_dirty = false;
            for mut text in &mut input_lines {
                text.0 = format!("{}_", menu.input);
            }
        }
        return;
    }
    menu.cursor_dirty = false;
    menu.input_dirty = false;
    menu.dirty = false;
    let Ok(root) = roots.single() else {
        return;
    };
    commands.entity(root).despawn_children();
    let menu = &*menu;
    commands.entity(root).with_children(|ui| {
        let line = |ui: &mut ChildSpawnerCommands, key: &'static str, size: f32, color: Color| {
            ui.spawn((menu_text(&assets, size, color), Tr::new(key)));
        };
        match menu.screen {
            Screen::Main => {
                for (i, key) in ["menu-solo", "menu-host", "menu-join", "menu-quit"].into_iter().enumerate() {
                    spawn_row(ui, &assets, i, i == menu.cursor, |row, color, tag| {
                        row.spawn((menu_text(&assets, 16.0, color), Tr::new(key), tag));
                    });
                }
                line(ui, "menu-hint", 8.0, css::LIGHT_GRAY.into());
            }
            Screen::Saves(mode) => {
                let title = if mode == GameMode::Solo { "menu-solo" } else { "menu-host" };
                line(ui, title, 16.0, css::LIGHT_SKY_BLUE.into());
                spawn_row(ui, &assets, 0, menu.cursor == 0, |row, color, tag| {
                    row.spawn((menu_text(&assets, 14.0, color), Tr::new("menu-new-game"), tag));
                });
                for (i, save) in menu.saves.iter().enumerate() {
                    spawn_row(ui, &assets, i + 1, menu.cursor == i + 1, |row, color, tag| {
                        row.spawn((menu_style(&assets, 14.0, color), Text::new(save.name.clone()), tag));
                        row.spawn((menu_text(&assets, 10.0, css::LIGHT_GRAY.into()), Tr::with_prefix("level-word", "- ")));
                        row.spawn((menu_style(&assets, 10.0, css::LIGHT_GRAY.into()), Text::new(save.level.to_string())));
                    });
                }
                if menu.saves.is_empty() {
                    line(ui, "menu-no-saves", 10.0, css::LIGHT_GRAY.into());
                }
                let hint = if menu.confirm_delete { "menu-confirm-delete" } else { "menu-saves-hint" };
                line(ui, hint, 8.0, if menu.confirm_delete { css::TOMATO.into() } else { css::LIGHT_GRAY.into() });
            }
            Screen::NewName(_) | Screen::JoinAddress => {
                let prompt = if menu.screen == Screen::JoinAddress { "menu-enter-address" } else { "menu-enter-name" };
                line(ui, prompt, 14.0, css::LIGHT_SKY_BLUE.into());
                ui.spawn((menu_style(&assets, 16.0, css::GOLD.into()), Text::new(format!("{}_", menu.input)), InputLine));
                line(ui, "menu-input-hint", 8.0, css::LIGHT_GRAY.into());
            }
            Screen::Connecting => {
                line(ui, "menu-connecting", 16.0, css::WHITE.into());
                if let Some(host) = &host {
                    ui.spawn((menu_text(&assets, 10.0, css::LIGHT_GRAY.into()), Tr::with_prefix("menu-host-address", "")));
                    ui.spawn((menu_style(&assets, 14.0, css::GOLD.into()), Text::new(host.0.clone())));
                }
            }
        }
        if let Some(error) = menu.error {
            line(ui, error, 10.0, css::TOMATO.into());
        }
    });
}

/// Lit une adresse saisie: `192.168.1.20` ou `192.168.1.20:5000`.
fn parse_address(text: &str) -> Option<SocketAddr> {
    let text = text.trim();
    text.parse::<SocketAddr>()
        .ok()
        .or_else(|| text.parse::<IpAddr>().ok().map(|ip| SocketAddr::new(ip, DEFAULT_PORT)))
}

fn menu_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<KeyboardInput>,
    state: Res<State<ClientState>>,
    mut menu: ResMut<Menu>,
    mut exit: MessageWriter<AppExit>,
) {
    // Une connexion est en cours (ou déjà établie): le menu ne répond plus.
    if menu.screen == Screen::Connecting || *state.get() != ClientState::Disconnected {
        typed.clear();
        return;
    }
    let up = keys.just_pressed(KeyCode::ArrowUp);
    let down = keys.just_pressed(KeyCode::ArrowDown);
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let escape = keys.just_pressed(KeyCode::Escape);
    let delete = keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace);

    // Saisie de texte.
    if matches!(menu.screen, Screen::NewName(_) | Screen::JoinAddress) {
        let join = menu.screen == Screen::JoinAddress;
        for event in typed.read() {
            if !event.state.is_pressed() {
                continue;
            }
            let text = match &event.logical_key {
                Key::Character(s) => s.as_str(),
                Key::Space => " ",
                Key::Backspace => {
                    menu.input.pop();
                    menu.input_dirty = true;
                    continue;
                }
                _ => continue,
            };
            for c in text.chars().filter(|c| !c.is_control()) {
                let allowed = if join { c.is_ascii_digit() || c == '.' || c == ':' } else { true };
                let limit = if join { 21 } else { SAVE_NAME_MAX };
                if allowed && menu.input.chars().count() < limit {
                    menu.input.push(c);
                    // Effacer un message d'erreur change la disposition; sinon seule la saisie est mise à jour.
                    if menu.error.take().is_some() {
                        menu.dirty = true;
                    } else {
                        menu.input_dirty = true;
                    }
                }
            }
        }
    } else {
        typed.clear();
    }

    if escape {
        match menu.screen {
            Screen::Main => {}
            Screen::NewName(mode) => menu.go(Screen::Saves(mode)),
            _ => menu.go(Screen::Main),
        }
        menu.error = None;
        return;
    }
    let count = menu.item_count();
    if count > 0 && (up || down) {
        menu.cursor = if up { (menu.cursor + count - 1) % count } else { (menu.cursor + 1) % count };
        menu.cursor_dirty = true;
        if std::mem::take(&mut menu.confirm_delete) {
            // Le message de confirmation disparaît: la disposition change.
            menu.dirty = true;
        }
    }

    match menu.screen {
        Screen::Main if enter => match menu.cursor {
            0 => menu.go(Screen::Saves(GameMode::Solo)),
            1 => menu.go(Screen::Saves(GameMode::Coop)),
            2 => menu.go(Screen::JoinAddress),
            _ => {
                exit.write(AppExit::Success);
            }
        },
        Screen::Saves(mode) => {
            if delete && menu.cursor > 0 {
                if menu.confirm_delete {
                    let name = menu.saves[menu.cursor - 1].name.clone();
                    save::delete(&name);
                    menu.go(Screen::Saves(mode));
                } else {
                    menu.confirm_delete = true;
                    menu.dirty = true;
                }
            } else if enter {
                if menu.cursor == 0 {
                    menu.go(Screen::NewName(mode));
                } else {
                    let name = menu.saves[menu.cursor - 1].name.clone();
                    launch(&mut commands, &mut menu, mode, &name);
                }
            }
        }
        Screen::NewName(mode) if enter => {
            let name = save::clean_name(&menu.input);
            match save::load(&name) {
                _ if name.is_empty() => {}
                // Même nom, même mode: c'est la partie existante qui est reprise.
                Some(existing) if existing.mode != mode => {
                    menu.error = Some("menu-name-taken");
                    menu.dirty = true;
                }
                _ => launch(&mut commands, &mut menu, mode, &name),
            }
        }
        Screen::JoinAddress if enter => match parse_address(&menu.input) {
            Some(addr) => {
                commands.insert_resource(ConnectAt::now(addr));
                menu.error = None;
                menu.go(Screen::Connecting);
            }
            None => {
                menu.error = Some("menu-bad-address");
                menu.dirty = true;
            }
        },
        _ => {}
    }
}

/// Démarre le serveur de la partie, puis se connecte dessus.
fn launch(commands: &mut Commands, menu: &mut Menu, mode: GameMode, name: &str) {
    match host::start_local_server(mode, name) {
        Ok(server) => {
            commands.insert_resource(server);
            let local = (std::net::Ipv4Addr::LOCALHOST, DEFAULT_PORT).into();
            commands.insert_resource(ConnectAt::after_server_start(local));
            if mode == GameMode::Coop {
                if let Some(addr) = host::lan_address() {
                    commands.insert_resource(HostAddress(addr.to_string()));
                }
            }
            menu.error = None;
            menu.go(Screen::Connecting);
        }
        Err(key) => {
            menu.error = Some(key);
            menu.dirty = true;
        }
    }
}

/// Ferme l'écran titre une fois connecté; ramène au menu si la connexion échoue ou se perd.
fn watch_connection(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<ClientState>>,
    pending: Option<Res<ConnectAt>>,
    mut menu: ResMut<Menu>,
    mut title: ResMut<TitleActive>,
    screens: Query<Entity, With<TitleScreen>>,
) {
    let connected = *state.get() == ClientState::Connected;
    if connected && title.0 {
        title.0 = false;
        for screen in &screens {
            commands.entity(screen).despawn();
        }
        return;
    }
    if !title.0 && !connected {
        // Le serveur a disparu en cours de partie (l'écran titre n'est fermé qu'une fois connecté).
        commands.trigger(ReturnToTitle(Some("menu-connection-lost")));
    } else if title.0 && menu.screen == Screen::Connecting && pending.is_none() {
        menu.waiting += time.delta_secs();
        if menu.waiting > CONNECT_TIMEOUT {
            commands.trigger(ReturnToTitle(Some("menu-connect-failed")));
        }
    }
}

fn return_to_title(
    request: On<ReturnToTitle>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut menu: ResMut<Menu>,
    mut title: ResMut<TitleActive>,
    screens: Query<(), With<TitleScreen>>,
    remote: Query<Entity, With<Remote>>,
    badges: Query<Entity, With<HostBadge>>,
) {
    // On abandonne la connexion et le serveur local; le serveur d'un hôte s'arrête avec lui.
    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();
    commands.remove_resource::<LocalServer>();
    commands.remove_resource::<HostAddress>();
    commands.remove_resource::<ConnectAt>();
    // Replicon laisse en place les entités reçues du serveur: on efface la partie affichée.
    for entity in remote.iter().chain(badges.iter()) {
        commands.entity(entity).despawn();
    }
    menu.waiting = 0.0;
    menu.go(Screen::Main);
    menu.error = request.0;
    title.0 = true;
    if screens.is_empty() {
        spawn_title(commands, assets);
    }
}

/// En coopératif, l'hôte garde sous les yeux l'adresse à communiquer aux autres joueurs.
fn show_host_address(
    mut commands: Commands,
    assets: Res<AssetServer>,
    title: Res<TitleActive>,
    host: Option<Res<HostAddress>>,
    badge: Query<(), With<HostBadge>>,
) {
    let Some(host) = host else {
        return;
    };
    if title.0 || !badge.is_empty() {
        return;
    }
    commands
        .spawn((
            HostBadge,
            Node { position_type: PositionType::Absolute, right: Val::Px(10.0), bottom: Val::Px(8.0), column_gap: Val::Px(8.0), ..default() },
            GlobalZIndex(50),
        ))
        .with_children(|badge| {
            badge.spawn((menu_text(&assets, 8.0, css::LIGHT_GRAY.into()), Tr::new("menu-host-address")));
            badge.spawn((menu_style(&assets, 8.0, css::GOLD.into()), Text::new(host.0.clone())));
        });
}
