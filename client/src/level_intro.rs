//! Écran noir qui annonce le niveau dans lequel on entre: au lancement de la partie et à chaque passage de portail.

use bevy::{color::palettes::css, prelude::*};
use castle_shared::Expedition;

use crate::i18n::Tr;
use crate::title::TitleActive;
use crate::ui::ui_font;

/// Secondes d'affichage plein, puis de fondu vers le jeu.
const HOLD: f32 = 2.2;
const FADE: f32 = 0.8;

#[derive(Resource, Default)]
pub struct LevelIntro {
    /// Une nouvelle expédition est arrivée: l'annonce doit être (re)lancée dès que l'écran titre est fermé.
    pending: bool,
    /// Temps écoulé depuis le début de l'annonce, tant qu'elle est affichée.
    elapsed: Option<f32>,
}

/// Condition d'exécution: le joueur peut agir (ni écran titre, ni annonce de niveau).
pub fn player_can_act(title: Res<TitleActive>, intro: Res<LevelIntro>) -> bool {
    !title.0 && intro.elapsed.is_none()
}

pub struct LevelIntroPlugin;

impl Plugin for LevelIntroPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelIntro>()
            .add_systems(Update, (queue_intro, start_intro, run_intro).chain());
    }
}

#[derive(Component)]
struct IntroScreen;

/// Texte de l'annonce: son opacité suit le fondu.
#[derive(Component)]
struct IntroText;

fn queue_intro(added: Query<(), Added<Expedition>>, mut intro: ResMut<LevelIntro>) {
    if !added.is_empty() {
        intro.pending = true;
    }
}

fn start_intro(
    mut commands: Commands,
    assets: Res<AssetServer>,
    title: Res<TitleActive>,
    mut intro: ResMut<LevelIntro>,
    expeditions: Query<&Expedition>,
    old: Query<Entity, With<IntroScreen>>,
) {
    if !intro.pending || title.0 {
        return;
    }
    let Some(expedition) = expeditions.iter().last() else {
        return;
    };
    intro.pending = false;
    intro.elapsed = Some(0.0);
    for screen in &old {
        commands.entity(screen).despawn();
    }
    let white = TextColor(css::WHITE.into());
    commands
        .spawn((
            IntroScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(90),
        ))
        .with_children(|screen| {
            screen.spawn(Node { column_gap: Val::Px(16.0), ..default() }).with_children(|line| {
                line.spawn((Text::default(), Tr::new("level-word"), ui_font(&assets, 36.0), white, IntroText));
                line.spawn((Text::new(expedition.level.to_string()), ui_font(&assets, 36.0), white, IntroText));
            });
            screen.spawn((
                Text::default(),
                Tr::new(expedition.region.name_key()),
                ui_font(&assets, 18.0),
                TextColor(css::SILVER.into()),
                IntroText,
            ));
        });
}

fn run_intro(
    mut commands: Commands,
    time: Res<Time>,
    mut intro: ResMut<LevelIntro>,
    mut screens: Query<(Entity, &mut BackgroundColor), With<IntroScreen>>,
    mut texts: Query<&mut TextColor, With<IntroText>>,
) {
    let Some(elapsed) = intro.elapsed.as_mut() else {
        return;
    };
    *elapsed += time.delta_secs();
    let alpha = (1.0 - (*elapsed - HOLD) / FADE).clamp(0.0, 1.0);
    for (entity, mut background) in &mut screens {
        if *elapsed >= HOLD + FADE {
            commands.entity(entity).despawn();
        } else {
            background.0 = Color::BLACK.with_alpha(alpha);
        }
    }
    for mut color in &mut texts {
        color.0 = color.0.with_alpha(alpha);
    }
    if *elapsed >= HOLD + FADE {
        intro.elapsed = None;
    }
}
