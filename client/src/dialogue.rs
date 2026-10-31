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
/// Durée pendant laquelle il faut maintenir Entrée pour purger les dialogues, en secondes.
pub const SKIP_HOLD_TIME: f32 = 0.7;

/// Entrée maintenu: temps écoulé, et demande déjà envoyée (une seule par appui).
#[derive(Resource, Default)]
pub struct SkipHold {
    held: f32,
    sent: bool,
}

impl SkipHold {
    /// Progression de la purge, de 0 à 1.
    pub fn progress(&self) -> f32 {
        (self.held / SKIP_HOLD_TIME).min(1.0)
    }
}

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(YarnSpinnerPlugin::new())
            .init_resource::<DialogueState>()
            .init_resource::<SkipHold>()
            .add_systems(Startup, spawn_panel)
            .add_systems(
                Update,
                (
                    spawn_runner.run_if(resource_added::<YarnProject>),
                    (dialogue_input, dialogue_skip_hold)
                        .run_if(in_state(ClientState::Connected))
                        .run_if(crate::level_intro::player_can_act)
                        .run_if(crate::pause::not_paused),
                    update_panel,
                )
                    .chain(),
            )
            .add_observer(on_begin)
            .add_observer(reset_dialogue)
            .add_observer(on_abort)
            .add_systems(Update, purge_on_new_level)
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
    /// Une réplique est à l'écran (et attend Entrée, ou un vote).
    pub fn showing(&self) -> bool {
        self.line.is_some()
    }

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

/// Efface la scène affichée; vrai si elle s'est terminée d'elle-même (sa dernière réplique était encore à l'écran).
fn take_finished_scene(state: &mut DialogueState) -> bool {
    let finished_by_itself = state.line.is_some();
    *state = DialogueState::default();
    finished_by_itself
}

fn on_completed(_done: On<DialogueCompleted>, mut commands: Commands, mut state: ResMut<DialogueState>) {
    // Une scène qui se termine d'elle-même a encore sa dernière réplique à l'écran, et le serveur attend qu'on la lui dise.
    // Une scène que nous avons abandonnée (purge, nouveau niveau) a déjà été effacée, et le serveur l'a oubliée: l'annoncer
    // maintenant pourrait fermer la scène suivante.
    if take_finished_scene(&mut state) {
        commands.client_trigger(DialogueFinished);
    }
}

/// Entrée maintenu: au bout de `SKIP_HOLD_TIME`, on demande au serveur de purger les dialogues (celui en cours et ceux qui
/// allaient démarrer). Le serveur répond par `DialogueAbort`.
fn dialogue_skip_hold(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut hold: ResMut<SkipHold>,
) {
    if keys.any_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        hold.held += time.delta_secs();
        if hold.held >= SKIP_HOLD_TIME && !hold.sent {
            hold.sent = true;
            commands.client_trigger(DialogueSkip);
        }
    } else if hold.held > 0.0 || hold.sent {
        *hold = SkipHold::default();
    }
}

/// Un nouveau château (nouveau niveau, ou retour au début du niveau): tout dialogue resté à l'écran est abandonné, avec ce
/// qui y était attendu, pour qu'il ne reparaisse pas dans le niveau suivant. Le serveur envoie aussi `DialogueAbort`; ceci
/// couvre le cas où l'ordre d'arrivée des messages aurait changé.
fn purge_on_new_level(
    fresh: Query<(), Added<Expedition>>,
    mut runners: Query<&mut DialogueRunner>,
    mut state: ResMut<DialogueState>,
    mut hold: ResMut<SkipHold>,
) {
    if fresh.is_empty() {
        return;
    }
    abandon_scene(&mut runners, &mut state);
    *hold = SkipHold::default();
}

/// Arrête la scène en cours et efface ce qu'elle affichait.
fn abandon_scene(runners: &mut Query<&mut DialogueRunner>, state: &mut DialogueState) {
    *state = DialogueState::default();
    if let Ok(mut runner) = runners.single_mut() {
        if runner.is_running() {
            runner.stop();
        }
    }
}

/// Le serveur a purgé les dialogues: la scène en cours s'arrête net.
fn on_abort(_: On<DialogueAbort>, mut runners: Query<&mut DialogueRunner>, mut state: ResMut<DialogueState>) {
    abandon_scene(&mut runners, &mut state);
}

/// Entrée: affiche la réplique en entier, puis demande la suivante. 1 à 4: vote pour un choix.
fn dialogue_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    localization: Option<Res<Localization>>,
    mode: Res<crate::controls::InputMode>,
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
            .map(|l| line_text(localization.as_deref(), l, *mode).chars().count())
            .unwrap_or(0) as f32;
        if state.revealed < length {
            state.revealed = length;
        } else {
            commands.client_trigger(DialogueRequestAdvance);
        }
    }
}

