//! Dialogues: `dialogue_script` gère le déroulé des scènes, Fluent fournit les textes dans la langue du joueur.
//!
//! Le serveur déclenche les scènes, fait avancer tout le monde en même temps et compte les votes.
//! Chaque client joue la même scène localement; la réplique `intro_01` se lit dans la clé Fluent `dlg-intro_01`.

use bevy::{color::palettes::css, prelude::*};
use bevy_fluent::prelude::Localization;
use castle_shared::*;
use fluent_content::Content;

use crate::dialogue_script::{Runner, Script, Shown};
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
        app.insert_resource(Script::builtin())
            .init_resource::<Runner>()
            .init_resource::<DialogueState>()
            .init_resource::<SkipHold>()
            .add_systems(Startup, (spawn_panel, spawn_prologue_screen))
            .init_resource::<Prologue>()
            .add_systems(Update, (play_voice, drive_prologue, finish_opening, fit_prologue_photos))
            .add_systems(
                Update,
                (
                    (dialogue_input, dialogue_skip_hold)
                        .run_if(can_talk)
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
            ;
    }
}

struct ShownLine {
    speaker: Option<String>,
    key: String,
}

struct ShownOption {
    key: String,
}

/// Ce que le joueur voit en ce moment.
#[derive(Resource, Default)]
pub struct DialogueState {
    line: Option<ShownLine>,
    options: Vec<ShownOption>,
    voted: bool,
    /// Nombre de caractères déjà affichés (effet machine à écrire).
    revealed: f32,
    /// Une réplique du prologue de Samélia est à l'écran.
    in_prologue: bool,
    /// La scène se joue avant que la partie n'existe (le prologue, joué avant le chargement du niveau): rien ne passe par la
    /// simulation, les répliques avancent d'elles-mêmes.
    pub offline: bool,
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

/// Écran à part du prologue: le portrait de la princesse Samélia en grand, sur fond noir, sous le dialogue.
#[derive(Component)]
struct PrologueScreen;

/// Une photo de l'écran du prologue (0: Samélia, 1 à 4: les héros dans l'ordre de `PROLOGUE_PHOTOS`): son opacité suit le fondu
/// et la photo de celui qui parle.
#[derive(Component)]
struct ProloguePhoto(usize);

/// Les photos du prologue: (orateur, fichier, largeur / hauteur). Samélia d'abord, elle parle la première.
const PROLOGUE_PHOTOS: [(&str, &str, f32); 5] = [
    ("samelia", "art/samelia.png", 1376.0 / 768.0),
    ("knight", "art/prologue_knight.png", 1264.0 / 842.0),
    ("sorcerer", "art/prologue_sorcerer.png", 1376.0 / 768.0),
    ("ranger", "art/prologue_ranger.png", 1376.0 / 768.0),
    ("forgebraise", "art/prologue_forgebraise.png", 1376.0 / 768.0),
];
/// Vitesse (part de l'opacité par seconde) du passage d'une photo à la suivante quand l'orateur change.
const PHOTO_SWAP: f32 = 3.0;

/// Où en est l'écran du prologue: fond noir qui couvre la génération du premier niveau, photo qui apparaît en fondu, répliques,
/// puis la photo s'efface vers le noir avant que l'annonce du niveau ne prenne le relais.
#[derive(Resource, Default)]
pub struct Prologue {
    phase: ProloguePhase,
    elapsed: f32,
    /// Au moins une réplique du prologue a été affichée.
    seen: bool,
    /// L'écran est parti (ou n'a pas lieu d'être): l'annonce du niveau peut commencer.
    pub done: bool,
    /// Le prologue est lancé avant la partie (voir `PendingGame`): l'écran apparaît tout de suite, sans attendre Samélia.
    pub started: bool,
    /// La photo de celui qui parle, et l'opacité de chaque photo.
    speaker: usize,
    weights: [f32; 5],
    /// Depuis combien de secondes chaque photo est à l'écran: l'image s'agrandit peu à peu (voir `fit_prologue_photos`).
    shown_for: [f32; 5],
}

#[derive(Default, PartialEq, Clone, Copy)]
enum ProloguePhase {
    #[default]
    Idle,
    FadeIn,
    Show,
    FadeOut,
}

/// Durées (secondes) du fondu d'entrée de la photo, du fondu vers le noir, et attente maximale d'une première réplique.
const PROLOGUE_FADE_IN: f32 = 1.5;
const PROLOGUE_FADE_OUT: f32 = 1.2;
const PROLOGUE_WAIT: f32 = 12.0;

/// La partie à commencer une fois le prologue joué: l'écran reste noir pendant le prologue, puis le niveau se charge.
#[derive(Resource)]
pub struct PendingGame(pub String);

/// Les dialogues peuvent avancer: en partie, ou pendant le prologue qui la précède.
fn can_talk(game: Option<Res<crate::sim::Campaign>>, state: Res<DialogueState>) -> bool {
    game.is_some() || state.offline
}

/// Le prologue est fini (la photo s'est fondue au noir): c'est seulement maintenant que la partie se charge. L'annonce du niveau, sur
/// fond noir, couvre le chargement.
fn finish_opening(mut commands: Commands, pending: Option<Res<PendingGame>>, prologue: Res<Prologue>, mut state: ResMut<DialogueState>) {
    let Some(pending) = pending else {
        return;
    };
    if !prologue.done {
        return;
    }
    let name = pending.0.clone();
    commands.remove_resource::<PendingGame>();
    state.offline = false;
    commands.queue(move |world: &mut World| crate::sim::begin_game(world, &name, true));
}

/// Mène l'écran du prologue. Il apparaît dès que Samélia est dans le niveau (à la toute première arrivée), avant même ses
/// premières paroles, pour cacher le tracé du premier niveau.
fn drive_prologue(
    time: Res<Time>,
    state: Res<DialogueState>,
    guides: Query<(), (With<Guide>, Without<Captive>)>,
    mut prologue: ResMut<Prologue>,
    mut screen: Single<&mut Visibility, With<PrologueScreen>>,
    mut photos: Query<(&ProloguePhoto, &mut ImageNode)>,
) {
    let dt = time.delta_secs();
    prologue.elapsed += dt;
    if state.in_prologue {
        prologue.seen = true;
        let speaker = state.line.as_ref().and_then(|l| l.speaker.as_deref());
        if let Some(index) = PROLOGUE_PHOTOS.iter().position(|(name, ..)| Some(*name) == speaker) {
            prologue.speaker = index;
        }
    }
    // Les photos se fondent l'une dans l'autre quand l'orateur change.
    let speaker = prologue.speaker;
    for (index, weight) in prologue.weights.iter_mut().enumerate() {
        let target = if index == speaker { 1.0 } else { 0.0 };
        *weight = if *weight < target { (*weight + PHOTO_SWAP * dt).min(target) } else { (*weight - PHOTO_SWAP * dt).max(target) };
    }
    let prologue = &mut *prologue;
    for (index, shown) in prologue.shown_for.iter_mut().enumerate() {
        *shown = if prologue.weights[index] > 0.01 { *shown + dt } else { 0.0 };
    }
    let alpha = match prologue.phase {
        ProloguePhase::Idle => {
            if prologue.done {
                // L'écran ne part qu'une image après la fin du prologue: l'annonce du niveau a eu le temps de le remplacer.
                if **screen != Visibility::Hidden {
                    **screen = Visibility::Hidden;
                }
                return;
            }
            if guides.is_empty() && !prologue.started {
                return;
            }
            prologue.phase = ProloguePhase::FadeIn;
            prologue.elapsed = 0.0;
            0.0
        }
        ProloguePhase::FadeIn => {
            if prologue.elapsed >= PROLOGUE_FADE_IN {
                prologue.phase = ProloguePhase::Show;
                prologue.elapsed = 0.0;
            }
            (prologue.elapsed / PROLOGUE_FADE_IN).min(1.0)
        }
        ProloguePhase::Show => {
            // Fini quand les répliques ont été dites (ou passées), ou qu'aucune ne vient.
            if (prologue.seen && !state.in_prologue) || prologue.elapsed > PROLOGUE_WAIT && !prologue.seen {
                prologue.phase = ProloguePhase::FadeOut;
                prologue.elapsed = 0.0;
            }
            1.0
        }
        ProloguePhase::FadeOut => {
            if prologue.elapsed >= PROLOGUE_FADE_OUT {
                prologue.phase = ProloguePhase::Idle;
                prologue.done = true;
                return;
            }
            1.0 - prologue.elapsed / PROLOGUE_FADE_OUT
        }
    };
    if **screen != Visibility::Inherited {
        **screen = Visibility::Inherited;
    }
    let smooth = alpha * alpha * (3.0 - 2.0 * alpha);
    for (photo, mut image) in &mut photos {
        image.color = Color::srgba(1.0, 1.0, 1.0, smooth * prologue.weights[photo.0]);
    }
}

/// Répliques dont la voix est enregistrée (`assets/audio/voice/<identifiant>.ogg`), jouée en même temps que le texte.
const VOICED: [&str; 5] = ["prologue_01", "prologue_02", "prologue_03", "prologue_04", "prologue_05"];

/// La voix en cours; la musique de fond baisse tant qu'elle existe.
#[derive(Component)]
pub struct Voice;

/// Joue la voix de la réplique à l'écran, et coupe celle de la précédente (ou de la scène achevée).
fn play_voice(
    mut commands: Commands,
    assets: Res<AssetServer>,
    state: Res<DialogueState>,
    voices: Query<Entity, With<Voice>>,
    mut shown: Local<Option<String>>,
) {
    let key = state.line.as_ref().map(|l| l.key.clone());
    if *shown == key {
        return;
    }
    for voice in &voices {
        commands.entity(voice).despawn();
    }
    if let Some(id) = key.as_deref().and_then(|k| k.strip_prefix("dlg-")).filter(|id| VOICED.contains(id)) {
        commands.spawn((Voice, AudioPlayer::new(assets.load::<AudioSource>(format!("audio/voice/{id}.ogg"))), PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(crate::settings::volume(crate::settings::Channel::Voice)))));
    }
    *shown = key;
}

