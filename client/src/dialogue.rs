//! Dialogues: Yarn Spinner gère le déroulé des scènes, Fluent fournit les textes dans la langue du joueur.
//!
//! Le serveur déclenche les scènes, fait avancer tout le monde en même temps et compte les votes.
//! Chaque client joue la même scène localement; une réplique `#line:intro_01` se lit dans la clé Fluent `dlg-intro_01`.

use bevy::{color::palettes::css, prelude::*};
use bevy_fluent::prelude::Localization;
use bevy_replicon::prelude::*;
use bevy_yarnspinner::{
    events::{DialogueCompleted, PresentLine, PresentOptions},
    prelude::{DialogueRunner, OptionId, YarnProject, YarnSpinnerPlugin},
};
use castle_shared::*;
use fluent_content::Content;

use crate::i18n::Tr;
use crate::ui::ui_font;

/// Caractères révélés par seconde.
const TYPEWRITER_SPEED: f32 = 45.0;

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(YarnSpinnerPlugin::new())
            .init_resource::<DialogueState>()
            .add_systems(Startup, spawn_panel)
            .add_systems(
                Update,
                (
                    spawn_runner.run_if(resource_added::<YarnProject>),
                    dialogue_input
                        .run_if(in_state(ClientState::Connected))
                        .run_if(crate::level_intro::player_can_act)
                        .run_if(crate::pause::not_paused),
                    update_panel,
                )
                    .chain(),
            )
            .add_observer(on_begin)
            .add_observer(reset_dialogue)
            .add_observer(on_advance)
            .add_observer(on_select)
            .add_observer(on_present_line)
            .add_observer(on_present_options)
            .add_observer(on_completed);
    }
}

struct ShownLine {
    speaker: Option<String>,
    key: String,
    fallback: String,
}

struct ShownOption {
    id: OptionId,
    key: String,
    fallback: String,
}

/// Ce que le joueur voit en ce moment.
#[derive(Resource, Default)]
pub struct DialogueState {
    line: Option<ShownLine>,
    options: Vec<ShownOption>,
    voted: bool,
    /// Nombre de caractères déjà affichés (effet machine à écrire).
    revealed: f32,
}

impl DialogueState {
    /// Un vote est en cours: les touches 1 à 4 servent à voter, pas à changer de classe.
    pub fn is_voting(&self) -> bool {
        !self.options.is_empty()
    }
}

#[derive(Component)]
struct DialoguePanel;

#[derive(Component)]
struct SpeakerText;

/// Vignette du personnage qui parle.
#[derive(Component)]
struct DialoguePortrait;

#[derive(Component)]
struct BodyText;

#[derive(Component)]
struct PromptText;

fn spawn_runner(mut commands: Commands, project: Res<YarnProject>) {
    let runner = project.create_dialogue_runner(&mut commands);
    commands.spawn(runner);
}

/// L'identifiant d'une réplique `line:intro_01` devient la clé Fluent `dlg-intro_01`.
fn fluent_key(id: &str) -> String {
    format!("dlg-{}", id.strip_prefix("line:").unwrap_or(id))
}

fn on_begin(begin: On<DialogueBegin>, mut runners: Query<&mut DialogueRunner>) {
    let Ok(mut runner) = runners.single_mut() else {
        warn!("dialogue '{}' requested before the dialogue runner is ready", begin.0);
        return;
    };
    if runner.is_running() {
        return;
    }
    if let Err(err) = runner.try_start_node(&begin.0) {
        warn!("cannot start dialogue '{}': {err}", begin.0);
    }
}

fn on_advance(_advance: On<DialogueAdvance>, mut runners: Query<&mut DialogueRunner>) {
    if let Ok(mut runner) = runners.single_mut() {
        if runner.is_running() && !runner.is_waiting_for_option_selection() {
            runner.continue_in_next_update();
        }
    }
}

fn on_select(
    select: On<DialogueSelect>,
    mut state: ResMut<DialogueState>,
    mut runners: Query<&mut DialogueRunner>,
) {
    let Ok(mut runner) = runners.single_mut() else {
        return;
    };
    if let Some(option) = state.options.get(select.0 as usize) {
        if let Err(err) = runner.select_option(option.id) {
            warn!("cannot select option: {err}");
        }
    }
    state.options.clear();
    state.voted = false;
}

fn on_present_line(
    line: On<PresentLine>,
    mut state: ResMut<DialogueState>,
    mut runners: Query<&mut DialogueRunner>,
) {
    let id = line.line.id.to_string();
    state.line = Some(ShownLine {
        speaker: line.line.character_name().map(str::to_string),
        key: fluent_key(&id),
        fallback: line.line.text_without_character_name(),
    });
    state.revealed = 0.0;
    // La dernière réplique avant des choix reste affichée avec eux, sans attendre d'avancement.
    if line.line.is_last_line_before_options() {
        if let Ok(mut runner) = runners.single_mut() {
            runner.continue_in_next_update();
        }
    }
}

fn on_present_options(options: On<PresentOptions>, mut state: ResMut<DialogueState>) {
    state.options = options
        .options
        .iter()
        .map(|option| ShownOption {
            id: option.id,
            key: fluent_key(&option.line.id.to_string()),
            fallback: option.line.text_without_character_name(),
        })
        .collect();
    state.voted = false;
}

fn on_completed(_done: On<DialogueCompleted>, mut commands: Commands, mut state: ResMut<DialogueState>) {
    *state = DialogueState::default();
    commands.client_trigger(DialogueFinished);
}

