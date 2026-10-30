//! Écrans noirs des transitions: le bilan du niveau terminé (à l'entrée dans le portail), puis l'annonce du niveau dans
//! lequel on entre, au lancement de la partie comme à chaque passage de portail. Le bilan s'estompe sur l'annonce, qui
//! s'estompe à son tour sur le jeu.

use bevy::{color::palettes::css, prelude::*};
use bevy_replicon::prelude::*;
use castle_shared::{ContinueLevel, Expedition, LevelSummary};

use crate::i18n::Tr;
use crate::title::{ReturnToTitle, TitleActive};
use crate::ui::ui_font;

/// Secondes d'affichage plein, puis de fondu vers le jeu.
const HOLD: f32 = 2.2;
const FADE: f32 = 0.8;
/// Bilan: fondu d'entrée de l'écran, décalage entre deux lignes, fondu d'une ligne, fondu de sortie sur l'annonce.
const SUMMARY_FADE_IN: f32 = 0.8;
const SUMMARY_ROW_DELAY: f32 = 0.3;
const SUMMARY_ROW_FADE: f32 = 0.5;
const SUMMARY_LEAVE: f32 = 0.7;

#[derive(Resource, Default)]
pub struct LevelIntro {
    /// Une nouvelle expédition est arrivée: l'annonce doit être (re)lancée dès que l'écran titre est fermé.
    pending: bool,
    /// Temps écoulé depuis le début de l'annonce, tant qu'elle est affichée.
    elapsed: Option<f32>,
    /// Le bilan du niveau terminé, tant qu'il est affiché.
    summary: Option<Summary>,
}

struct Summary {
    elapsed: f32,
    /// Quand l'invite à continuer apparaît, et si le joueur a déjà validé (il attend alors les autres).
    ready_at: f32,
    waiting: bool,
    /// Temps écoulé depuis que le niveau suivant est arrivé: le bilan s'estompe alors.
    leaving: Option<f32>,
}

impl LevelIntro {
    /// Le bilan de fin de niveau est affiché.
    pub fn summary_active(&self) -> bool {
        self.summary.is_some()
    }
}

/// Condition d'exécution: le joueur peut agir (ni écran titre, ni bilan, ni annonce de niveau).
pub fn player_can_act(title: Res<TitleActive>, intro: Res<LevelIntro>) -> bool {
    !title.0 && intro.elapsed.is_none() && intro.summary.is_none()
}

pub struct LevelIntroPlugin;

impl Plugin for LevelIntroPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelIntro>()
            .add_observer(reset_intro)
            .add_observer(show_summary)
            .add_systems(Update, (queue_intro, start_intro, run_intro, confirm_summary, run_summary).chain());
    }
}

#[derive(Component)]
struct IntroScreen;

/// Écran du bilan; ses textes portent le délai avant leur apparition.
#[derive(Component)]
struct SummaryScreen;

#[derive(Component)]
struct SummaryText(f32);

/// L'invite « appuyez pour continuer »: elle pulse, et devient « en attente » une fois validée.
#[derive(Component)]
struct SummaryPrompt;

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
    // Le bilan du niveau précédent s'estompe sur cette annonce.
    if let Some(summary) = intro.summary.as_mut() {
        summary.leaving.get_or_insert(0.0);
    }
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

/// Retour à l'écran titre: plus d'annonce en cours ni en attente.
fn reset_intro(
    _: On<ReturnToTitle>,
    mut commands: Commands,
    mut intro: ResMut<LevelIntro>,
    screens: Query<Entity, Or<(With<IntroScreen>, With<SummaryScreen>)>>,
) {
    *intro = LevelIntro::default();
    for screen in &screens {
        commands.entity(screen).despawn();
    }
}