/// Les répliques du prologue (`prologue_01`...) se jouent sur cet écran à part.
fn is_prologue(key: &str) -> bool {
    key.starts_with("dlg-prologue_")
}

#[derive(Component)]
struct SpeakerText;

/// Vignette du personnage qui parle.
#[derive(Component)]
struct DialoguePortrait;

#[derive(Component)]
struct BodyText;

/// La suite du texte, pas encore révélée (transparente, mais elle occupe déjà sa place).
#[derive(Component)]
struct BodyHidden;

#[derive(Component)]
struct PromptText;

/// Un identifiant de réplique `intro_01` devient la clé Fluent `dlg-intro_01`.
fn fluent_key(id: &str) -> String {
    format!("dlg-{id}")
}

/// Met à l'écran ce que l'interpréteur vient de rendre. Une scène terminée prévient le serveur, qui attend qu'on la lui dise.
/// La dernière réplique avant des choix reste affichée avec eux, sans attendre d'avancement.
fn show(shown: Shown, runner: &mut Runner, state: &mut DialogueState, commands: &mut Commands) {
    let is_line = matches!(shown, Shown::Line { .. });
    match shown {
        Shown::Line { speaker, id } => {
            state.line = Some(ShownLine { speaker: Some(speaker), key: fluent_key(&id) });
            state.revealed = 0.0;
        }
        Shown::Options(ids) => {
            state.options = ids.iter().map(|id| ShownOption { key: fluent_key(id) }).collect();
            state.voted = false;
        }
        Shown::Finished => {
            // Une scène qui se termine d'elle-même a encore sa dernière réplique à l'écran. Une scène abandonnée (purge,
            // nouveau niveau) a déjà été effacée et le serveur l'a oubliée: l'annoncer pourrait fermer la scène suivante.
            let offline = state.offline;
            if take_finished_scene(state) && !offline {
                commands.trigger(DialogueFinished);
            }
            state.offline = offline;
        }
    }
    if is_line && runner.next_is_choice() {
        let options = runner.advance();
        show(options, runner, state, commands);
    }
}