/// Entrée: affiche la réplique en entier, puis demande la suivante. 1 à 4: vote pour un choix.
fn dialogue_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    localization: Option<Res<Localization>>,
    mut state: ResMut<DialogueState>,
) {
    if state.line.is_none() {
        return;
    }
    if state.is_voting() {
        if state.voted {
            return;
        }
        for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4]
            .into_iter()
            .enumerate()
        {
            if keys.just_pressed(key) && i < state.options.len() {
                state.voted = true;
                commands.client_trigger(DialogueVote(i as u32));
            }
        }
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let length = state
            .line
            .as_ref()
            .map(|l| line_text(localization.as_deref(), l).chars().count())
            .unwrap_or(0) as f32;
        if state.revealed < length {
            state.revealed = length;
        } else {
            commands.client_trigger(DialogueRequestAdvance);
        }
    }
}

fn line_text(localization: Option<&Localization>, line: &ShownLine) -> String {
    localization
        .and_then(|l| l.content(line.key.as_str()))
        .unwrap_or_else(|| line.fallback.clone())
}

fn speaker_color(speaker: &str) -> Color {
    match speaker {
        "knight" => css::STEEL_BLUE,
        "sorcerer" => css::MEDIUM_ORCHID,
        "ranger" => css::FOREST_GREEN,
        "forgebraise" => css::ORANGE_RED,
        _ => css::CRIMSON,
    }
    .into()
}

fn spawn_panel(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            DialoguePanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(120.0),
                left: Val::Percent(10.0),
                width: Val::Percent(80.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.05, 0.12, 0.92)),
            BorderColor::all(Color::from(css::DIM_GRAY)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                DialoguePortrait,
                ImageNode::default(),
                Node { width: Val::Px(80.0), height: Val::Px(80.0), flex_shrink: 0.0, ..default() },
                Visibility::Hidden,
            ));
            panel
                .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), flex_grow: 1.0, ..default() })
                .with_children(|column| {
                    column.spawn((Text::default(), SpeakerText, ui_font(&assets, 16.0)));
                    column.spawn((Text::default(), BodyText, ui_font(&assets, 16.0)));
                    column.spawn((
                        Text::default(),
                        Tr::new("dialogue-continue"),
                        PromptText,
                        ui_font(&assets, 12.0),
                        TextColor(css::GRAY.into()),
                    ));
                });
        });
}

fn update_panel(
    time: Res<Time>,
    localization: Option<Res<Localization>>,
    mut state: ResMut<DialogueState>,
    mut panel: Single<&mut Visibility, With<DialoguePanel>>,
    mut speaker: Single<(&mut Text, &mut TextColor), (With<SpeakerText>, Without<BodyText>, Without<PromptText>)>,
    mut body: Single<&mut Text, (With<BodyText>, Without<SpeakerText>, Without<PromptText>)>,
    mut prompt: Single<&mut Tr, With<PromptText>>,
    art: Res<crate::art::Art>,
    mut portrait: Single<(&mut ImageNode, &mut Visibility), (With<DialoguePortrait>, Without<DialoguePanel>)>,
) {
    let loc = localization.as_deref();
    let Some(line) = state.line.as_ref() else {
        if **panel != Visibility::Hidden {
            **panel = Visibility::Hidden;
        }
        return;
    };
    if **panel != Visibility::Inherited {
        **panel = Visibility::Inherited;
    }

    // La vignette du personnage qui parle (le Château n'en a pas).
    let speaker_class = line.speaker.as_deref().and_then(|s| match s {
        "knight" => Some(HeroClass::Knight),
        "sorcerer" => Some(HeroClass::Sorcerer),
        "ranger" => Some(HeroClass::Ranger),
        "forgebraise" => Some(HeroClass::Forgebraise),
        _ => None,
    });
    let (image, visibility) = &mut *portrait;
    match speaker_class {
        Some(class) => {
            image.image = art.portraits.image.clone();
            image.texture_atlas = Some(TextureAtlas { layout: art.portraits.layout.clone(), index: crate::art::class_row(class) });
            **visibility = Visibility::Inherited;
        }
        None => **visibility = Visibility::Hidden,
    }

    let name = line.speaker.as_deref().map(|s| {
        loc.and_then(|l| l.content(format!("speaker-{s}").as_str()))
            .unwrap_or_else(|| s.to_string())
    });
    let color = line.speaker.as_deref().map(speaker_color).unwrap_or(Color::WHITE);
    let (speaker_text, speaker_color) = &mut *speaker;
    if let Some(name) = name {
        if speaker_text.0 != name {
            speaker_text.0 = name;
        }
    }
    if speaker_color.0 != color {
        speaker_color.0 = color;
    }

    let full = line_text(loc, line);
    let length = full.chars().count() as f32;
    let mut revealed = (state.revealed + TYPEWRITER_SPEED * time.delta_secs()).min(length);
    if state.is_voting() {
        revealed = length;
    }
    let mut shown: String = full.chars().take(revealed as usize).collect();
    if state.is_voting() {
        for (i, option) in state.options.iter().enumerate() {
            let text = loc
                .and_then(|l| l.content(option.key.as_str()))
                .unwrap_or_else(|| option.fallback.clone());
            shown.push_str(&format!("\n\n{}  {text}", i + 1));
        }
    }
    if body.0 != shown {
        body.0 = shown;
    }
    state.revealed = revealed;

    let prompt_key = if state.is_voting() {
        if state.voted { "dialogue-waiting" } else { "dialogue-vote" }
    } else {
        "dialogue-continue"
    };
    if prompt.key != prompt_key {
        prompt.key = prompt_key;
    }
}

/// Retour à l'écran titre: la scène en cours disparaît.
fn reset_dialogue(_: On<crate::title::ReturnToTitle>, mut state: ResMut<DialogueState>) {
    *state = DialogueState::default();
}
