//! Menu de pause (Échap): reprendre, ou abandonner le niveau en cours et revenir au menu principal.
//!
//! La partie est sauvegardée à l'entrée de chaque niveau: revenir au menu perd donc ce qui s'est passé depuis. En solo
//! le serveur s'arrête pendant que le menu est ouvert.

use bevy::{color::palettes::css, prelude::*};
use bevy_replicon::prelude::*;
use castle_shared::SetPause;

use crate::i18n::Tr;
use crate::title::{HostAddress, ReturnToTitle, TitleActive};
use crate::ui::ui_font;

/// Entrées: reprendre, commandes tactiles (oui/non), abandonner.
const ITEMS: [&str; 3] = ["pause-resume", "settings-touch", "pause-quit"];

#[derive(Resource, Default)]
pub struct PauseMenu {
    pub open: bool,
    cursor: usize,
    /// Une entrée vient d'être cliquée ou touchée.
    click: Option<usize>,
    /// Le menu doit être redessiné (le réglage tactile a changé).
    redraw: bool,
}

/// Condition d'exécution: le menu de pause est fermé.
pub fn not_paused(pause: Res<PauseMenu>) -> bool {
    !pause.open
}

pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseMenu>()
            .add_observer(close_on_return)
            .add_systems(Update, (toggle_pause, pointer, navigate, sync_overlay).chain());
    }
}

#[derive(Component)]
struct PauseOverlay;

/// Texte d'une entrée, qui prend la couleur de sélection.
#[derive(Component)]
struct PauseRow(usize);

/// Une entrée du menu, qui se clique ou se touche.
#[derive(Component)]
struct PauseButton(usize);

fn set_open(commands: &mut Commands, pause: &mut PauseMenu, open: bool) {
    pause.open = open;
    pause.cursor = 0;
    commands.client_trigger(SetPause(open));
}

fn toggle_pause(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<ClientState>>,
    title: Res<TitleActive>,
    mut pause: ResMut<PauseMenu>,
) {
    if title.0 || *state.get() != ClientState::Connected || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let open = !pause.open;
    set_open(&mut commands, &mut pause, open);
}

/// Clic ou toucher sur une entrée (le survol déplace la sélection).
fn pointer(mut pause: ResMut<PauseMenu>, buttons: Query<(&Interaction, &PauseButton), Changed<Interaction>>) {
    if !pause.open {
        return;
    }
    for (interaction, button) in &buttons {
        match interaction {
            Interaction::Pressed => pause.click = Some(button.0),
            Interaction::Hovered => pause.cursor = button.0,
            _ => {}
        }
    }
}

fn navigate(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut pause: ResMut<PauseMenu>) {
    if !pause.open {
        return;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        pause.cursor = (pause.cursor + ITEMS.len() - 1) % ITEMS.len();
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        pause.cursor = (pause.cursor + 1) % ITEMS.len();
    }
    let mut enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    if let Some(index) = pause.click.take() {
        pause.cursor = index;
        enter = true;
    }
    if enter {
        match pause.cursor {
            0 => set_open(&mut commands, &mut pause, false),
            1 => {
                crate::settings::set_touch_ui(!crate::settings::touch_ui());
                pause.redraw = true;
            }
            _ => commands.trigger(ReturnToTitle(None)),
        }
    }
}

fn close_on_return(_: On<ReturnToTitle>, mut pause: ResMut<PauseMenu>) {
    *pause = PauseMenu::default();
}

/// Affiche ou retire le menu; la sélection se recolore sur place (pas de reconstruction, donc pas de clignotement).
fn sync_overlay(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut pause: ResMut<PauseMenu>,
    host: Option<Res<HostAddress>>,
    overlay: Query<Entity, With<PauseOverlay>>,
    mut rows: Query<(&PauseRow, &mut TextColor, &mut Text)>,
) {
    if !pause.open || pause.redraw {
        for entity in &overlay {
            commands.entity(entity).despawn();
        }
        if !pause.open {
            return;
        }
        // Le menu se redessine au prochain passage (les entités retirées sont encore là dans ce passage).
        pause.redraw = false;
        return;
    }
    if overlay.is_empty() {
        let font = |size| ui_font(&assets, size);
        commands
            .spawn((
                PauseOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(14.0),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.7)),
                GlobalZIndex(80),
            ))
            .with_children(|ui| {
                ui.spawn((Text::default(), Tr::new("pause-title"), font(24.0), TextColor(css::LIGHT_SKY_BLUE.into())));
                for (i, key) in ITEMS.into_iter().enumerate() {
                    ui.spawn((
                        Button,
                        PauseButton(i),
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(10.0),
                            padding: UiRect::axes(Val::Px(22.0), Val::Px(8.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE.with_alpha(0.08)),
                    ))
                    .with_children(|row| {
                        row.spawn((Text::default(), Tr::new(key), font(16.0), TextColor(css::WHITE.into()), PauseRow(i)));
                        if i == 1 {
                            let value = if crate::settings::touch_ui() { "settings-on" } else { "settings-off" };
                            row.spawn((Text::default(), Tr::new(value), font(16.0), TextColor(css::LIGHT_GRAY.into())));
                        }
                    });
                }
                // Abandonner coûte la progression du niveau; l'hôte coupe aussi les autres joueurs.
                ui.spawn((Text::default(), Tr::new("pause-warning"), font(8.0), TextColor(css::TOMATO.into())));
                if host.is_some() {
                    ui.spawn((Text::default(), Tr::new("pause-host-warning"), font(8.0), TextColor(css::TOMATO.into())));
                }
            });
        return;
    }
    for (row, mut color, _) in &mut rows {
        color.0 = if row.0 == pause.cursor { css::GOLD.into() } else { css::WHITE.into() };
    }
}