/// Le niveau est terminé: l'écran se couvre de noir et le bilan apparaît ligne par ligne.
fn show_summary(
    summary: On<LevelSummary>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut intro: ResMut<LevelIntro>,
    old: Query<Entity, With<SummaryScreen>>,
) {
    for screen in &old {
        commands.entity(screen).despawn();
    }
    // L'invite à continuer arrive après la dernière ligne.
    let seconds = summary.seconds as u32;
    let mut rows: Vec<(&'static str, String)> = vec![
        ("summary-time", format!("{}:{:02}", seconds / 60, seconds % 60)),
        ("summary-kills", summary.kills.to_string()),
        ("summary-generators", summary.generators.to_string()),
        ("summary-damage", (summary.damage_taken.round() as u32).to_string()),
        ("summary-downed", summary.downed.to_string()),
        ("summary-gold", summary.gold.to_string()),
    ];
    // Le reste n'apparaît que si l'équipe en a trouvé.
    for (key, count) in [
        ("summary-keys", summary.keys),
        ("summary-food", summary.food),
        ("summary-potions", summary.potions),
        ("summary-pieces", summary.pieces),
    ] {
        if count > 0 {
            rows.push((key, count.to_string()));
        }
    }
    let fade_in = SUMMARY_FADE_IN * 0.5;
    let prompt_at = fade_in + SUMMARY_ROW_DELAY * (rows.len() + 2) as f32;
    intro.summary = Some(Summary { elapsed: 0.0, ready_at: prompt_at, waiting: false, leaving: None });
    let white = TextColor(css::WHITE.into());
    let silver = TextColor(css::SILVER.into());
    commands
        .spawn((
            SummaryScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(26.0),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.0)),
            GlobalZIndex(95),
        ))
        .with_children(|screen| {
            screen.spawn(Node { column_gap: Val::Px(14.0), ..default() }).with_children(|line| {
                line.spawn((Text::default(), Tr::new("level-word"), ui_font(&assets, 30.0), white, SummaryText(fade_in)));
                line.spawn((Text::new(summary.level.to_string()), ui_font(&assets, 30.0), white, SummaryText(fade_in)));
                line.spawn((Text::default(), Tr::new("level-complete"), ui_font(&assets, 30.0), white, SummaryText(fade_in)));
            });
            screen
                .spawn(Node {
                    width: Val::Px(560.0),
                    max_width: Val::Percent(92.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|list| {
                    for (index, (key, value)) in rows.iter().cloned().enumerate() {
                        let delay = fade_in + SUMMARY_ROW_DELAY * (index + 1) as f32;
                        list.spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((Text::default(), Tr::new(key), ui_font(&assets, 14.0), silver, SummaryText(delay)));
                            row.spawn((Text::new(value), ui_font(&assets, 14.0), white, SummaryText(delay)));
                        });
                    }
                });
            screen.spawn((
                Text::default(),
                Tr::new("summary-continue"),
                ui_font(&assets, 14.0),
                TextColor(css::GOLD.into()),
                SummaryText(prompt_at),
                SummaryPrompt,
            ));
        });
}

/// Le joueur valide le bilan (Entrée ou Espace, A sur une manette, OK sur écran tactile): le niveau suivant démarre
/// quand tous les joueurs ont validé.
fn confirm_summary(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut intro: ResMut<LevelIntro>,
    mut prompts: Query<&mut Tr, With<SummaryPrompt>>,
) {
    let Some(summary) = intro.summary.as_mut() else {
        return;
    };
    if summary.waiting || summary.leaving.is_some() || summary.elapsed < summary.ready_at {
        return;
    }
    if !(keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space)) {
        return;
    }
    summary.waiting = true;
    commands.client_trigger(ContinueLevel);
    for mut prompt in &mut prompts {
        prompt.key = "summary-waiting";
    }
}

/// Fait vivre le bilan: fondu d'entrée du noir, apparition des lignes une à une, puis fondu de sortie quand le niveau
/// suivant est arrivé.
fn run_summary(
    mut commands: Commands,
    time: Res<Time>,
    mut intro: ResMut<LevelIntro>,
    mut screens: Query<(Entity, &mut BackgroundColor), With<SummaryScreen>>,
    mut texts: Query<(&SummaryText, &mut TextColor, Has<SummaryPrompt>)>,
) {
    let dt = time.delta_secs();
    let pulse = 0.65 + 0.35 * (time.elapsed_secs() * 4.0).sin();
    let Some(summary) = intro.summary.as_mut() else {
        return;
    };
    summary.elapsed += dt;
    let keep = match summary.leaving.as_mut() {
        Some(leaving) => {
            *leaving += dt;
            1.0 - (*leaving / SUMMARY_LEAVE).clamp(0.0, 1.0)
        }
        None => 1.0,
    };
    let elapsed = summary.elapsed;
    let done = summary.leaving.is_some() && keep <= 0.0;
    for (entity, mut background) in &mut screens {
        if done {
            commands.entity(entity).despawn();
        } else {
            background.0 = Color::BLACK.with_alpha((elapsed / SUMMARY_FADE_IN).clamp(0.0, 1.0) * keep);
        }
    }
    for (text, mut color, is_prompt) in &mut texts {
        let appear = ((elapsed - text.0) / SUMMARY_ROW_FADE).clamp(0.0, 1.0);
        color.0 = color.0.with_alpha(appear * keep * if is_prompt { pulse } else { 1.0 });
    }
    if done {
        intro.summary = None;
    }
}
