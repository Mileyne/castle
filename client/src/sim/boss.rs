//! Les boss: un personnage unique par niveau de boss, à vaincre en groupe pour libérer la sortie.
//!
//! Chaque boss enchaîne ses coups ordinaires avec un geste spécial annoncé à l'avance (`BossAlert`: les clients dessinent
//! la zone à esquiver), et appelle des renforts quand sa vie passe certains seuils.
//!
//! - **Warg** (niveau 4): hurle régulièrement pour rameuter des loups (davantage sous la moitié de vie), et mord.
//! - **Troll** (niveau 8): frappe le sol de ses poings, zone à quitter avant le coup. À mi-vie il appelle des gobelins, se
//!   met en rage (plus vite, coups plus rapprochés).
//! - **Lord Muldant** (niveau 12): un colosse qui fonce au corps à corps, lance une onde de foudre noire autour de lui (et,
//!   sous la moitié de vie, frappe aussi le sol), et appelle des squelettes à deux tiers puis à un tiers de vie.

use super::*;

/// Ce que fait un boss en ce moment.
#[derive(Clone, Copy, PartialEq)]
enum Act {
    /// Poursuit un héros et frappe au corps à corps.
    Hunt,
    /// Se prépare (immobile) à un geste que les héros peuvent esquiver.
    Windup(BossMove),
    /// Reprend son souffle après un geste.
    Recover,
}

/// L'état d'un boss (côté serveur seulement).
#[derive(Component)]
pub struct BossBrain {
    act: Act,
    timer: f32,
    /// Délai avant le prochain geste spécial.
    special_in: f32,
    /// Combien de seuils de vie (appel de renforts) sont déjà passés.
    waves: usize,
}

impl BossBrain {
    pub fn new() -> Self {
        Self {
            act: Act::Hunt,
            timer: 0.0,
            special_in: 3.0,
            waves: 0,
        }
    }
}

/// Le chef de meute ne rameute plus de loups quand tant de loups sont déjà en vie.
const MAX_PACK: usize = 7;
const SLAM_WINDUP: f32 = 1.0;
const SLAM_RADIUS: f32 = 52.0;
const SLAM_DAMAGE: f32 = 42.0;
const NOVA_WINDUP: f32 = 1.0;
const NOVA_RADIUS: f32 = 85.0;
const NOVA_DAMAGE: f32 = 30.0;
const HOWL_TIME: f32 = 1.0;
const RECOVER_TIME: f32 = 0.7;

/// Seuils de vie (part restante) auxquels le boss appelle des renforts, et ce qu'il appelle.
fn waves(kind: MonsterKind) -> (&'static [f32], MonsterKind, usize) {
    match kind {
        MonsterKind::Warg => (&[], MonsterKind::Wolf, 2),
        MonsterKind::Troll => (&[0.5], MonsterKind::Goblin, 3),
        // Le Jardinier appelle de petits gobelins à deux tiers puis à un tiers de vie.
        MonsterKind::Gardener => (&[0.66, 0.33], MonsterKind::Goblin, 4),
        MonsterKind::Dragon => (&[0.5], MonsterKind::Skeleton, 3),
        _ => (&[0.66, 0.33], MonsterKind::Skeleton, 3),
    }
}

/// Délai entre deux gestes spéciaux; il raccourcit sous la moitié de vie.
fn special_interval(kind: MonsterKind, enraged: bool) -> f32 {
    let base = match kind {
        MonsterKind::Warg => 6.5,
        MonsterKind::Troll | MonsterKind::Gardener => 4.0,
        _ => 4.5,
    };
    if enraged { base * 0.7 } else { base }
}

/// Le geste spécial de ce boss.
fn special(kind: MonsterKind, enraged: bool) -> BossMove {
    match kind {
        MonsterKind::Warg => BossMove::Howl,
        MonsterKind::Troll | MonsterKind::Gardener => BossMove::Slam,
        MonsterKind::Dragon => BossMove::Nova,
        // Muldant alterne la foudre noire et les coups de poing au sol une fois enragé.
        _ if enraged => BossMove::Slam,
        _ => BossMove::Nova,
    }
}