fn line_text(localization: Option<&Localization>, line: &ShownLine, mode: crate::controls::InputMode) -> String {
    let text = localization
        .and_then(|l| l.content(line.key.as_str()))
        .unwrap_or_else(|| line.fallback.clone());
    // Les noms de commandes (`%action%`...) sont ceux de l'appareil utilisé.
    match localization {
        Some(l) => crate::i18n::resolve_controls(text, l, mode),
        None => text,
    }
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
    skip: Res<SkipHold>,
    mode: Res<crate::controls::InputMode>,
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

    let full = line_text(loc, line, *mode);
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
            let text = match loc {
                Some(l) => crate::i18n::resolve_controls(text, l, *mode),
                None => text,
            };
            shown.push_str(&format!("\n\n{}  {text}", i + 1));
        }
    }
    if body.0 != shown {
        body.0 = shown;
    }
    state.revealed = revealed;

    // Pendant que Entrée est maintenu, l'invite annonce la purge; sinon elle rappelle comment avancer (et comment tout passer).
    let prompt_key = if skip.progress() > 0.2 {
        "dialogue-skipping"
    } else if state.is_voting() {
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

#[cfg(test)]
mod tests {
    use super::*;

    const YARN: &str = include_str!("../assets/dialogue/castle.yarn");
    const FR: &str = include_str!("../assets/locales/fr-FR/main.ftl");
    const EN: &str = include_str!("../assets/locales/en-US/main.ftl");

    /// Identifiants `#line:xxx` du script.
    fn yarn_ids() -> Vec<String> {
        YARN.split("#line:")
            .skip(1)
            .map(|rest| rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect())
            .collect()
    }

    fn ftl_ids(text: &str) -> Vec<String> {
        text.lines().filter_map(|l| l.strip_prefix("dlg-")).filter_map(|l| l.split_once(" =")).map(|(id, _)| id.to_string()).collect()
    }

    #[test]
    fn every_line_is_translated_in_both_languages_and_nothing_is_orphaned() {
        let ids = yarn_ids();
        for first in ["intro_01", "royal_door_06", "puzzle_rune_01", "puzzle_gears_01", "puzzle_target_01", "alcove_01"] {
            assert!(ids.iter().any(|id| id == first), "la réplique {first} manque au script");
        }
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "identifiants de réplique en double");
        for (name, text) in [("fr-FR", FR), ("en-US", EN)] {
            let translated = ftl_ids(text);
            for id in &ids {
                assert!(translated.contains(id), "{name}: la réplique {id} n'est pas traduite");
            }
            for id in &translated {
                assert!(ids.contains(id), "{name}: la clé dlg-{id} ne correspond à aucune réplique");
            }
        }
    }

    #[test]
    fn puzzle_dialogues_exist_and_name_each_colour() {
        for node in ["royal_door", "puzzle_rune", "puzzle_gears", "puzzle_target"] {
            assert!(YARN.contains(&format!("title: {node}")), "scène {node} manquante");
        }
        // La scène de la porte annonce les couleurs des trois classes qui agissent, et l'acier du Chevalier.
        let door = &YARN[YARN.find("title: royal_door").unwrap()..YARN.find("title: puzzle_rune").unwrap()];
        for word in ["violet", "orange", "vert", "acier"] {
            assert!(door.contains(word), "la scène de la porte ne parle pas de {word}");
        }
    }

    fn shown_line() -> ShownLine {
        ShownLine { speaker: Some("knight".into()), key: "dlg-intro_01".into(), fallback: "Bonjour".into() }
    }

    /// Un dialogue laissé à l'écran disparaît au nouveau niveau, avec ses choix en attente.
    #[test]
    fn a_new_level_clears_an_unfinished_scene() {
        let mut app = App::new();
        app.init_resource::<SkipHold>();
        app.insert_resource(DialogueState { line: Some(shown_line()), voted: true, revealed: 12.0, ..default() });
        app.add_systems(Update, purge_on_new_level);
        // Sans nouveau château, la scène reste.
        app.update();
        assert!(app.world().resource::<DialogueState>().line.is_some());
        // Un nouveau château arrive: elle est purgée, et Entrée maintenu est remis à zéro.
        app.world_mut().resource_mut::<SkipHold>().held = 0.5;
        app.world_mut().spawn(Expedition { seed: 7, region: Region::Crypts, level: 2 });
        app.update();
        let state = app.world().resource::<DialogueState>();
        assert!(state.line.is_none() && !state.voted && state.revealed == 0.0);
        assert_eq!(app.world().resource::<SkipHold>().progress(), 0.0);
        // Et elle ne se purge pas à chaque image.
        app.world_mut().resource_mut::<DialogueState>().line = Some(shown_line());
        app.update();
        assert!(app.world().resource::<DialogueState>().line.is_some(), "une scène du nouveau niveau n'est pas purgée");
    }

    /// Seule une scène terminée d'elle-même prévient le serveur: une scène abandonnée est déjà oubliée de lui.
    #[test]
    fn only_a_scene_that_finished_by_itself_tells_the_server() {
        let mut shown = DialogueState { line: Some(shown_line()), ..default() };
        assert!(take_finished_scene(&mut shown));
        assert!(shown.line.is_none());
        // Déjà effacée (purge, nouveau niveau): rien à annoncer, sinon on fermerait la scène suivante côté serveur.
        assert!(!take_finished_scene(&mut shown));
    }

    #[test]
    fn skip_hold_progress_grows_to_one() {
        let mut hold = SkipHold::default();
        assert_eq!(hold.progress(), 0.0);
        hold.held = SKIP_HOLD_TIME / 2.0;
        assert!((hold.progress() - 0.5).abs() < 1e-5);
        hold.held = SKIP_HOLD_TIME * 3.0;
        assert_eq!(hold.progress(), 1.0);
    }
}
