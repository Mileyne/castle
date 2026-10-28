//! Écran titre: l'illustration du jeu couvre la fenêtre jusqu'à ce que le joueur choisisse de reprendre la partie en
//! cours sur le serveur (Entrée ou clic) ou de repartir de zéro (N).

use bevy::{color::palettes::css, prelude::*};
use bevy_replicon::prelude::*;
use castle_shared::NewGame;

use crate::i18n::Tr;
use crate::ui::ui_font;

/// Vrai tant que l'écran titre est affiché: le jeu ignore alors les touches (dialogue d'intro compris).
#[derive(Resource)]
pub struct TitleActive(pub bool);

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TitleActive(true))
            .add_systems(Startup, spawn_title)
            .add_systems(Update, close_title.run_if(|t: Res<TitleActive>| t.0));
    }
}

#[derive(Component)]
struct TitleScreen;

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
                    image
                        .spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(10.0), ..default() })
                        .with_children(|menu| {
                            for key in ["title-continue", "title-new"] {
                                menu.spawn((
                                    Text::default(),
                                    Tr::new(key),
                                    ui_font(&assets, 16.0),
                                    TextColor(css::WHITE.into()),
                                    TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK },
                                ));
                            }
                        });
                });
        });
}

fn close_title(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<State<ClientState>>,
    mut title: ResMut<TitleActive>,
    screens: Query<Entity, With<TitleScreen>>,
) {
    // Les choix ne valent qu'une fois connecté au serveur.
    if *state.get() != ClientState::Connected {
        return;
    }
    let new_game = keys.just_pressed(KeyCode::KeyN);
    let resume = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) || mouse.just_pressed(MouseButton::Left);
    if !new_game && !resume {
        return;
    }
    if new_game {
        commands.client_trigger(NewGame);
    }
    title.0 = false;
    for screen in &screens {
        commands.entity(screen).despawn();
    }
}