pub fn boss_ai(
    mut commands: Commands,
    time: Res<Time>,
    room: Res<Room>,
    layout: Res<Layout>,
    level: Res<Level>,
    mut rng: ResMut<Rng>,
    minions: Query<&Monster, Without<BossBrain>>,
    mut bosses: Query<
        (
            Entity,
            &mut Position,
            &Health,
            &Monster,
            &mut BossBrain,
            &mut Cooldown,
            &mut Strikes,
        ),
        Without<Player>,
    >,
    mut players: Query<
        (Entity, &Position, &mut Health, Has<Shielded>),
        (With<Player>, Without<Downed>, Without<BossBrain>),
    >,
) {
    let dt = time.delta_secs();
    let ease = ease(level.0);
    for (_entity, mut pos, health, monster, mut brain, mut cooldown, mut strikes) in &mut bosses {
        let kind = monster.0;
        let stats = kind.stats();
        let fraction = health.current / health.max;
        let enraged = fraction < 0.5;
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        brain.timer -= dt;
        brain.special_in -= dt;
        let speed = stats.speed
            * ease
            * if enraged && kind == MonsterKind::Troll {
                1.4
            } else {
                1.0
            };
        let home = layout.room_at(&room, pos.0);
        let target = players
            .iter()
            .filter(|(_, p, ..)| home.is_none_or(|h| layout.room_at(&room, p.0) == Some(h)))
            .map(|(_, p, ..)| (p.0.distance(pos.0), p.0))
            .filter(|(d, _)| *d <= stats.sight)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((dist, target_pos)) = target else {
            continue;
        };
        let damage = |players: &mut Query<
            (Entity, &Position, &mut Health, Has<Shielded>),
            (With<Player>, Without<Downed>, Without<BossBrain>),
        >,
                      center: Vec2,
                      radius: f32,
                      amount: f32,
                      already: &[Entity]|
         -> Vec<Entity> {
            let mut hit = Vec::new();
            for (entity, p, mut health, shielded) in players.iter_mut() {
                if already.contains(&entity) || p.0.distance(center) > radius + HERO_HALF_SIZE {
                    continue;
                }
                let factor = if shielded { balance().abilities.bulwark_damage_factor } else { 1.0 };
                health.current -= amount * factor * ease;
                hit.push(entity);
            }
            hit
        };
        match brain.act {
            Act::Windup(mv) => {
                if brain.timer > 0.0 {
                    continue;
                }
                match mv {
                    BossMove::Slam => {
                        damage(&mut players, pos.0, SLAM_RADIUS, SLAM_DAMAGE, &[]);
                        commands.trigger(ScreenShake(0.8));
                        strikes.0 = strikes.0.wrapping_add(1);
                        brain.timer = RECOVER_TIME;
                        brain.act = Act::Recover;
                    }
                    BossMove::Nova => {
                        damage(&mut players, pos.0, NOVA_RADIUS, NOVA_DAMAGE, &[]);
                        commands.trigger(ScreenShake(0.6));
                        strikes.0 = strikes.0.wrapping_add(1);
                        brain.timer = RECOVER_TIME;
                        brain.act = Act::Recover;
                    }
                    BossMove::Howl => {
                        let (_, minion, base) = waves(kind);
                        // Le chef de meute rameute plus de loups quand il est blessé, sans dépasser la taille de la meute.
                        let wanted = if kind == MonsterKind::Warg && enraged { base + 1 } else { base };
                        let alive = minions.iter().filter(|m| m.0 == minion).count();
                        let count = if kind == MonsterKind::Warg { wanted.min(MAX_PACK.saturating_sub(alive)) } else { wanted };
                        for k in 0..count {
                            // Les renforts surgissent en cercle autour du boss, hors des murs.
                            let around = rng.next_f32() * std::f32::consts::TAU + k as f32;
                            let at = pos.0 + Vec2::from_angle(around) * (stats.half_size + 30.0);
                            let at = if room.overlaps_wall(at, minion.stats().half_size) {
                                pos.0
                            } else {
                                at
                            };
                            spawn_monster(&mut commands, minion, at);
                        }
                        commands.trigger(ScreenShake(0.5));
                        brain.timer = RECOVER_TIME;
                        brain.act = Act::Recover;
                    }
                }
            }
            Act::Recover => {
                if brain.timer <= 0.0 {
                    brain.act = Act::Hunt;
                }
            }
            Act::Hunt => {
                // Les renforts d'abord: à chaque seuil de vie franchi, un cri.
                let (thresholds, ..) = waves(kind);
                if brain.waves < thresholds.len() && fraction <= thresholds[brain.waves] {
                    brain.waves += 1;
                    brain.act = Act::Windup(BossMove::Howl);
                    brain.timer = HOWL_TIME;
                    commands.trigger(BossAlert {
                            kind: BossMove::Howl,
                            at: pos.0,
                            radius: 0.0,
                            delay: HOWL_TIME,
                        });
                    continue;
                }
                if brain.special_in <= 0.0 && dist > stats.half_size + HERO_HALF_SIZE + 4.0
                    || brain.special_in <= -3.0
                {
                    let mv = special(kind, enraged);
                    let (windup, radius) = match mv {
                        BossMove::Slam => (SLAM_WINDUP, SLAM_RADIUS),
                        BossMove::Nova => (NOVA_WINDUP, NOVA_RADIUS),
                        _ => (HOWL_TIME, 0.0),
                    };
                    let at = pos.0;
                    brain.act = Act::Windup(mv);
                    brain.timer = windup;
                    brain.special_in = special_interval(kind, enraged);
                    commands.trigger(BossAlert {
                            kind: mv,
                            at,
                            radius,
                            delay: windup,
                        });
                    continue;
                }
                let reach = stats.half_size + HERO_HALF_SIZE + balance().rules.monster_reach_slack;
                if dist <= reach {
                    if cooldown.0 == 0.0 {
                        // Le corps à corps frappe le héros le plus proche.
                        if let Some((_, _, mut health, shielded)) = players
                            .iter_mut()
                            .filter(|(_, p, ..)| p.0.distance(pos.0) <= reach)
                            .min_by(|a, b| a.1.0.distance(pos.0).total_cmp(&b.1.0.distance(pos.0)))
                        {
                            let factor = if shielded { balance().abilities.bulwark_damage_factor } else { 1.0 };
                            health.current -= stats.damage * factor * ease;
                            cooldown.0 = stats.cooldown;
                            strikes.0 = strikes.0.wrapping_add(1);
                        }
                    }
                    continue;
                }
                let dir = (target_pos - pos.0).normalize_or_zero();
                pos.0 = room.move_and_collide_solids(pos.0, dir * speed * dt, stats.half_size, &[]);
            }
        }
    }
}
