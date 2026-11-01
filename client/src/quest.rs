//! Objectif du niveau: affichage en haut de l'écran, et sprites des reliques et des prisonniers.

use bevy::{color::palettes::css, prelude::*};
use bevy::sprite::Anchor;
use castle_shared::{Captive, Guide, Position, Quest, Relic};

use crate::art::{Art, Prop, SPRITE_SIZE, object};
use crate::i18n::Tr;
use crate::ui::ui_font;

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_quest_hud)
            .add_observer(on_guide_gone)
            .add_systems(Update, (dress_quest_entities, update_quest_hud, fade_departed_guides));
    }
}

#[derive(Component)]
struct QuestHud;

#[derive(Component)]
struct QuestName;

#[derive(Component)]
struct QuestCount;

fn spawn_quest_hud(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            QuestHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(12.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|hud| {
            hud.spawn((Text::default(), Tr::new("quest-hunt"), QuestName, ui_font(&assets, 14.0), TextColor(css::GOLD.into())));
            hud.spawn((Text::default(), QuestCount, ui_font(&assets, 14.0), TextColor(css::WHITE.into())));
        });
}

/// L'objectif à montrer: le premier à accomplir (le boss, puis la chasse, avant les autres), ou le dernier une fois tous remplis.
fn current_quest<'a>(quests: impl Iterator<Item = &'a Quest>) -> Option<&'a Quest> {
    let mut all: Vec<&Quest> = quests.collect();
    all.sort_by_key(|quest| match quest.kind {
        castle_shared::QuestKind::Boss => 0,
        castle_shared::QuestKind::Hunt => 1,
        _ => 2,
    });
    all.iter()
        .copied()
        .find(|quest| !quest.is_done())
        .or_else(|| all.last().copied())
}

fn update_quest_hud(
    quests: Query<&Quest>,
    mut hud: Single<&mut Visibility, With<QuestHud>>,
    mut name: Single<(&mut Tr, &mut TextColor), (With<QuestName>, Without<QuestCount>)>,
    mut count: Single<&mut Text, With<QuestCount>>,
) {
    let Some(quest) = current_quest(quests.iter()) else {
        **hud = Visibility::Hidden;
        return;
    };
    // « Objectif accompli » seulement quand tous les objectifs le sont.
    let all_done = quests.iter().all(|q| q.is_done());
    **hud = Visibility::Inherited;
    let (tr, color) = &mut *name;
    let (key, shown, tint) = if all_done {
        ("quest-done", String::new(), css::LIME)
    } else {
        (quest.kind.name_key(), format!("{}/{}", quest.progress, quest.goal), css::GOLD)
    };
    if tr.key != key {
        tr.key = key;
    }
    if color.0 != Color::from(tint) {
        color.0 = tint.into();
    }
    if count.0 != shown {
        count.0 = shown;
    }
}

/// Donne un sprite aux reliques (provisoire) et aux prisonniers (un prisonnier dans une cage).
fn dress_quest_entities(
    mut commands: Commands,
    art: Res<Art>,
    relics: Query<(Entity, &Position), Added<Relic>>,
    captives: Query<(Entity, &Position, Has<Guide>), Added<Captive>>,
    guides: Query<(Entity, &Position), (Added<Guide>, Without<Captive>)>,
) {
    let size = Vec2::splat(SPRITE_SIZE);
    for (entity, pos) in &relics {
        let (mut sprite, anchor) = art.prop_sprite(object::KEY, 0, size);
        sprite.color = Color::srgb(0.55, 1.0, 1.0);
        commands.entity(entity).insert((sprite, Prop(object::KEY), anchor, Transform::from_translation(pos.0.extend(0.6))));
    }
    for (entity, pos, is_guide) in &captives {
        let row = if is_guide { object::SAMELIA_CAGED } else { object::CAPTIVE };
        let (sprite, anchor) = art.prop_sprite(row, 0, size);
        commands.entity(entity).insert((sprite, Prop(row), anchor, Transform::from_translation(pos.0.extend(0.6))));
    }
    for (entity, pos) in &guides {
        let (sprite, anchor) = art.prop_sprite(object::SAMELIA, 0, size);
        commands.entity(entity).insert((sprite, Prop(object::SAMELIA), anchor, Transform::from_translation(pos.0.extend(0.6))));
    }
}

/// Durée de la disparition de Samélia, et rayon final du halo (unités du monde).
const DEPARTURE: f32 = 2.0;
const HALO_RADIUS: f32 = 46.0;

/// Samélia qui s'en va: son sprite reste un instant, de plus en plus lumineux, au centre d'un halo qui s'élargit.
#[derive(Component)]
struct Departing(f32);

fn on_guide_gone(
    gone: On<Remove<Guide>>,
    guides: Query<(&Sprite, &Anchor, &Transform), Without<Captive>>,
    mut commands: Commands,
) {
    if let Ok((sprite, anchor, transform)) = guides.get(gone.entity) {
        commands.spawn((Departing(0.0), sprite.clone(), *anchor, *transform));
    }
}

fn fade_departed_guides(
    mut commands: Commands,
    time: Res<Time>,
    mut gizmos: Gizmos,
    mut departing: Query<(Entity, &mut Departing, &mut Sprite, &Transform)>,
) {
    for (entity, mut departure, mut sprite, transform) in &mut departing {
        departure.0 += time.delta_secs();
        let t = (departure.0 / DEPARTURE).clamp(0.0, 1.0);
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // Le sprite blanchit puis s'efface; des anneaux dorés s'élargissent autour de sa taille.
        let glow = (t * 2.0).min(1.0);
        sprite.color = Color::srgba(1.0 + glow, 1.0 + glow, 1.0 + 0.6 * glow, 1.0 - t * t);
        let center = Isometry2d::from_translation(transform.translation.truncate() + Vec2::new(0.0, 12.0));
        for k in 0..4 {
            let phase = (t * 1.4 - k as f32 * 0.15).clamp(0.0, 1.0);
            gizmos.circle_2d(center, HALO_RADIUS * phase, Color::srgba(1.0, 0.92, 0.55, 0.8 * (1.0 - phase)));
        }
        gizmos.circle_2d(center, 10.0 + 30.0 * t, Color::srgba(1.0, 1.0, 0.85, 0.5 * (1.0 - t)));
    }
}
