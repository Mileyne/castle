//! Les boss: barre de vie en haut de l'écran, et zones à esquiver annoncées avant chaque geste (`BossAlert`).

use bevy::{color::palettes::css, prelude::*};
use castle_shared::{BossAlert, BossMove, Health, Monster};

use crate::i18n::Tr;
use crate::ui::ui_font;

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_boss_bar)
            .add_observer(on_alert)
            .add_systems(Update, (update_boss_bar, draw_telegraphs));
    }
}

#[derive(Component)]
struct BossBar;

#[derive(Component)]
struct BossName;

#[derive(Component)]
struct BossFill;

fn spawn_boss_bar(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(34.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            bar.spawn((
                Text::default(),
                Tr::new("boss-warg"),
                BossName,
                ui_font(&assets, 18.0),
                TextColor(css::ORANGE_RED.into()),
            ));
            bar.spawn((
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(12.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.12, 0.02, 0.03)),
                BorderColor::all(Color::srgb(0.7, 0.55, 0.2)),
            ))
            .with_children(|frame| {
                frame.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.8, 0.12, 0.1)),
                    BossFill,
                ));
            });
        });
}

/// La barre du boss vivant le plus blessé (ou le seul), tant qu'il y en a un.
fn update_boss_bar(
    bosses: Query<(&Monster, &Health)>,
    mut bar: Single<&mut Visibility, With<BossBar>>,
    mut name: Single<&mut Tr, With<BossName>>,
    mut fill: Single<&mut Node, With<BossFill>>,
) {
    let boss = bosses
        .iter()
        .find_map(|(monster, health)| monster.0.boss_name_key().map(|key| (key, health)));
    let Some((key, health)) = boss else {
        **bar = Visibility::Hidden;
        return;
    };
    **bar = Visibility::Inherited;
    if name.key != key {
        name.key = key;
    }
    let width = Val::Percent((health.current / health.max).clamp(0.0, 1.0) * 100.0);
    if fill.width != width {
        fill.width = width;
    }
}

/// Une zone à esquiver, affichée jusqu'au coup.
#[derive(Component)]
struct Telegraph {
    alert: BossAlert,
    elapsed: f32,
}

fn on_alert(alert: On<BossAlert>, mut commands: Commands) {
    commands.spawn(Telegraph { alert: *alert, elapsed: 0.0 });
}

fn draw_telegraphs(
    mut commands: Commands,
    time: Res<Time>,
    mut gizmos: Gizmos,
    mut telegraphs: Query<(Entity, &mut Telegraph)>,
) {
    for (entity, mut telegraph) in &mut telegraphs {
        telegraph.elapsed += time.delta_secs();
        let alert = telegraph.alert;
        let progress = (telegraph.elapsed / alert.delay).clamp(0.0, 1.0);
        if telegraph.elapsed > alert.delay + 0.1 {
            commands.entity(entity).despawn();
            continue;
        }
        let pulse = 0.55 + 0.35 * (telegraph.elapsed * 18.0).sin().abs();
        match alert.kind {
            BossMove::Slam | BossMove::Nova => {
                let color: Color = if alert.kind == BossMove::Slam {
                    css::RED.into()
                } else {
                    css::MEDIUM_ORCHID.into()
                };
                let center = Isometry2d::from_translation(alert.at);
                gizmos.circle_2d(center, alert.radius, color.with_alpha(pulse));
                // Le disque se remplit jusqu'au coup.
                for k in 1..=5 {
                    gizmos.circle_2d(
                        center,
                        alert.radius * progress * k as f32 / 5.0,
                        color.with_alpha(0.25 * progress),
                    );
                }
            }
            BossMove::Howl => {
                let center = Isometry2d::from_translation(alert.at);
                gizmos.circle_2d(
                    center,
                    20.0 + 60.0 * progress,
                    Color::from(css::WHITE).with_alpha(0.5 * (1.0 - progress)),
                );
            }
        }
    }
}