fn on_begin(
    begin: On<DialogueBegin>,
    script: Res<Script>,
    mut runner: ResMut<Runner>,
    mut state: ResMut<DialogueState>,
    mut commands: Commands,
) {
    if runner.is_running() {
        return;
    }
    match runner.start(&script, &begin.0) {
        Some(shown) => show(shown, &mut runner, &mut state, &mut commands),
        None => warn!("cannot start dialogue '{}': unknown scene", begin.0),
    }
}

fn on_advance(
    _advance: On<DialogueAdvance>,
    mut runner: ResMut<Runner>,
    mut state: ResMut<DialogueState>,
    mut commands: Commands,
) {
    if runner.is_running() && !runner.is_waiting_for_option() {
        let shown = runner.advance();
        show(shown, &mut runner, &mut state, &mut commands);
    }
}

fn on_select(
    select: On<DialogueSelect>,
    mut runner: ResMut<Runner>,
    mut state: ResMut<DialogueState>,
    mut commands: Commands,
) {
    if !runner.is_waiting_for_option() {
        return;
    }
    state.options.clear();
    state.voted = false;
    runner.select(select.0 as usize);
    let shown = runner.advance();
    show(shown, &mut runner, &mut state, &mut commands);
}

/// Efface la scène affichée; vrai si elle s'est terminée d'elle-même (sa dernière réplique était encore à l'écran).
fn take_finished_scene(state: &mut DialogueState) -> bool {
    let finished_by_itself = state.line.is_some();
    *state = DialogueState::default();
    finished_by_itself
}

/// Entrée maintenu: au bout de `SKIP_HOLD_TIME`, on demande au serveur de purger les dialogues (celui en cours et ceux qui
/// allaient démarrer). Le serveur répond par `DialogueAbort`.
fn dialogue_skip_hold(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<DialogueState>,
    mut hold: ResMut<SkipHold>,
) {
    if keys.any_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        hold.held += time.delta_secs();
        if hold.held >= SKIP_HOLD_TIME && !hold.sent {
            hold.sent = true;
            if state.offline {
                commands.trigger(DialogueAbort);
            } else {
                commands.trigger(DialogueSkip);
            }
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
    mut runner: ResMut<Runner>,
    mut state: ResMut<DialogueState>,
    mut hold: ResMut<SkipHold>,
) {
    if fresh.is_empty() {
        return;
    }
    abandon_scene(&mut runner, &mut state);
    *hold = SkipHold::default();
}

/// Arrête la scène en cours et efface ce qu'elle affichait.
fn abandon_scene(runner: &mut Runner, state: &mut DialogueState) {
    let offline = state.offline;
    *state = DialogueState::default();
    state.offline = offline;
    runner.stop();
}

/// Le serveur a purgé les dialogues: la scène en cours s'arrête net.
fn on_abort(_: On<DialogueAbort>, mut runner: ResMut<Runner>, mut state: ResMut<DialogueState>) {
    abandon_scene(&mut runner, &mut state);
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
                commands.trigger(DialogueVote(i as u32));
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
        } else if state.offline {
            commands.trigger(DialogueAdvance);
        } else {
            commands.trigger(DialogueRequestAdvance);
        }
    }
}

fn line_text(localization: Option<&Localization>, line: &ShownLine, mode: crate::controls::InputMode) -> String {
    let text = localization
        .and_then(|l| l.content(line.key.as_str()))
        .unwrap_or_else(|| line.key.clone());
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
        "samelia" => css::MEDIUM_SEA_GREEN,
        "gardener" => css::OLIVE_DRAB,
        _ => css::CRIMSON,
    }
    .into()
}

/// Agrandissement lent des photos: part de la taille gagnée chaque seconde, et plafond.
const PHOTO_ZOOM_RATE: f32 = 0.004;
const PHOTO_ZOOM_MAX: f32 = 0.12;

/// Chaque photo remplit toute la fenêtre, en largeur comme en hauteur: elle est agrandie jusqu'à couvrir l'écran, centrée, et ce qui
/// dépasse est rogné. Elle grandit ensuite très lentement vers son centre (un léger zoom avant). Tailles et places en pourcentages de la
/// fenêtre: elles ne dépendent pas de l'échelle de l'interface.
fn fit_prologue_photos(
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    prologue: Res<Prologue>,
    mut photos: Query<(&ProloguePhoto, &mut Node)>,
) {
    let (w, h) = (window.width(), window.height());
    for (photo, mut node) in &mut photos {
        let ratio = PROLOGUE_PHOTOS[photo.0].2;
        let (pw, ph) = if w / h <= ratio { (h * ratio, h) } else { (w, w / ratio) };
        let zoom = 1.0 + (prologue.shown_for[photo.0] * PHOTO_ZOOM_RATE).min(PHOTO_ZOOM_MAX);
        let (pw, ph) = (pw * zoom, ph * zoom);
        node.width = Val::Percent(pw / w * 100.0);
        node.height = Val::Percent(ph / h * 100.0);
        node.left = Val::Percent((w - pw) / 2.0 / w * 100.0);
        node.top = Val::Percent((h - ph) / 2.0 / h * 100.0);
    }
}

fn spawn_prologue_screen(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            PrologueScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            for (index, (_, file, _)) in PROLOGUE_PHOTOS.iter().enumerate() {
                // Chaque photo occupe tout l'écran (centrée), les unes sur les autres.
                screen
                    .spawn(Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        overflow: Overflow::clip(),
                        ..default()
                    })
                    .with_children(|slot| {
                        // La taille et la place de la photo sont réglées chaque image par `fit_prologue_photos` (elle remplit l'écran).
                        slot.spawn((
                            ProloguePhoto(index),
                            ImageNode::new(assets.load(*file)).with_color(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                            Node { position_type: PositionType::Absolute, ..default() },
                        ));
                    });
            }
        });
}

fn spawn_panel(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            DialoguePanel,
            GlobalZIndex(105),
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
                    // Le texte entier est mis en page dès le début, pour que la case ait sa hauteur définitive: la partie pas encore
                    // révélée est un second morceau, transparent.
                    column
                        .spawn((Text::default(), BodyText, ui_font(&assets, 16.0)))
                        .with_child((TextSpan::default(), BodyHidden, ui_font(&assets, 16.0), TextColor(Color::NONE)));
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
    mut hidden: Single<&mut TextSpan, With<BodyHidden>>,
    mut prompt: Single<&mut Tr, With<PromptText>>,
    art: Res<crate::art::Art>,
    mut portrait: Single<(&mut ImageNode, &mut Visibility), (With<DialoguePortrait>, Without<DialoguePanel>)>,
    skip: Res<SkipHold>,
    mode: Res<crate::controls::InputMode>,
) {
    let loc = localization.as_deref();
    let showing_prologue = state.line.as_ref().is_some_and(|l| is_prologue(&l.key));
    if showing_prologue != state.in_prologue {
        state.in_prologue = showing_prologue;
    }
    let Some(line) = state.line.as_ref() else {
        if **panel != Visibility::Hidden {
            **panel = Visibility::Hidden;
        }
        return;
    };
    let prologue = is_prologue(&line.key);
    if **panel != Visibility::Inherited {
        **panel = Visibility::Inherited;
    }

    // La vignette du personnage qui parle (le Château n'en a pas).
    let speaker_portrait = line.speaker.as_deref().and_then(|s| match s {
        "knight" => Some(crate::art::class_row(HeroClass::Knight)),
        "sorcerer" => Some(crate::art::class_row(HeroClass::Sorcerer)),
        "ranger" => Some(crate::art::class_row(HeroClass::Ranger)),
        "forgebraise" => Some(crate::art::class_row(HeroClass::Forgebraise)),
        "samelia" => Some(crate::art::SAMELIA_PORTRAIT),
        _ => None,
    });
    let (image, visibility) = &mut *portrait;
    // Sur l'écran du prologue, Samélia est déjà là en grand: pas de vignette pour elle.
    let speaker_portrait = speaker_portrait.filter(|index| !(prologue && *index == crate::art::SAMELIA_PORTRAIT));
    match speaker_portrait {
        Some(index) => {
            image.image = art.portraits.image.clone();
            image.texture_atlas = Some(TextureAtlas { layout: art.portraits.layout.clone(), index });
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
                .unwrap_or_else(|| option.key.clone());
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
    // Ce qui reste à écrire, invisible mais déjà mis en page.
    let rest: String = if state.is_voting() { String::new() } else { full.chars().skip(revealed as usize).collect() };
    if hidden.0 != rest {
        hidden.0 = rest;
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
fn reset_dialogue(_: On<crate::title::ReturnToTitle>, mut state: ResMut<DialogueState>, mut prologue: ResMut<Prologue>) {
    *state = DialogueState::default();
    *prologue = Prologue::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    const FR: &str = include_str!("../assets/locales/fr-FR/main.ftl");
    const EN: &str = include_str!("../assets/locales/en-US/main.ftl");

    fn ftl_ids(text: &str) -> Vec<String> {
        text.lines().filter_map(|l| l.strip_prefix("dlg-")).filter_map(|l| l.split_once(" =")).map(|(id, _)| id.to_string()).collect()
    }

    #[test]
    fn every_line_is_translated_in_both_languages_and_nothing_is_orphaned() {
        let ids = Script::builtin().ids();
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
        let script = Script::builtin();
        for node in ["royal_door", "puzzle_rune", "puzzle_gears", "puzzle_target"] {
            assert!(script.has_scene(node), "scène {node} manquante");
        }
        // La scène de la porte annonce les couleurs des trois classes qui agissent, et l'acier du Chevalier.
        let door: String = FR.lines().filter(|l| l.starts_with("dlg-royal_door_")).collect();
        for word in ["violet", "orange", "vert", "acier"] {
            assert!(door.contains(word), "la scène de la porte ne parle pas de {word}");
        }
    }

    fn shown_line() -> ShownLine {
        ShownLine { speaker: Some("knight".into()), key: "dlg-intro_01".into() }
    }

    /// Un dialogue laissé à l'écran disparaît au nouveau niveau, avec ses choix en attente.
    #[test]
    fn a_new_level_clears_an_unfinished_scene() {
        let mut app = App::new();
        app.init_resource::<SkipHold>().init_resource::<Runner>();
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
