//! Serveur de Castle: simulation autoritaire, sans fenêtre.
//!
//! Bibliothèque: le programme `castle_server` (ordinateur) la lance dans son propre processus; sur iOS, où une app ne
//! peut pas lancer d'autre programme, le client la fait tourner sur un fil à part (`run_in_thread`).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;
use std::time::SystemTime;

use bevy::{app::ScheduleRunnerPlugin, prelude::*, state::app::StatesPlugin};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetServer, RepliconRenetPlugins,
    netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
    renet::ConnectionConfig,
};
mod outdoor;

use castle_shared::save::{self, GameMode, SaveGame};
use castle_shared::*;

/// Compagnons: distance de détection des ennemis, portée de tir, retour auprès du chef.
const AI_AGGRO: f32 = 110.0;
/// Un itinéraire est recalculé quand le but a bougé de plus que cela, ou après ce délai.
const ROUTE_GOAL_DRIFT: f32 = 24.0;
const ROUTE_MAX_AGE: f32 = 0.6;
const AI_SHOT_REACH: f32 = 150.0;
const AI_LEASH: f32 = 220.0;
/// Les compagnons n'engagent que les ennemis à cette distance du héros dirigé.
const AI_GROUP_RADIUS: f32 = 150.0;
/// Les compagnons soutiennent le héros dirigé sans le remplacer: leurs coups font un peu moins mal et sont un peu plus
/// lents, et ils s'attaquent aux monstres comme aux générateurs.
const AI_DAMAGE_FACTOR: f32 = 0.6;
const AI_COOLDOWN_FACTOR: f32 = 1.25;
/// Les corps se bloquent: marge ajoutée à la portée de frappe d'un monstre.
const MONSTER_REACH_SLACK: f32 = 6.0;

/// Distance à laquelle un allié relève un héros à terre, et temps nécessaire.
const REVIVE_RANGE: f32 = 20.0;
const REVIVE_TIME: f32 = 1.5;

/// Une scène de dialogue ne démarre que si aucun monstre n'est à cette distance d'un héros.
const DIALOGUE_SAFE_RADIUS: f32 = 170.0;

const GENERATOR_HEALTH: f32 = 40.0;
/// Un générateur ne produit que si un héros est à moins de cette distance.
const GENERATOR_ACTIVATION: f32 = 220.0;
const GENERATOR_INTERVAL: f32 = 2.2;
/// Nombre maximal de monstres vivants dans la salle.
const MAX_MONSTERS: usize = 28;
/// La sortie s'active quand tous les héros debout sont dans ce rayon, pendant ce délai.
const EXIT_RADIUS: f32 = 48.0;
const EXIT_DELAY: f32 = 1.5;

/// Rempart: rayon d'effet, durée et part des dégâts qui passe encore.
const BULWARK_RADIUS: f32 = 90.0;
const BULWARK_DURATION: f32 = 6.0;
const BULWARK_DAMAGE_FACTOR: f32 = 0.5;
/// Tempête Arcane: décalage devant la sorcière, rayon, dégâts.
const ARCANE_OFFSET: f32 = 56.0;
const ARCANE_RADIUS: f32 = 48.0;
const ARCANE_DAMAGE: f32 = 35.0;
/// Pluie de Flèches: décalage, rayon, dégâts par impulsion, intervalle et durée.
const RAIN_OFFSET: f32 = 80.0;
const RAIN_RADIUS: f32 = 56.0;
const RAIN_DAMAGE: f32 = 6.0;
const RAIN_INTERVAL: f32 = 0.25;
const RAIN_DURATION: f32 = 2.0;
/// Tourelle: durée de vie, portée, cadence, dégâts et vitesse des tirs.
const TURRET_LIFETIME: f32 = 15.0;
const TURRET_RANGE: f32 = 110.0;
const TURRET_COOLDOWN: f32 = 0.6;
const TURRET_DAMAGE: f32 = 7.0;
const TURRET_SHOT_SPEED: f32 = 220.0;

/// Pièges: dégâts, délai entre deux déclenchements, rayon de déclenchement, temps de désamorçage.
const TRAP_DAMAGE: f32 = 12.0;
const TRAP_COOLDOWN: f32 = 1.0;
const TRAP_TRIGGER_RADIUS: f32 = 8.0;
const TRAP_DISARM_TIME: f32 = 0.6;
/// Enchantement de la porte royale: vitesse à laquelle il repousse un héros trop proche.
const WARD_PUSH_SPEED: f32 = 130.0;
/// Une flèche du Rôdeur qui passe à moins de cette distance de la cible fixée la touche.
const TARGET_HIT_RADIUS: f32 = 14.0;

/// Un piège dort, pointes rentrées, tant que personne n'est proche: un héros à moins de `TRAP_WAKE_RADIUS` le réveille
/// (pointes sorties), et il se rendort `TRAP_SLEEP_DELAY` secondes après le départ du dernier héros.
const TRAP_WAKE_RADIUS: f32 = 56.0;
const TRAP_SLEEP_DELAY: f32 = 1.2;

/// Temps écoulé depuis qu'un héros était proche d'un piège réveillé.
#[derive(Component, Default)]
struct TrapIdle(f32);

/// Piège désarmé pour de bon par le Rôdeur: il ne se réveille plus.
#[derive(Component)]
struct Disarmed;
/// Une porte à clé s'ouvre quand un héros muni d'une clé s'en approche à moins de cette distance.
const DOOR_OPEN_RANGE: f32 = 16.0;

/// Dialogues: pause minimale entre deux répliques, délai de vote, durée maximale sans activité.
const DIALOGUE_ADVANCE_COOLDOWN: f32 = 0.25;
const DIALOGUE_VOTE_TIMEOUT: f32 = 15.0;
const DIALOGUE_IDLE_TIMEOUT: f32 = 90.0;

const PICKUP_RANGE: f32 = 10.0;
const FOOD_HEAL: f32 = 25.0;

/// Potion de régénération: durée de l'effet et part des PV maximum rendue chaque seconde, pour tous les héros de l'équipe.
const POTION_DURATION: f32 = 8.0;
const POTION_HEAL_PER_SECOND: f32 = 0.06;
/// Nombre de potions posées dans un niveau, et chance qu'un monstre en laisse une en mourant.
const POTION_DROP_CHANCE: f32 = 0.04;

/// Régénération en cours sur un héros: secondes restantes.
#[derive(Component)]
struct Regen(f32);

fn potions_in_level(level: u32) -> u32 {
    2 + level.min(9) / 3
}

/// Monstres posés d'avance dans chaque salle de combat, en plus de ceux des générateurs.
fn monsters_per_room(level: u32) -> u32 {
    (1 + level / 2).min(6)
}

/// Les premiers niveaux sont plus doux: les monstres vont moins vite, voient moins loin et les générateurs produisent
/// moins souvent. Le facteur monte de niveau en niveau jusqu'à 1.
fn ease(level: u32) -> f32 {
    (0.7 + 0.1 * level.saturating_sub(1) as f32).min(1.0)
}

/// Direction demandée par le joueur, jamais répliquée.
#[derive(Component, Default)]
struct MoveDir(Vec2);

#[derive(Component, Default)]
struct Attacking(bool);

/// Temps restant avant la prochaine attaque.
#[derive(Component, Default)]
struct Cooldown(f32);

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Component)]
struct Damage(f32);

#[derive(Component)]
struct Lifetime(f32);

/// Progression de la réanimation d'un héros à terre, en secondes.
#[derive(Component, Default)]
struct Revive(f32);

/// État de la touche du pouvoir spécial; `queued` devient vrai sur chaque nouvel appui.
#[derive(Component, Default)]
struct SpecialInput {
    held: bool,
    queued: bool,
}

#[derive(Component, Default)]
struct SpecialCooldown(f32);

#[derive(Component)]
struct ShieldTimer(f32);

/// Durée restante d'une zone et rythme de ses dégâts.
#[derive(Component)]
struct ZoneLife(f32);

#[derive(Component)]
struct ZoneDamage {
    damage: f32,
    interval: f32,
    timer: f32,
}

#[derive(Component)]
struct TurretLife(f32);

#[derive(Component)]
struct TurretOwner(Entity);

/// Héros actuellement dirigé par un joueur (les autres de son équipe suivent et se défendent seuls).
#[derive(Component)]
struct Controlled;

/// Sur l'entité de connexion d'un client: le héros qu'il dirige, et son équipe.
#[derive(Component)]
struct ActiveHero(Entity);

#[derive(Component)]
struct ClientParty(u32);

#[derive(Resource, Default)]
struct NextParty(u32);

/// Itinéraire d'un compagnon vers sa cible: points de passage calculés par `Room::find_path`.
#[derive(Component, Default)]
struct Route {
    goal: Vec2,
    waypoints: Vec<Vec2>,
    /// Temps depuis le dernier calcul; l'itinéraire est refait régulièrement car le chef bouge.
    age: f32,
}

/// Coup en préparation: l'effet (dégâts, flèche partie) a lieu à l'instant de la frappe de l'animation d'arme.
#[derive(Component)]
struct PendingAttack {
    hero: Entity,
    timer: f32,
}

/// Délai entre le début du geste et l'instant de la frappe, en secondes.
const ATTACK_WINDUP: f32 = 0.2;

/// Touche d'action maintenue par le joueur.
#[derive(Component, Default)]
struct Interacting(bool);

/// Temps pendant lequel l'action est maintenue sur un objet, en secondes.
#[derive(Component, Default)]
struct Hold(f32);

/// Obstacle rectangulaire (demi-dimensions), par exemple une porte fermée.
#[derive(Component)]
struct Solid(Vec2);

/// Délai restant avant qu'un piège puisse se redéclencher.
#[derive(Component, Default)]
struct TrapCooldown(f32);

/// Demi-côté de la boîte qui reçoit les coups (monstres et générateurs).
#[derive(Component)]
struct Hurtbox(f32);

/// Temps avant la prochaine apparition d'un monstre.
#[derive(Component)]
struct SpawnTimer(f32);

/// Scène de dialogue en cours. Le serveur ne la joue pas: il la déclenche, fait avancer tout le monde
/// en même temps et compte les votes. Les textes vivent côté client.
#[derive(Resource, Default)]
struct DialogueSession {
    active: Option<String>,
    advance_cooldown: f32,
    votes: HashMap<Entity, u32>,
    vote_timer: f32,
    idle: f32,
}

/// Déclenche un dialogue la première fois qu'un héros reste dans la zone.
#[derive(Component)]
struct DialogueTrigger {
    node: &'static str,
    radius: f32,
    /// Temps de présence nécessaire avant le déclenchement.
    delay: f32,
    fired: bool,
}

/// Petit générateur pseudo-aléatoire (xorshift), suffisant pour le butin et les variations.
#[derive(Resource)]
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Demande d'arrêt d'un serveur lancé sur un fil: il s'arrête à la frame suivante.
#[derive(Resource, Clone, Default)]
pub struct StopFlag(pub Arc<AtomicBool>);

impl StopFlag {
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn stop_when_asked(flag: Res<StopFlag>, mut exit: MessageWriter<AppExit>) {
    if flag.0.load(Ordering::Relaxed) {
        exit.write(AppExit::Success);
    }
}

/// Serveur lancé dans le processus d'un client: le drapeau l'arrête, et le fil se termine avec lui.
pub struct ThreadServer {
    pub stop: StopFlag,
    pub thread: std::thread::JoinHandle<()>,
}

/// Lance le serveur sur un fil du processus courant. Il partage les journaux du client (pas de second `LogPlugin`).
pub fn run_in_thread(options: Options) -> std::io::Result<ThreadServer> {
    let stop = StopFlag::default();
    let flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("castle-server".into())
        .spawn(move || run(options, Some(flag)))?;
    Ok(ThreadServer { stop, thread })
}

/// Fait tourner le serveur jusqu'à son arrêt. Sans drapeau (programme autonome), il installe ses propres journaux.
pub fn run(options: Options, stop: Option<StopFlag>) {
    let embedded = stop.is_some();
    let campaign = options.open_campaign();
    info!("campaign '{}' ({:?}), level {}", campaign.0.name, campaign.0.mode, campaign.0.level);
    let mut app = App::new();
    app.add_plugins((
        // Sans cadence, la boucle d'un serveur sans fenêtre tourne à vide et occupe un cœur entier.
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(8))),
        StatesPlugin,
    ));
    if !embedded {
        app.add_plugins(bevy::log::LogPlugin::default());
    }
    if let Some(flag) = stop {
        app.insert_resource(flag).add_systems(First, stop_when_asked);
    }
    app
        .add_plugins((
            RepliconPlugins,
            RepliconRenetPlugins,
            CastleSharedPlugin,
        ))
        .insert_resource(Rng(campaign.0.entry_rng))
        .insert_resource(campaign)
        .insert_resource(options)
        .init_resource::<DialogueSession>()
        .init_resource::<Paused>()
        .init_resource::<NextParty>()
        .insert_resource(LevelRegion(Region::Crypts))
        .init_resource::<LevelStats>()
        .init_resource::<Transition>()
        .add_systems(Startup, (start_server, start_first_expedition))
        .add_systems(
            FixedUpdate,
            (
                // Actions des héros, puis effets, puis monde et dialogues.
                (
                    hold_heroes,
                    move_players,
                    player_attacks,
                    resolve_attacks,
                    auto_switch,
                    hero_ai,
                    leash_followers,
                    regen_mana,
                    player_specials,
                    tick_shields,
                    run_zones,
                    run_turrets,
                    move_projectiles,
                )
                    .chain(),
                (
                    run_generators.run_if(not_in_transition),
                    run_dialogue_triggers,
                    tick_dialogue,
                    run_interactions,
                    run_ward,
                    run_traps,
                    open_locked_doors,
                    monster_ai.run_if(not_in_transition),
                    reap_dead,
                    collect_pickups,
                    run_regen,
                    collect_relics,
                    rescue_captives,
                    update_mechanism_quests,
                    open_exit,
                    down_players,
                    revive_players,
                    check_wipe,
                    track_stats,
                    run_exit,
                )
                    .chain(),
            )
                .chain()
                .run_if(not_paused),
        )
        .add_observer(spawn_party)
        .add_observer(despawn_party)
        .add_observer(switch_hero)
        .add_observer(set_pause)
        .add_observer(apply_input)
        .add_observer(continue_level)
        .add_observer(dialogue_advance)
        .add_observer(dialogue_skip)
        .add_observer(dialogue_vote)
        .add_observer(dialogue_finished)
        .run();
}

/// Réglages de la partie (en ligne de commande: `--port N`, `--mode solo|coop`, `--save NOM`).
#[derive(Resource, Clone)]
pub struct Options {
    pub port: u16,
    pub mode: GameMode,
    pub save: String,
}

impl Options {
    pub fn from_args() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1));
        Self {
            port: value("--port").and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT),
            mode: value("--mode").and_then(|m| GameMode::from_arg(m)).unwrap_or(GameMode::Coop),
            save: value("--save").cloned().unwrap_or_else(|| "Castle".into()),
        }
    }

    /// Reprend la partie de ce nom, ou en commence une nouvelle.
    fn open_campaign(&self) -> Campaign {
        let mut save = match save::load(&self.save) {
            Some(save) => save,
            None => {
                let seed = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(1);
                let fresh = SaveGame::new(&self.save, self.mode, seed);
                if let Err(err) = save::write(&fresh) {
                    warn!("cannot write save: {err}");
                }
                fresh
            }
        };
        // L'éditeur de niveaux lance le jeu sur un niveau précis pour l'essayer.
        if let Some(level) = std::env::var("CASTLE_START_LEVEL").ok().and_then(|v| v.parse::<u32>().ok()) {
            save.level = level.clamp(1, LEVEL_COUNT);
        }
        Campaign(save)
    }
}

/// La simulation est en pause (menu de pause d'une partie solo).
#[derive(Resource, Default)]
struct Paused(bool);

fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}

/// Région du niveau en cours (elle décide des monstres que les générateurs produisent).
#[derive(Resource)]
struct LevelRegion(Region);

/// Ce que l'équipe a fait dans le niveau en cours; remis à zéro à chaque nouveau niveau.
#[derive(Resource, Default)]
struct LevelStats {
    seconds: f32,
    kills: u32,
    generators: u32,
    damage_taken: f32,
    downed: u32,
    gold: u32,
    keys: u32,
    food: u32,
    potions: u32,
    pieces: u32,
}

/// Temps écoulé depuis que l'équipe est entrée dans le portail: les clients montrent le bilan, la partie est figée
/// (monstres, générateurs et héros) et le niveau suivant attend que chaque joueur ait validé.
#[derive(Resource, Default)]
struct Transition(Option<f32>);

/// Un joueur absent ne bloque pas les autres indéfiniment: au bout de ce délai, le niveau suivant démarre.
const SUMMARY_TIMEOUT: f32 = 120.0;

/// Sur un client: il a validé le bilan.
#[derive(Component)]
struct Ready;

fn continue_level(input: On<FromClient<ContinueLevel>>, transition: Res<Transition>, mut commands: Commands) {
    if transition.0.is_none() {
        return;
    }
    if let Some(client) = input.client_id.entity() {
        commands.entity(client).try_insert(Ready);
    }
}

fn not_in_transition(transition: Res<Transition>) -> bool {
    transition.0.is_none()
}

/// Les héros sont retenus pendant le bilan de fin de niveau et pendant les dialogues: une scène ne démarre qu'à l'abri des
/// ennemis, et on ne la laisse pas passer inaperçue (on peut la passer en maintenant Entrée). Une scène d'introduction qui
/// n'a pas encore démarré (elle attend un court délai à l'arrivée) retient déjà les héros. Le client n'envoie plus d'ordres,
/// mais le dernier reste: on remet tout à zéro.
fn hold_heroes(
    transition: Res<Transition>,
    session: Res<DialogueSession>,
    triggers: Query<&DialogueTrigger>,
    monsters: Query<&Position, (With<Monster>, Without<Player>)>,
    mut heroes: Query<(&Position, &mut MoveDir, &mut Attacking, &mut SpecialInput, &mut Interacting), With<Player>>,
) {
    // Une scène qui n'a pas encore démarré ne retient les héros que si elle peut le faire tout de suite: tant qu'un monstre
    // est à proximité elle attend, et les héros doivent pouvoir se battre (sinon ils resteraient figés sous les crocs).
    let calm = !monsters.iter().any(|m| heroes.iter().any(|(p, ..)| p.0.distance(m.0) <= DIALOGUE_SAFE_RADIUS));
    let scene = session.active.is_some() || (calm && triggers.iter().any(|t| t.node.ends_with("intro") && !t.fired));
    if transition.0.is_none() && !scene {
        return;
    }
    for (_, mut dir, mut attacking, mut special, mut interacting) in &mut heroes {
        dir.0 = Vec2::ZERO;
        attacking.0 = false;
        special.queued = false;
        interacting.0 = false;
    }
}

/// Compte le temps passé, les points de vie perdus par les héros et les héros tombés.
fn track_stats(
    time: Res<Time>,
    transition: Res<Transition>,
    mut stats: ResMut<LevelStats>,
    mut last: Local<HashMap<Entity, f32>>,
    heroes: Query<(Entity, &Health), With<Player>>,
    fallen: Query<(), (With<Player>, Added<Downed>)>,
) {
    for (hero, health) in &heroes {
        // Seules les baisses comptent (les soins et les remises en forme les font monter).
        let before = last.insert(hero, health.current).unwrap_or(health.current);
        if transition.0.is_none() && before > health.current {
            stats.damage_taken += before - health.current;
        }
    }
    if transition.0.is_none() {
        stats.seconds += time.delta_secs();
        stats.downed += fallen.iter().count() as u32;
    }
}

/// Le menu de pause fige la partie, mais seulement en solo: en coopératif les autres joueurs continuent.
fn set_pause(request: On<FromClient<SetPause>>, options: Res<Options>, mut paused: ResMut<Paused>) {
    paused.0 = options.mode == GameMode::Solo && request.0;
}

/// La partie en cours: le niveau où l'équipe est entrée, de quoi le régénérer et ce qu'elle portait.
#[derive(Resource)]
struct Campaign(SaveGame);

impl Campaign {
    fn write(&self) {
        if let Err(err) = save::write(&self.0) {
            warn!("cannot write save: {err}");
        }
    }
}

fn start_server(mut commands: Commands, channels: Res<RepliconChannels>, options: Res<Options>) -> Result {
    let port = options.port;
    // Solo: le serveur n'écoute que cette machine et n'accepte qu'un joueur.
    let (bind, max_clients) = match options.mode {
        GameMode::Solo => (Ipv4Addr::LOCALHOST, 1),
        GameMode::Coop => (Ipv4Addr::UNSPECIFIED, MAX_PLAYERS),
    };

    let server = RenetServer::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    });

    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let socket = UdpSocket::bind((bind, port))?;
    let transport = NetcodeServerTransport::new(
        ServerConfig {
            current_time,
            max_clients,
            protocol_id: PROTOCOL_ID,
            authentication: ServerAuthentication::Unsecure,
            public_addresses: Default::default(),
        },
        socket,
    )?;

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("Castle server listening on {bind}:{port}");
    Ok(())
}

/// Un client authentifié reçoit une équipe de quatre héros; le Chevalier est dirigé au départ.
fn spawn_party(
    add: On<Add, AuthorizedClient>,
    mut commands: Commands,
    layout: Res<Layout>,
    room: Res<Room>,
    campaign: Res<Campaign>,
    mut next_party: ResMut<NextParty>,
    heroes: Query<(), With<Player>>,
) {
    let party = next_party.0;
    next_party.0 += 1;
    info!("player joined ({:?}), party {party}", add.entity);
    let first_slot = heroes.iter().count();
    // Les équipes arrivent dans le même ordre qu'à la sauvegarde: la n-ième retrouve sa bourse.
    let (gold, keys) = campaign.0.purses.get(party as usize).copied().unwrap_or_default();
    let mut active = None;
    for (i, class) in HeroClass::ALL.into_iter().enumerate() {
        let stats = class.stats();
        let hero = commands
            .spawn((
                Replicated,
                Player,
                PartyId(party),
                class,
                Health::full(stats.max_hp as f32),
                Mana::full(stats.max_mana),
                Gold(if i == 0 { gold } else { 0 }),
                Keys(if i == 0 { keys } else { 0 }),
                Position(spawn_point(first_slot + i, &layout, &room)),
            ))
            .insert((
                MoveDir::default(),
                Facing(Vec2::X),
                Attacking::default(),
                Cooldown::default(),
                Revive::default(),
                SpecialInput::default(),
                SpecialCooldown::default(),
                Interacting::default(),
                Route::default(),
                Strikes::default(),
            ))
            .id();
        if i == 0 {
            commands.entity(hero).insert(Controlled);
            active = Some(hero);
        }
    }
    let hero = active.expect("une équipe a au moins un héros");
    commands.entity(add.entity).insert((ActiveHero(hero), ClientParty(party)));
    commands.server_trigger(ToClients {
        targets: SendTargets::Single(ClientId::Client(add.entity)),
        message: YouAre { hero, party },
    });
}

/// Quand un client part, son équipe disparaît avec lui.
fn despawn_party(
    remove: On<Remove, ConnectedClient>,
    mut commands: Commands,
    clients: Query<&ClientParty>,
    heroes: Query<(Entity, &PartyId), With<Player>>,
) {
    let Ok(party) = clients.get(remove.entity) else {
        return;
    };
    for (entity, id) in &heroes {
        if id.0 == party.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn apply_input(
    input: On<FromClient<PlayerInput>>,
    clients: Query<&ActiveHero>,
    mut heroes: Query<
        (&mut MoveDir, &mut Facing, &mut Attacking, &mut SpecialInput, &mut Interacting),
        (With<Controlled>, Without<Downed>),
    >,
) {
    let Some(client) = input.client_id.entity() else {
        return;
    };
    let Ok(active) = clients.get(client) else {
        return;
    };
    let Ok((mut dir, mut facing, mut attacking, mut special, mut interacting)) =
        heroes.get_mut(active.0)
    else {
        return;
    };
    // Ne jamais faire confiance au client: direction finie et bornée.
    dir.0 = if input.direction.is_finite() {
        input.direction.clamp_length_max(1.0)
    } else {
        Vec2::ZERO
    };
    if let Some(d) = dir.0.try_normalize() {
        facing.0 = d;
    }
    attacking.0 = input.attack;
    interacting.0 = input.interact;
    if input.special && !special.held {
        special.queued = true;
    }
    special.held = input.special;
}

/// Le joueur prend la main sur un autre héros de son équipe; l'ancien redevient un compagnon.
fn switch_hero(
    choice: On<FromClient<SwitchHero>>,
    mut commands: Commands,
    mut clients: Query<(&mut ActiveHero, &ClientParty)>,
    heroes: Query<(Entity, &PartyId, &HeroClass, Has<Downed>), With<Player>>,
) {
    let Some(client) = choice.client_id.entity() else {
        return;
    };
    let Ok((mut active, party)) = clients.get_mut(client) else {
        return;
    };
    let target = heroes
        .iter()
        .find(|(_, id, class, downed)| id.0 == party.0 && **class == choice.0 && !downed)
        .map(|(entity, ..)| entity);
    let Some(target) = target.filter(|t| *t != active.0) else {
        return;
    };
    hand_over(&mut commands, active.0, target);
    active.0 = target;
    commands.server_trigger(ToClients {
        targets: SendTargets::Single(ClientId::Client(client)),
        message: YouAre { hero: target, party: party.0 },
    });
}

/// Retire le contrôle à `from` (qui s'arrête) et le donne à `to`.
fn hand_over(commands: &mut Commands, from: Entity, to: Entity) {
    commands.entity(from).remove::<Controlled>().insert((
        MoveDir::default(),
        Attacking::default(),
        Interacting::default(),
        SpecialInput::default(),
    ));
    commands.entity(to).insert(Controlled);
}

/// Si le héros dirigé tombe, le contrôle passe à un autre héros debout de l'équipe.
fn auto_switch(
    mut commands: Commands,
    mut clients: Query<(Entity, &mut ActiveHero, &ClientParty)>,
    heroes: Query<(Entity, &PartyId, Has<Downed>), With<Player>>,
) {
    for (client, mut active, party) in &mut clients {
        let down = heroes.get(active.0).map(|(_, _, downed)| downed).unwrap_or(true);
        if !down {
            continue;
        }
        let Some((next, ..)) = heroes
            .iter()
            .find(|(_, id, downed)| id.0 == party.0 && !downed)
        else {
            continue;
        };
        hand_over(&mut commands, active.0, next);
        active.0 = next;
        commands.server_trigger(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: YouAre { hero: next, party: party.0 },
        });
    }
}

fn move_players(
    time: Res<Time>,
    room: Res<Room>,
    doors: Query<(&Position, &Solid), Without<Player>>,
    bodies: Query<(&Position, &Hurtbox), Without<Player>>,
    mut players: Query<(&mut Position, &MoveDir, &HeroClass), (With<Player>, Without<Downed>)>,
) {
    // Portes, monstres et générateurs arrêtent les héros; les héros se traversent entre eux.
    let solids: Vec<_> = doors
        .iter()
        .map(|(p, s)| (p.0, s.0))
        .chain(bodies.iter().map(|(p, h)| (p.0, Vec2::splat(h.0))))
        .collect();
    for (mut pos, dir, class) in &mut players {
        if dir.0 == Vec2::ZERO {
            continue;
        }
        let delta = dir.0 * class.stats().speed * time.delta_secs();
        pos.0 = room.move_and_collide_solids(pos.0, delta, HERO_HALF_SIZE, &solids);
    }
}

fn player_attacks(
    mut commands: Commands,
    time: Res<Time>,
    mut players: Query<
        (Entity, &Attacking, &HeroClass, &mut Cooldown, &mut Strikes, Has<Controlled>),
        (With<Player>, Without<Downed>),
    >,
) {
    for (entity, attacking, class, mut cooldown, mut strikes, controlled) in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
        if !attacking.0 || cooldown.0 > 0.0 {
            continue;
        }
        cooldown.0 = class.stats().attack.cooldown() * if controlled { 1.0 } else { AI_COOLDOWN_FACTOR };
        // Le client joue le geste; l'effet suit à l'instant de la frappe (`resolve_attacks`).
        strikes.0 = strikes.0.wrapping_add(1);
        commands.spawn(PendingAttack { hero: entity, timer: ATTACK_WINDUP });
    }
}

/// Applique l'effet d'une attaque une fois le geste arrivé à sa frappe: coup au corps à corps ou tir.
fn resolve_attacks(
    mut commands: Commands,
    time: Res<Time>,
    mut pending: Query<(Entity, &mut PendingAttack)>,
    heroes: Query<(&Position, &Facing, &HeroClass, Has<Controlled>), (With<Player>, Without<Downed>)>,
    mut targets: Query<(&Position, &mut Health, &Hurtbox), Without<Player>>,
) {
    for (entity, mut attack) in &mut pending {
        attack.timer -= time.delta_secs();
        if attack.timer > 0.0 {
            continue;
        }
        commands.entity(entity).despawn();
        // Le héros peut être tombé ou parti entre-temps: le coup est alors perdu.
        let Ok((pos, facing, class, controlled)) = heroes.get(attack.hero) else {
            continue;
        };
        let power = if controlled { 1.0 } else { AI_DAMAGE_FACTOR };
        match class.stats().attack {
            Attack::Melee { range, damage, .. } => {
                for (target_pos, mut health, hurtbox) in &mut targets {
                    let to = target_pos.0 - pos.0;
                    let dist = to.length();
                    // Cône d'environ 150° devant le héros, ou au contact.
                    let in_front = dist < 8.0 || to.normalize_or_zero().dot(facing.0) > 0.25;
                    if dist <= range + hurtbox.0 && in_front {
                        health.current -= damage * power;
                    }
                }
                commands.server_trigger(ToClients {
                    targets: SendTargets::All,
                    message: AttackFx { pos: pos.0, dir: facing.0, class: *class },
                });
            }
            Attack::Shot { speed, damage, lifetime, .. } => {
                commands.spawn((
                    Replicated,
                    Projectile { class: *class, dir: facing.0 },
                    Position(pos.0 + facing.0 * 8.0),
                    Velocity(facing.0 * speed),
                    Damage(damage * power),
                    Lifetime(lifetime),
                ));
            }
        }
    }
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    room: Res<Room>,
    mut projectiles: Query<
        (Entity, &mut Position, &Velocity, &Damage, &mut Lifetime, &Projectile),
        With<Projectile>,
    >,
    mut targets: Query<
        (&Position, &mut Health, &Hurtbox),
        (Without<Player>, Without<Projectile>),
    >,
    mut panels: Query<(&Position, &mut Interactable), Without<Projectile>>,
    portcullises: Query<Entity, With<Portcullis>>,
) {
    for (entity, mut pos, velocity, damage, mut lifetime, shot) in &mut projectiles {
        let next = pos.0 + velocity.0 * time.delta_secs();
        lifetime.0 -= time.delta_secs();
        // Une flèche du Rôdeur qui touche la cible garnie lève la herse. Le test précède celui du mur: la cible est
        // dans le mur, la flèche s'y arrête.
        if shot.class == HeroClass::Ranger {
            let hit = panels.iter_mut().find(|(p, t)| {
                t.kind == InteractKind::Target && t.mounted && !t.done && p.0.distance(next) <= TARGET_HIT_RADIUS
            });
            if let Some((_, mut target)) = hit {
                target.done = true;
                target.progress = 1.0;
                commands.entity(entity).despawn();
                for herse in &portcullises {
                    commands.entity(herse).despawn();
                }
                commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(0.7) });
                info!("target hit, portcullis raised");
                continue;
            }
        }
        if lifetime.0 <= 0.0 || room.overlaps_wall(next, PROJECTILE_RADIUS) {
            commands.entity(entity).despawn();
            continue;
        }
        pos.0 = next;
        let hit = targets
            .iter_mut()
            .find(|(t, _, hurtbox)| t.0.distance(next) <= hurtbox.0 + PROJECTILE_RADIUS);
        if let Some((_, mut health, _)) = hit {
            health.current -= damage.0;
            commands.entity(entity).despawn();
        }
    }
}

/// Les générateurs produisent des monstres tant qu'un héros est proche.
fn run_generators(
    mut commands: Commands,
    time: Res<Time>,
    room: Res<Room>,
    mut rng: ResMut<Rng>,
    mut generators: Query<(&Position, &mut SpawnTimer), With<Generator>>,
    players: Query<&Position, (With<Player>, Without<Downed>, Without<Generator>)>,
    monsters: Query<(), With<Monster>>,
    level: Res<Level>,
    region: Res<LevelRegion>,
) {
    let mut alive = monsters.iter().count();
    for (pos, mut timer) in &mut generators {
        let nearest = players
            .iter()
            .map(|p| p.0)
            .min_by(|a, b| a.distance(pos.0).total_cmp(&b.distance(pos.0)));
        let Some(target) = nearest.filter(|t| t.distance(pos.0) <= GENERATOR_ACTIVATION) else {
            continue;
        };
        timer.0 -= time.delta_secs();
        if timer.0 > 0.0 || alive >= MAX_MONSTERS {
            continue;
        }
        timer.0 = GENERATOR_INTERVAL / ease(level.0);
        let kind = MonsterKind::pick_in(region.0, level.0, rng.next_f32());
        // Le monstre sort du côté du héros, sauf si un mur gêne.
        let out = pos.0 + (target - pos.0).normalize_or_zero() * (GENERATOR_HALF_SIZE + 6.0);
        let at = if room.overlaps_wall(out, kind.stats().half_size) { pos.0 } else { out };
        spawn_monster(&mut commands, kind, at);
        alive += 1;
    }
}

fn monster_ai(
    time: Res<Time>,
    room: Res<Room>,
    layout: Res<Layout>,
    level: Res<Level>,
    doors: Query<(&Position, &Solid), (Without<Monster>, Without<Player>)>,
    generators: Query<(&Position, &Hurtbox), (With<Generator>, Without<Monster>, Without<Player>)>,
    mut monsters: Query<
        (Entity, &mut Position, &mut Cooldown, &Monster, &mut Strikes),
        (Without<Player>, Without<Solid>, Without<Generator>),
    >,
    mut players: Query<
        (&Position, &mut Health, Has<Shielded>),
        (With<Player>, Without<Downed>, Without<Monster>),
    >,
) {
    let dt = time.delta_secs();
    let ease = ease(level.0);
    // Obstacles fixes pour tous les monstres, puis les corps des monstres entre eux.
    let fixed: Vec<(Vec2, Vec2)> = doors
        .iter()
        .map(|(p, s)| (p.0, s.0))
        .chain(generators.iter().map(|(p, h)| (p.0, Vec2::splat(h.0))))
        .chain(players.iter().map(|(p, ..)| (p.0, Vec2::splat(HERO_HALF_SIZE))))
        .collect();
    let bodies: Vec<(Entity, Vec2, f32)> = monsters
        .iter()
        .map(|(e, p, _, m, _)| (e, p.0, m.0.stats().half_size))
        .collect();
    for (entity, mut pos, mut cooldown, monster, mut strikes) in &mut monsters {
        let stats = monster.0.stats();
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        // Un monstre n'engage un héros que dans sa propre salle: depuis un couloir ou une autre salle, rien.
        let home = layout.room_at(&room, pos.0);
        let target = players
            .iter_mut()
            .map(|(p, h, shielded)| (p.0.distance(pos.0), p.0, h, shielded))
            .filter(|(d, ..)| *d <= stats.sight * ease)
            .filter(|(_, hero, ..)| home.is_none_or(|h| layout.room_at(&room, *hero) == Some(h)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((dist, target_pos, mut health, shielded)) = target else {
            continue;
        };
        if dist <= stats.half_size + HERO_HALF_SIZE + MONSTER_REACH_SLACK {
            if cooldown.0 == 0.0 {
                let factor = if shielded { BULWARK_DAMAGE_FACTOR } else { 1.0 };
                health.current -= stats.damage * factor * ease;
                cooldown.0 = stats.cooldown;
                // Le client joue le geste d'attaque à chaque coup.
                strikes.0 = strikes.0.wrapping_add(1);
            }
        } else {
            let dir = (target_pos - pos.0).normalize_or_zero();
            let mut solids = fixed.clone();
            solids.extend(
                bodies
                    .iter()
                    .filter(|(e, ..)| *e != entity)
                    .map(|(_, p, h)| (*p, Vec2::splat(*h))),
            );
            pos.0 = room.move_and_collide_solids(
                pos.0,
                dir * stats.speed * ease * dt,
                stats.half_size,
                &solids,
            );
        }
    }
}

fn spawn_pickup(commands: &mut Commands, kind: PickupKind, pos: Vec2, rng: &mut Rng) {
    let jitter = Vec2::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5) * 12.0;
    commands.spawn((Replicated, Pickup(kind), Position(pos + jitter)));
}

/// Retire ce qui est mort (monstres, générateurs) et laisse tomber du butin.
fn reap_dead(
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    dead: Query<
        (Entity, &Health, &Position, Has<Generator>, Has<Monster>),
        (With<Hurtbox>, Without<Player>),
    >,
    mut quests: Query<&mut Quest>,
    mut stats: ResMut<LevelStats>,
) {
    for (entity, health, pos, is_generator, is_monster) in &dead {
        if health.current > 0.0 {
            continue;
        }
        commands.entity(entity).despawn();
        if is_generator {
            stats.generators += 1;
        } else if is_monster {
            stats.kills += 1;
        }
        if is_monster || is_generator {
            for mut quest in &mut quests {
                if quest.kind == QuestKind::Hunt && !quest.is_done() {
                    quest.progress += 1;
                }
            }
        }
        if is_generator {
            commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(0.6) });
            for _ in 0..3 {
                spawn_pickup(&mut commands, PickupKind::Coin, pos.0, &mut rng);
            }
            if rng.next_f32() < 0.5 {
                spawn_pickup(&mut commands, PickupKind::Key, pos.0, &mut rng);
            }
        } else {
            let roll = rng.next_f32();
            let kind = if roll < 0.45 {
                Some(PickupKind::Coin)
            } else if roll < 0.55 {
                Some(PickupKind::Food)
            } else if roll < 0.58 {
                Some(PickupKind::Key)
            } else if roll < 0.58 + POTION_DROP_CHANCE {
                Some(PickupKind::Potion)
            } else {
                None
            };
            if let Some(kind) = kind {
                spawn_pickup(&mut commands, kind, pos.0, &mut rng);
            }
        }
    }
}

/// Ramasse les reliques de la quête de récupération.
fn collect_relics(
    mut commands: Commands,
    players: Query<&Position, (With<Player>, With<Controlled>, Without<Downed>)>,
    relics: Query<(Entity, &Position), (With<Relic>, Without<Player>)>,
    mut quests: Query<&mut Quest>,
) {
    for (entity, relic_pos) in &relics {
        if players.iter().any(|pos| pos.0.distance(relic_pos.0) <= PICKUP_RANGE) {
            commands.entity(entity).despawn();
            for mut quest in &mut quests {
                if quest.kind == QuestKind::Retrieve && !quest.is_done() {
                    quest.progress += 1;
                }
            }
        }
    }
}

/// Un héros qui reste près d'un prisonnier le délivre.
fn rescue_captives(
    mut commands: Commands,
    time: Res<Time>,
    players: Query<&Position, (With<Player>, With<Controlled>, Without<Downed>)>,
    mut captives: Query<(Entity, &Position, &mut RescueHold), (With<Captive>, Without<Player>)>,
    mut quests: Query<&mut Quest>,
    mut rng: ResMut<Rng>,
) {
    for (entity, captive_pos, mut hold) in &mut captives {
        if players.iter().any(|pos| pos.0.distance(captive_pos.0) <= RESCUE_RANGE) {
            hold.0 += time.delta_secs();
        } else {
            hold.0 = (hold.0 - time.delta_secs()).max(0.0);
        }
        if hold.0 >= RESCUE_TIME {
            commands.entity(entity).despawn();
            // Un prisonnier délivré remercie: quelques pièces et de quoi se soigner.
            for _ in 0..3 {
                spawn_pickup(&mut commands, PickupKind::Coin, captive_pos.0, &mut rng);
            }
            spawn_pickup(&mut commands, PickupKind::Food, captive_pos.0, &mut rng);
            for mut quest in &mut quests {
                if quest.kind == QuestKind::Rescue && !quest.is_done() {
                    quest.progress += 1;
                }
            }
        }
    }
}

/// L'objectif est rempli: le portail de sortie apparaît dans la salle de sortie, avec une secousse.
/// Un objectif propre à une classe (réparer le pont, libérer le pont-levis, briser le sceau) est rempli dès que son mécanisme
/// est accompli.
fn update_mechanism_quests(interactables: Query<&Interactable>, mut quests: Query<&mut Quest>) {
    for mut quest in &mut quests {
        let Some(kind) = quest.kind.mechanism() else {
            continue;
        };
        // Une porte poussée disparaît: l'objectif, une fois rempli, le reste.
        if quest.progress < quest.goal && interactables.iter().any(|i| i.kind == kind && i.done) {
            quest.progress = quest.goal;
        }
    }
}

/// La sortie s'ouvre quand l'objectif du niveau est rempli (ou, sans objectif, quand plus rien ne reste à combattre), que
/// tous les mécanismes du niveau sont accomplis et, dans un combat de boss, que la meute est vaincue.
fn open_exit(
    mut commands: Commands,
    level: Res<Level>,
    mut quests: Query<&mut Quest>,
    mut exits: Query<&mut LevelExit>,
    interactables: Query<&Interactable>,
    remaining: Query<(), Or<(With<Monster>, With<Generator>)>>,
) {
    // Plus rien à combattre: la chasse est réputée accomplie, pour ne jamais bloquer la sortie.
    if remaining.is_empty() {
        for mut quest in &mut quests {
            if quest.kind == QuestKind::Hunt && !quest.is_done() {
                quest.progress = quest.goal;
            }
        }
    }
    let quests_done = if quests.is_empty() { remaining.is_empty() } else { quests.iter().all(|quest| quest.is_done()) };
    let mechanisms_done = interactables.iter().all(|i| i.done);
    let fight_done = !is_boss_level(level.0) || remaining.is_empty();
    let done = quests_done && mechanisms_done && fight_done;
    for mut exit in &mut exits {
        if done && !exit.open {
            exit.open = true;
            commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(0.5) });
        }
    }
}

fn collect_pickups(
    mut commands: Commands,
    mut players: Query<
        (&Position, &mut Health, &mut Gold, &mut Keys, &PartyId),
        (With<Player>, With<Controlled>, Without<Downed>),
    >,
    team: Query<(Entity, &PartyId), With<Player>>,
    mut kit: Query<&mut PuzzleKit>,
    pickups: Query<(Entity, &Position, &Pickup), Without<Player>>,
    mut stats: ResMut<LevelStats>,
) {
    for (entity, pickup_pos, pickup) in &pickups {
        for (pos, mut health, mut gold, mut keys, party) in &mut players {
            if pos.0.distance(pickup_pos.0) > PICKUP_RANGE {
                continue;
            }
            match pickup.0 {
                PickupKind::Coin => {
                    gold.0 += 1;
                    stats.gold += 1;
                }
                PickupKind::Key => {
                    keys.0 += 1;
                    stats.keys += 1;
                }
                // Les pièces de l'énigme vont dans la réserve commune de l'équipe.
                PickupKind::Ingredient => {
                    if let Ok(mut stock) = kit.single_mut() {
                        stock.ingredients = stock.ingredients.saturating_add(1);
                        stats.pieces += 1;
                    }
                }
                PickupKind::GearPart => {
                    if let Ok(mut stock) = kit.single_mut() {
                        stock.gear = stock.gear.saturating_add(1);
                        stats.pieces += 1;
                    }
                }
                PickupKind::TargetDisc => {
                    if let Ok(mut stock) = kit.single_mut() {
                        stock.target = stock.target.saturating_add(1);
                        stats.pieces += 1;
                    }
                }
                PickupKind::Potion => {
                    stats.potions += 1;
                    // Toute l'équipe de celui qui boit se régénère (boire une deuxième fois relance la durée).
                    for (hero, hero_party) in &team {
                        if hero_party == party {
                            commands.entity(hero).insert((Regen(POTION_DURATION), Regenerating));
                        }
                    }
                }
                PickupKind::Food => {
                    // La nourriture reste au sol si le héros est déjà en pleine forme.
                    if health.current >= health.max {
                        continue;
                    }
                    health.current = (health.current + FOOD_HEAL).min(health.max);
                    stats.food += 1;
                }
            }
            commands.entity(entity).despawn();
            break;
        }
    }
}

/// Les héros sous potion regagnent des PV chaque seconde; l'effet s'arrête à la fin du délai ou si le héros tombe.
fn run_regen(
    mut commands: Commands,
    time: Res<Time>,
    mut heroes: Query<(Entity, &mut Health, &mut Regen, Has<Downed>), With<Player>>,
) {
    for (entity, mut health, mut regen, downed) in &mut heroes {
        let step = time.delta_secs().min(regen.0);
        regen.0 -= step;
        if !downed {
            health.current = (health.current + health.max * POTION_HEAL_PER_SECOND * step).min(health.max);
        }
        if regen.0 <= 0.0 || downed {
            commands.entity(entity).remove::<(Regen, Regenerating)>();
        }
    }
}

fn down_players(
    mut commands: Commands,
    mut players: Query<
        (Entity, &mut Health, &mut MoveDir, &mut Attacking, &mut Interacting),
        (With<Player>, Without<Downed>),
    >,
) {
    for (entity, mut health, mut dir, mut attacking, mut interacting) in &mut players {
        if health.current <= 0.0 {
            health.current = 0.0;
            dir.0 = Vec2::ZERO;
            attacking.0 = false;
            interacting.0 = false;
            commands.entity(entity).insert(Downed);
        }
    }
}

/// Un allié debout à proximité relève un héros à terre au bout de quelques secondes.
fn revive_players(
    mut commands: Commands,
    time: Res<Time>,
    mut downed: Query<(Entity, &Position, &mut Revive, &mut Health), With<Downed>>,
    standing: Query<&Position, (With<Player>, Without<Downed>)>,
) {
    for (entity, pos, mut revive, mut health) in &mut downed {
        let helped = standing.iter().any(|p| p.0.distance(pos.0) <= REVIVE_RANGE);
        if helped {
            revive.0 += time.delta_secs();
        } else {
            revive.0 = (revive.0 - time.delta_secs()).max(0.0);
        }
        if revive.0 >= REVIVE_TIME {
            revive.0 = 0.0;
            health.current = health.max * 0.5;
            commands.entity(entity).remove::<Downed>();
        }
    }
}

/// Tous les héros à terre: la salle est réinitialisée.
/// Toute l'équipe est à terre: retour au début du niveau, tel qu'à l'entrée (même château, monstres revenus, bourse
/// de l'entrée).
fn check_wipe(
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    mut session: ResMut<DialogueSession>,
    campaign: Res<Campaign>,
    mut players: Query<
        (Entity, &mut Position, &mut Health, &mut Gold, &mut Keys, &PartyId, Has<Downed>),
        With<Player>,
    >,
    world: Query<Entity, (With<Replicated>, Without<Player>)>,
    triggers: Query<Entity, With<DialogueTrigger>>,
) {
    if players.is_empty() || !players.iter().all(|(.., downed)| downed) {
        return;
    }
    info!("all heroes down, back to the start of level {}", campaign.0.level);
    let dungeon = generate_level(&mut commands, &mut rng, &campaign, &mut session, world.iter().chain(triggers.iter()));
    // La n-ième équipe retrouve la bourse qu'elle portait en entrant; son premier héros la tient.
    let mut order: Vec<u32> = players.iter().map(|(.., party, _)| party.0).collect();
    order.sort_unstable();
    order.dedup();
    let mut given = Vec::new();
    for (slot, (entity, mut pos, mut health, mut gold, mut keys, party, _)) in players.iter_mut().enumerate() {
        pos.0 = spawn_point(slot, &dungeon.layout, &dungeon.room);
        health.current = health.max;
        commands.entity(entity).remove::<Downed>();
        let index = order.iter().position(|p| *p == party.0).unwrap_or(0);
        let (purse_gold, purse_keys) = if given.contains(&party.0) {
            (0, 0)
        } else {
            given.push(party.0);
            campaign.0.purses.get(index).copied().unwrap_or_default()
        };
        gold.0 = purse_gold;
        keys.0 = purse_keys;
    }
    commands.insert_resource(dungeon.room);
    commands.insert_resource(dungeon.layout);
}

/// Point d'apparition d'un héros dans la salle de départ: quatre par rangée.
fn spawn_point(slot: usize, layout: &Layout, room: &Room) -> Vec2 {
    // Option de développement: `CASTLE_DEBUG_START=royal` fait apparaître l'équipe dans l'antichambre de la porte royale,
    // pour essayer l'énigme sans traverser tout le château.
    let map = room;
    let room = match std::env::var("CASTLE_DEBUG_START").as_deref() {
        Ok("royal") => layout.royal_connection().b,
        _ => layout.start,
    };
    let center = tile_to_world(map, layout.rooms[room].center());
    center + Vec2::new((slot % 4) as f32 * 22.0 - 33.0, (slot / 4) as f32 * 24.0)
}

/// Centre d'une tuile en pixels; la carte est centrée sur l'origine, comme `Room::tile_center`.
fn tile_to_world(room: &Room, tile: UVec2) -> Vec2 {
    room.tile_center(tile.x, tile.y)
}

/// Un point au hasard sur le sol d'une salle, à `margin` tuiles des bords.
fn random_point(room: &Room, info: &RoomInfo, margin: u32, rng: &mut Rng) -> Vec2 {
    for _ in 0..12 {
        let x = info.min.x + margin + (rng.next_f32() * (info.size().x - 2 * margin) as f32) as u32;
        let y = info.min.y + margin + (rng.next_f32() * (info.size().y - 2 * margin) as f32) as u32;
        if room.tile(x as i32, y as i32) == Tile::Floor {
            return tile_to_world(room, UVec2::new(x, y));
        }
    }
    tile_to_world(room, info.center())
}

/// Purge tout dialogue en cours ou en attente (nouveau niveau, retour au début du niveau): la session du serveur est remise à
/// zéro et chaque client est prévenu d'abandonner la scène qu'il affiche. Sans cela, une scène non terminée resterait à l'écran
/// du niveau suivant, bloquée, puisque le serveur ne la connaît plus.
fn reset_dialogues(commands: &mut Commands, session: &mut DialogueSession) {
    *session = DialogueSession::default();
    commands.server_trigger(ToClients { targets: SendTargets::All, message: DialogueAbort });
}

/// Charge le niveau de la campagne: c'est toujours le même (un fichier de `shared/levels/`), à chaque chargement et à
/// chaque retour au début du niveau. `old` est l'ancien château, à effacer.
fn generate_level(
    commands: &mut Commands,
    rng: &mut Rng,
    campaign: &Campaign,
    session: &mut DialogueSession,
    old: impl IntoIterator<Item = Entity>,
) -> Dungeon {
    for entity in old {
        commands.entity(entity).despawn();
    }
    reset_dialogues(commands, session);
    rng.0 = campaign.0.entry_rng;
    let level = campaign.0.level;
    let def = level_def(level);
    let dungeon = def.to_dungeon().unwrap_or_else(|e| panic!("niveau {level}: {e}"));
    let first = level == 1 && !campaign.0.intro_done;
    // Les décors au sol tirent leurs détails de cette graine: elle ne dépend que du niveau.
    let seed = u64::from(level).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    spawn_level(commands, def, level, first, seed);
    dungeon
}

fn start_first_expedition(
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    campaign: Res<Campaign>,
    mut session: ResMut<DialogueSession>,
) {
    let dungeon = generate_level(&mut commands, &mut rng, &campaign, &mut session, []);
    commands.insert_resource(Level(campaign.0.level));
    commands.insert_resource(dungeon.room);
    commands.insert_resource(dungeon.layout);
}

/// Numéro du niveau en cours.
#[derive(Resource)]
struct Level(u32);

/// Temps pendant lequel un héros est resté près d'un prisonnier.
#[derive(Component)]
struct RescueHold(f32);

/// Secondes à passer près d'un prisonnier pour le délivrer, et distance de rencontre.
const RESCUE_TIME: f32 = 1.2;
const RESCUE_RANGE: f32 = 22.0;

/// Scènes de dialogue qu'un niveau peut déclencher (les noms sont des chaînes dans les fichiers de niveau).
fn dialogue_node(name: &str) -> Option<&'static str> {
    [
        "intro",
        "royal_door",
        "alcove",
        "puzzle_target",
        "puzzle_rune",
        "puzzle_gears",
        "bridge_intro",
        "bridge_panel",
        "drawbridge_intro",
        "drawbridge_panel",
        "gate_intro",
        "gate_panel",
        "castle_intro",
        "knight_intro",
        "knight_door",
    ]
    .into_iter()
    .find(|n| *n == name)
}

/// Compose un niveau à partir d'un château généré: tous les tirages au sort (générateurs, monstres, pièges, trésor, butin,
/// énigme, objectif) se font ici, et le résultat est une description (`LevelDef`) qu'on peut écrire dans un fichier.
fn plan_level(dungeon: &Dungeon, name: &str, region: Region, level: u32, rng: &mut Rng) -> LevelDef {
    let (room, layout) = (&dungeon.room, &dungeon.layout);
    let size = room.size();
    let mut things = Vec::new();
    // Position en tuiles d'un point du monde, pour la description du niveau.
    let at = |world: Vec2| LevelDef::tile_in(size, world);
    // Un objet au sol est un peu décalé de son point d'origine.
    let scattered = |world: Vec2, rng: &mut Rng| {
        let jitter = Vec2::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5) * 12.0;
        at(world + jitter)
    };

    for info in &layout.rooms {
        match info.kind {
            RoomKind::Combat => {
                let area = info.size().x * info.size().y;
                // Une salle de taille moyenne a deux générateurs, une grande salle en a parfois trois.
                let count = 1 + u32::from(area > 90) + u32::from(area > 200 && rng.next_f32() < 0.5);
                for _ in 0..count {
                    things.push(Thing::Generator { at: at(random_point(room, info, 2, rng)) });
                }
                for _ in 0..monsters_per_room(level) {
                    let kind = MonsterKind::pick(level, rng.next_f32());
                    things.push(Thing::Monster { kind, at: at(random_point(room, info, 2, rng)) });
                }
                if rng.next_f32() < 0.4 {
                    for _ in 0..3 {
                        things.push(Thing::Trap { at: at(random_point(room, info, 2, rng)) });
                    }
                }
            }
            RoomKind::Treasure => {
                let center = tile_to_world(room, info.center());
                for offset in [(-20.0, -12.0), (-20.0, 12.0), (0.0, -12.0), (0.0, 12.0), (20.0, 0.0)] {
                    things.push(Thing::Pickup { kind: PickupKind::Coin, at: scattered(center + Vec2::from(offset), rng) });
                }
                things.push(Thing::Pickup { kind: PickupKind::Food, at: scattered(center, rng) });
                things.push(Thing::Pickup { kind: PickupKind::Potion, at: scattered(center + Vec2::new(0.0, -22.0), rng) });
            }
            RoomKind::Start | RoomKind::Exit => {}
        }
    }

    // Des potions de régénération dans les salles de combat.
    let fights: Vec<&RoomInfo> = layout.rooms.iter().filter(|r| r.kind == RoomKind::Combat).collect();
    if !fights.is_empty() {
        for _ in 0..potions_in_level(level) {
            let info = fights[(rng.next_u64() % fights.len() as u64) as usize];
            let world = random_point(room, info, 2, rng);
            things.push(Thing::Pickup { kind: PickupKind::Potion, at: scattered(world, rng) });
        }
    }

    // Au niveau du Chevalier, la porte royale n'a ni garde, ni herse, ni panneaux: il suffit de la pousser. Ailleurs c'est
    // l'énigme complète: trois obstacles, un par classe, sont des panneaux dans le mur de l'antichambre; ce qu'il faut
    // pour les vaincre est dispersé dans le château.
    let knight_level = level == KNIGHT_LEVEL;
    let sites = layout.puzzle_sites(room);
    if !knight_level {
        things.push(Thing::Ward { at: at(sites.door) });
        things.push(Thing::Portcullis { at: at(sites.portcullis), vertical: sites.vertical });
        let kinds = [InteractKind::Target, InteractKind::Rune, InteractKind::Mechanism];
        let nodes = ["puzzle_target", "puzzle_rune", "puzzle_gears"];
        // L'attribution des panneaux varie d'un château à l'autre.
        let shift = (rng.next_u64() % 3) as usize;
        for (i, panel) in sites.panels.iter().enumerate() {
            let k = (i + shift) % 3;
            things.push(Thing::Panel { kind: kinds[k], at: at(*panel), node: nodes[k].to_string() });
        }
    }
    // Trois ingrédients violets, une pièce orange et un disque de cible vert, chacun dans une salle de combat.
    if !fights.is_empty() && !knight_level {
        let mut order: Vec<usize> = (0..fights.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, (rng.next_u64() % (i as u64 + 1)) as usize);
        }
        let wanted = [
            (PickupKind::Ingredient, INGREDIENTS_NEEDED as usize),
            (PickupKind::GearPart, 1),
            (PickupKind::TargetDisc, 1),
        ];
        let mut next = 0;
        for (kind, count) in wanted {
            for _ in 0..count {
                let info = fights[order[next % order.len()]];
                next += 1;
                let world = random_point(room, info, 2, rng);
                things.push(Thing::Pickup { kind, at: scattered(world, rng) });
            }
        }
    }

    // L'objectif du niveau, et ce qu'il faut trouver pour l'accomplir.
    let (kind, goal) = if knight_level { (QuestKind::Breach, 1) } else { QuestKind::for_level(level) };
    if !fights.is_empty() {
        for _ in 0..goal {
            let info = fights[(rng.next_u64() % fights.len() as u64) as usize];
            let pos = at(random_point(room, info, 2, rng));
            match kind {
                QuestKind::Retrieve => things.push(Thing::Relic { at: pos }),
                QuestKind::Rescue => things.push(Thing::Captive { at: pos }),
                QuestKind::Hunt | QuestKind::Repair | QuestKind::Release | QuestKind::Unseal | QuestKind::Breach => {}
            }
        }
    }
    // Une scène présente le niveau: celui du Chevalier, puis la première énigme complète.
    let intro = match level {
        KNIGHT_LEVEL => Some("knight_intro"),
        6 => Some("castle_intro"),
        _ => None,
    };
    if let Some(node) = intro {
        things.push(Thing::Trigger {
            node: node.to_string(),
            at: at(tile_to_world(room, layout.rooms[layout.start].center())),
            radius: 200.0,
            delay: 2.0,
        });
    }
    LevelDef::from_dungeon(name, region, dungeon, Some((kind, goal)), things)
}

/// Pose un niveau dans le monde: ses éléments, ses portes et son objectif.
fn spawn_level(commands: &mut Commands, def: &LevelDef, level: u32, first: bool, seed: u64) {
    info!("new expedition: level {level} ({}), region {:?}", def.name, def.region);
    commands.spawn((Replicated, Expedition { seed, region: def.region, level }));
    commands.insert_resource(LevelRegion(def.region));
    commands.spawn((Replicated, PuzzleKit::default()));

    for thing in &def.things {
        let pos = def.world(thing.at());
        match thing {
            Thing::Monster { kind, .. } => spawn_monster(commands, *kind, pos),
            Thing::Generator { .. } => {
                commands.spawn((
                    Replicated,
                    Generator,
                    Health::full(GENERATOR_HEALTH),
                    Position(pos),
                    Hurtbox(GENERATOR_HALF_SIZE),
                    SpawnTimer(GENERATOR_INTERVAL),
                ));
            }
            Thing::Pickup { kind, .. } => {
                commands.spawn((Replicated, Pickup(*kind), Position(pos)));
            }
            Thing::Trap { .. } => {
                commands.spawn((
                    Replicated,
                    Trap { armed: false },
                    Position(pos),
                    Hold::default(),
                    TrapCooldown::default(),
                    TrapIdle::default(),
                ));
            }
            Thing::Relic { .. } => {
                commands.spawn((Replicated, Relic, Position(pos)));
            }
            Thing::Captive { .. } => {
                commands.spawn((Replicated, Captive, Position(pos), RescueHold(0.0)));
            }
            Thing::Ward { .. } => {
                commands.spawn((Replicated, Ward { radius: WARD_RADIUS }, Position(pos)));
            }
            Thing::Portcullis { vertical, .. } => {
                let half = if *vertical { Vec2::new(TILE_SIZE / 2.0, TILE_SIZE) } else { Vec2::new(TILE_SIZE, TILE_SIZE / 2.0) };
                commands.spawn((Replicated, Portcullis { vertical: *vertical }, Position(pos), Solid(half)));
            }
            Thing::Panel { kind, node, .. } => {
                commands.spawn((Replicated, Interactable::new(*kind), Position(pos), Hold::default()));
                if let Some(node) = dialogue_node(node) {
                    commands.spawn((Position(pos), DialogueTrigger { node, radius: 64.0, delay: 0.4, fired: false }));
                }
            }
            Thing::Exit { kind, .. } => {
                commands.spawn((Replicated, LevelExit { kind: *kind, open: false }, Position(pos)));
            }
            Thing::Trigger { node, radius, delay, .. } => {
                if let Some(node) = dialogue_node(node) {
                    commands.spawn((Position(pos), DialogueTrigger { node, radius: *radius, delay: *delay, fired: false }));
                }
            }
        }
    }

    // Portes: la royale devant la sortie, une porte à clé devant le trésor.
    let royal_node = if def.things.iter().any(|t| matches!(t, Thing::Ward { .. })) { "royal_door" } else { "knight_door" };
    let center_of = |tile: UVec2| def.world(tile.as_vec2() + Vec2::splat(0.5));
    for conn in &def.layout.connections {
        let Some(kind) = conn.door else {
            continue;
        };
        let (a, b) = if conn.door_vertical {
            (conn.door_min, conn.door_min + UVec2::Y)
        } else {
            (conn.door_min, conn.door_min + UVec2::X)
        };
        let center = (center_of(a) + center_of(b)) / 2.0;
        let half = if conn.door_vertical {
            Vec2::new(TILE_SIZE / 2.0, TILE_SIZE)
        } else {
            Vec2::new(TILE_SIZE, TILE_SIZE / 2.0)
        };
        let door = commands
            .spawn((
                Replicated,
                Door { kind, vertical: conn.door_vertical },
                Position(center),
                Solid(half),
            ))
            .id();
        // La porte royale est entourée d'un enchantement qui tient à distance: son dialogue part au bord de celui-ci.
        let (node, radius) = match kind {
            DoorKind::Royal => {
                commands
                    .entity(door)
                    .insert((Interactable::new(InteractKind::Push), Hold::default()));
                (royal_node, WARD_RADIUS + 40.0)
            }
            DoorKind::Locked => ("alcove", 70.0),
        };
        commands.spawn((
            Position(center),
            DialogueTrigger { node, radius, delay: 0.3, fired: false },
        ));
    }

    // Sans sortie posée à la main, elle est au centre de la salle de sortie.
    if !def.things.iter().any(|t| matches!(t, Thing::Exit { .. })) {
        let exit = def.layout.rooms[def.layout.exit].center();
        commands.spawn((Replicated, LevelExit { kind: ExitKind::Stairs, open: false }, Position(center_of(exit))));
    }

    // L'objectif du niveau. La sortie reste fermée tant qu'il n'est pas rempli (voir `open_exit`).
    // Dans un combat de boss, vaincre toute la meute est un objectif à part, affiché avant les autres.
    if is_boss_level(level) && def.quest.is_none_or(|(kind, _)| kind != QuestKind::Hunt) {
        let beasts = def.things.iter().filter(|t| matches!(t, Thing::Monster { .. })).count() as u32;
        if beasts > 0 {
            commands.spawn((Replicated, Quest { kind: QuestKind::Hunt, goal: beasts, progress: 0 }));
        }
    }
    if let Some((kind, goal)) = def.quest {
        commands.spawn((Replicated, Quest { kind, goal, progress: 0 }));
    }

    if first {
        let start = def.layout.rooms[def.layout.start].center();
        commands.spawn((
            Position(center_of(start)),
            DialogueTrigger { node: "intro", radius: 200.0, delay: 3.0, fired: false },
        ));
    }
}

/// Région de chaque niveau de la campagne (les extérieurs d'abord, puis les régions du château).
/// Le niveau où le Chevalier est à l'honneur: la porte royale n'a qu'à être poussée.
const KNIGHT_LEVEL: u32 = 5;

const CAMPAIGN_REGIONS: [Region; LEVEL_COUNT as usize] = [
    Region::Forest,
    Region::Marsh,
    Region::Approach,
    Region::Approach,
    Region::Crypts,
    Region::Gardens,
    Region::Library,
    Region::Library,
    Region::Forge,
    Region::Tower,
    Region::Royal,
    Region::Royal,
];

/// Les niveaux extérieurs de la campagne: la forêt (chasser la meute pour ouvrir la barrière), le marais (retrouver les
/// planches du pont), le chemin du château (délivrer les gardes qui baissent le pont-levis) et l'arène de la meute, devant
/// la porte du château.
fn outdoor_spec(level: u32) -> Option<outdoor::OutdoorSpec> {
    let spec = match level {
        // Le premier niveau sert d'introduction: pas de mécanisme, juste la chasse.
        1 => outdoor::OutdoorSpec {
            name: "La forêt de brume",
            region: Region::Forest,
            level,
            clearings: 3,
            pack: 3,
            // Douze loups en plus: la forêt est infestée.
            extra_wolves: 12,
            generators: false,
            quest: Some((QuestKind::Hunt, 12)),
            exit: ExitKind::Gate,
            arena: false,
            mechanism: None,
            captives: 0,
        },
        // Le Forgebraise répare le pont avec une pièce de rechange trouvée dans une clairière.
        2 => outdoor::OutdoorSpec {
            name: "Les marais hantés",
            region: Region::Marsh,
            level,
            clearings: 4,
            pack: 3,
            extra_wolves: 0,
            generators: true,
            quest: Some((QuestKind::Repair, 1)),
            exit: ExitKind::Bridge,
            arena: false,
            mechanism: Some(outdoor::Mechanism { kind: InteractKind::Mechanism, intro: "bridge_intro", panel: "bridge_panel" }),
            captives: 0,
        },
        // Le Rôdeur fixe le disque d'une cible puis la touche d'une flèche pour libérer le pont-levis; des gardes du château
        // sont prisonniers dans des cages le long du chemin (délivrés, ils laissent du butin).
        3 => outdoor::OutdoorSpec {
            name: "Le chemin du château",
            region: Region::Approach,
            level,
            clearings: 4,
            pack: 4,
            extra_wolves: 0,
            generators: true,
            quest: Some((QuestKind::Release, 1)),
            exit: ExitKind::Drawbridge,
            arena: false,
            mechanism: Some(outdoor::Mechanism { kind: InteractKind::Target, intro: "drawbridge_intro", panel: "drawbridge_panel" }),
            captives: 2,
        },
        // La meute garde la porte: l'Ensorceleur brise le sceau (trois ingrédients), une fois la meute vaincue.
        4 => outdoor::OutdoorSpec {
            name: "La meute",
            region: Region::Approach,
            level,
            clearings: 1,
            pack: 12,
            extra_wolves: 0,
            generators: false,
            quest: Some((QuestKind::Unseal, 1)),
            exit: ExitKind::Gate,
            arena: true,
            mechanism: Some(outdoor::Mechanism { kind: InteractKind::Rune, intro: "gate_intro", panel: "gate_panel" }),
            captives: 0,
        },
        _ => return None,
    };
    Some(spec)
}

/// Écrit les niveaux de départ de la campagne dans `dir`: les extérieurs sont générés, l'intérieur vient de châteaux
/// générés avec une graine fixe. Ils sont à retoucher ensuite dans l'éditeur: un niveau déjà écrit n'est jamais écrasé,
/// sauf avec `force` (et `only` limite l'export à certains niveaux).
pub fn export_levels(dir: &std::path::Path, force: bool, only: Option<&[u32]>) -> std::io::Result<()> {
    for level in 1..=LEVEL_COUNT {
        if only.is_some_and(|only| !only.contains(&level)) {
            continue;
        }
        let path = dir.join(level_file(level));
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if !force && !existing.trim().is_empty() && existing.trim() != "()" {
            println!("niveau {level}: déjà écrit, laissé tel quel ({})", path.display());
            continue;
        }
        let seed = 1000 + u64::from(level) * 7919;
        let def = match outdoor_spec(level) {
            Some(spec) => outdoor::outdoor_level(&spec, seed),
            None => {
                let dungeon = Dungeon::generate(seed);
                let region = CAMPAIGN_REGIONS[level as usize - 1];
                plan_level(&dungeon, &format!("Niveau {level}"), region, level, &mut Rng(seed | 1))
            }
        };
        let problems = def.validate();
        if !problems.is_empty() {
            return Err(std::io::Error::other(format!("niveau {level}: {problems:?}")));
        }
        std::fs::write(&path, def.to_ron())?;
        println!("niveau {level}: {}", path.display());
    }
    Ok(())
}

/// Remplit un nouveau château généré: tirage au sort puis pose (utilisé par les tests; la campagne lit des fichiers).
#[cfg(test)]
fn populate(commands: &mut Commands, dungeon: &Dungeon, seed: u64, level: u32, first: bool, rng: &mut Rng) {
    let region = Region::CASTLE[(rng.next_u64() % Region::CASTLE.len() as u64) as usize];
    let def = plan_level(dungeon, &format!("château {seed}"), region, level, rng);
    spawn_level(commands, &def, level, first, seed);
}

/// Or et clés de chaque équipe, dans l'ordre d'arrivée des équipes.
fn purses<'a>(heroes: impl Iterator<Item = (&'a PartyId, &'a Gold, &'a Keys)>) -> Vec<(u32, u32)> {
    let mut by_party: std::collections::BTreeMap<u32, (u32, u32)> = Default::default();
    for (party, gold, keys) in heroes {
        let purse = by_party.entry(party.0).or_default();
        purse.0 += gold.0;
        purse.1 += keys.0;
    }
    by_party.into_values().collect()
}

/// Quand tous les héros debout sont dans la sortie, l'équipe entre dans le niveau suivant: la partie est sauvegardée
/// à cet instant, puis le château est généré.
fn run_exit(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut session: ResMut<DialogueSession>,
    mut level: ResMut<Level>,
    mut campaign: ResMut<Campaign>,
    mut countdown: Local<f32>,
    mut transition: ResMut<Transition>,
    mut stats: ResMut<LevelStats>,
    clients: Query<(Entity, Has<Ready>), With<ActiveHero>>,
    exits: Query<(&Position, &LevelExit), Without<Player>>,
    mut players: Query<
        (Entity, &mut Position, &mut Health, &PartyId, &Gold, &Keys, Has<Downed>, Has<Controlled>),
        With<Player>,
    >,
    world: Query<Entity, (With<Replicated>, Without<Player>)>,
    triggers: Query<Entity, With<DialogueTrigger>>,
) {
    let Ok((exit_pos, exit)) = exits.single() else {
        return;
    };
    if let Some(elapsed) = transition.0.as_mut() {
        // Les clients montrent le bilan: le niveau suivant attend que tous les joueurs aient validé.
        *elapsed += time.delta_secs();
        if *elapsed < SUMMARY_TIMEOUT && !clients.iter().all(|(_, ready)| ready) {
            return;
        }
        transition.0 = None;
        for (client, _) in &clients {
            commands.entity(client).remove::<Ready>();
        }
    } else {
        // Seuls les héros dirigés comptent: les compagnons sont ramenés avec eux.
        let standing: Vec<_> = players
            .iter()
            .filter(|(.., downed, controlled)| !downed && *controlled)
            .collect();
        let all_in = exit.open
            && !standing.is_empty()
            && standing.iter().all(|(_, p, ..)| p.0.distance(exit_pos.0) <= EXIT_RADIUS);
        if !all_in {
            *countdown = 0.0;
            return;
        }
        *countdown += time.delta_secs();
        if *countdown < EXIT_DELAY {
            return;
        }
        *countdown = 0.0;
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: LevelSummary {
                level: level.0,
                seconds: stats.seconds,
                kills: stats.kills,
                generators: stats.generators,
                damage_taken: stats.damage_taken,
                downed: stats.downed,
                gold: stats.gold,
                keys: stats.keys,
                food: stats.food,
                potions: stats.potions,
                pieces: stats.pieces,
            },
        });
        transition.0 = Some(0.0);
        return;
    }
    *stats = LevelStats::default();

    // Entrée dans le niveau suivant: c'est ici que la partie est sauvegardée.
    campaign.0.level += 1;
    campaign.0.entry_rng = rng.0;
    campaign.0.purses = purses(players.iter().map(|(_, _, _, party, gold, keys, ..)| (party, gold, keys)));
    campaign.write();
    level.0 = campaign.0.level;

    let dungeon = generate_level(&mut commands, &mut rng, &campaign, &mut session, world.iter().chain(triggers.iter()));
    for (slot, (entity, mut pos, mut health, ..)) in players.iter_mut().enumerate() {
        pos.0 = spawn_point(slot, &dungeon.layout, &dungeon.room);
        health.current = health.max;
        commands.entity(entity).remove::<Downed>();
    }
    commands.insert_resource(dungeon.room);
    commands.insert_resource(dungeon.layout);
}

fn regen_mana(time: Res<Time>, mut players: Query<(&mut Mana, &HeroClass), (With<Player>, Without<Downed>)>) {
    for (mut mana, class) in &mut players {
        mana.current = (mana.current + class.stats().mana_regen * time.delta_secs()).min(mana.max);
    }
}

/// Déclenche le pouvoir spécial demandé, si le mana et la recharge le permettent.
fn player_specials(
    mut commands: Commands,
    time: Res<Time>,
    mut players: Query<
        (Entity, &Position, &Facing, &HeroClass, &mut Mana, &mut SpecialInput, &mut SpecialCooldown, &mut Strikes),
        (With<Player>, Without<Downed>),
    >,
    allies: Query<(Entity, &Position), (With<Player>, Without<Downed>)>,
    turrets: Query<(Entity, &TurretOwner)>,
) {
    for (entity, pos, facing, class, mut mana, mut input, mut cooldown, mut strikes) in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
        if !std::mem::take(&mut input.queued) {
            continue;
        }
        let stats = class.stats();
        if cooldown.0 > 0.0 || mana.current < stats.special_cost {
            continue;
        }
        mana.current -= stats.special_cost;
        cooldown.0 = stats.special_cooldown;
        strikes.0 = strikes.0.wrapping_add(1);
        match class {
            HeroClass::Knight => {
                for (ally, ally_pos) in &allies {
                    if ally_pos.0.distance(pos.0) <= BULWARK_RADIUS {
                        commands.entity(ally).insert((Shielded, ShieldTimer(BULWARK_DURATION)));
                    }
                }
            }
            HeroClass::Sorcerer => {
                commands.spawn((
                    Replicated,
                    Zone { radius: ARCANE_RADIUS, class: *class },
                    Position(pos.0 + facing.0 * ARCANE_OFFSET),
                    ZoneLife(0.4),
                    // Un seul coup, au premier instant.
                    ZoneDamage { damage: ARCANE_DAMAGE, interval: 10.0, timer: 0.0 },
                ));
            }
            HeroClass::Ranger => {
                commands.spawn((
                    Replicated,
                    Zone { radius: RAIN_RADIUS, class: *class },
                    Position(pos.0 + facing.0 * RAIN_OFFSET),
                    ZoneLife(RAIN_DURATION),
                    ZoneDamage { damage: RAIN_DAMAGE, interval: RAIN_INTERVAL, timer: 0.0 },
                ));
            }
            HeroClass::Forgebraise => {
                // Une seule tourelle par Forgebraise.
                for (turret, owner) in &turrets {
                    if owner.0 == entity {
                        commands.entity(turret).despawn();
                    }
                }
                commands.spawn((
                    Replicated,
                    Turret,
                    TurretOwner(entity),
                    Position(pos.0 + facing.0 * 14.0),
                    TurretLife(TURRET_LIFETIME),
                    Cooldown::default(),
                ));
            }
        }
    }
}

fn tick_shields(
    mut commands: Commands,
    time: Res<Time>,
    mut shields: Query<(Entity, &mut ShieldTimer)>,
) {
    for (entity, mut timer) in &mut shields {
        timer.0 -= time.delta_secs();
        if timer.0 <= 0.0 {
            commands.entity(entity).remove::<(Shielded, ShieldTimer)>();
        }
    }
}

fn run_zones(
    mut commands: Commands,
    time: Res<Time>,
    mut zones: Query<(Entity, &Position, &Zone, &mut ZoneLife, &mut ZoneDamage)>,
    mut targets: Query<(&Position, &mut Health, &Hurtbox), (Without<Player>, Without<Zone>)>,
) {
    let dt = time.delta_secs();
    for (entity, pos, zone, mut life, mut damage) in &mut zones {
        damage.timer -= dt;
        if damage.timer <= 0.0 {
            damage.timer += damage.interval;
            for (target_pos, mut health, hurtbox) in &mut targets {
                if target_pos.0.distance(pos.0) <= zone.radius + hurtbox.0 {
                    health.current -= damage.damage;
                }
            }
        }
        life.0 -= dt;
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Les tourelles tirent sur la cible la plus proche à portée.
fn run_turrets(
    mut commands: Commands,
    time: Res<Time>,
    mut turrets: Query<(Entity, &Position, &mut Cooldown, &mut TurretLife), With<Turret>>,
    targets: Query<&Position, (With<Hurtbox>, Without<Player>, Without<Turret>)>,
) {
    let dt = time.delta_secs();
    for (entity, pos, mut cooldown, mut life) in &mut turrets {
        life.0 -= dt;
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        if cooldown.0 > 0.0 {
            continue;
        }
        let nearest = targets
            .iter()
            .map(|t| t.0)
            .filter(|t| t.distance(pos.0) <= TURRET_RANGE)
            .min_by(|a, b| a.distance(pos.0).total_cmp(&b.distance(pos.0)));
        if let Some(target) = nearest {
            let dir = (target - pos.0).normalize_or_zero();
            cooldown.0 = TURRET_COOLDOWN;
            commands.spawn((
                Replicated,
                Projectile { class: HeroClass::Forgebraise, dir },
                Position(pos.0 + dir * 8.0),
                Velocity(dir * TURRET_SHOT_SPEED),
                Damage(TURRET_DAMAGE),
                Lifetime(TURRET_RANGE / TURRET_SHOT_SPEED + 0.2),
            ));
        }
    }
}

/// Maintenir la touche d'action avec la bonne classe fait avancer l'objet visé.
/// La porte royale n'obéit au Chevalier qu'une fois les trois autres étapes terminées.
fn run_interactions(
    mut commands: Commands,
    time: Res<Time>,
    players: Query<(&Position, &HeroClass, &Interacting), (With<Player>, Without<Downed>)>,
    mut things: Query<(Entity, &Position, &mut Interactable, &mut Hold), Without<Player>>,
    mut kit: Query<&mut PuzzleKit>,
    wards: Query<Entity, With<Ward>>,
    mut quests: Query<&mut Quest>,
) {
    let dt = time.delta_secs();
    let puzzle_ready = things
        .iter()
        .filter(|(_, _, i, _)| i.kind != InteractKind::Push)
        .all(|(_, _, i, _)| i.done);
    // Ce que l'équipe a trouvé; ce qu'une étape consomme est retiré au fur et à mesure.
    let mut have = kit.single().map(|k| *k).unwrap_or_default();
    for (entity, pos, mut interactable, mut hold) in &mut things {
        if interactable.done {
            continue;
        }
        let kind = interactable.kind;
        // Une cible déjà garnie n'attend plus qu'une flèche (voir `move_projectiles`).
        if kind == InteractKind::Target && interactable.mounted {
            continue;
        }
        let unlocked = kind != InteractKind::Push || puzzle_ready;
        // Il faut avoir trouvé de quoi agir: ingrédients, pièce de rechange ou disque.
        let stocked = match kind {
            InteractKind::Target => have.target >= 1,
            InteractKind::Rune => have.ingredients >= INGREDIENTS_NEEDED,
            InteractKind::Mechanism => have.gear >= 1,
            InteractKind::Push => true,
        };
        let active = unlocked
            && stocked
            && players.iter().any(|(p, class, interacting)| {
                interacting.0 && *class == kind.class() && p.0.distance(pos.0) <= INTERACT_RANGE + 4.0
            });
        if active {
            hold.0 += dt;
        } else {
            hold.0 = (hold.0 - dt * 2.0).max(0.0);
        }
        let progress = (hold.0 / kind.hold_time()).min(1.0);
        if (interactable.progress - progress).abs() > 0.01 {
            interactable.progress = progress;
        }
        if hold.0 < kind.hold_time() {
            continue;
        }
        info!("{kind:?} done");
        // La porte royale qui cède secoue plus fort qu'une étape intermédiaire.
        let strength = if kind == InteractKind::Push { 0.85 } else { 0.5 };
        commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(strength) });
        match kind {
            InteractKind::Target => {
                // Le disque est fixé: la cible attend maintenant la flèche.
                have.target -= 1;
                interactable.mounted = true;
                interactable.progress = 0.0;
                hold.0 = 0.0;
            }
            InteractKind::Rune => {
                have.ingredients -= INGREDIENTS_NEEDED;
                interactable.done = true;
                interactable.progress = 1.0;
                // L'enchantement se dissipe.
                for ward in &wards {
                    commands.entity(ward).despawn();
                }
            }
            InteractKind::Mechanism => {
                have.gear -= 1;
                interactable.done = true;
                interactable.progress = 1.0;
            }
            InteractKind::Push => {
                interactable.done = true;
                interactable.progress = 1.0;
                commands.entity(entity).despawn();
            }
        }
        // Un objectif propre à cette classe est rempli sur-le-champ (la porte poussée disparaît: on ne pourrait plus le lire).
        if kind != InteractKind::Target {
            for mut quest in &mut quests {
                if quest.kind.mechanism() == Some(kind) {
                    quest.progress = quest.goal;
                }
            }
        }
    }
    if let Ok(mut stock) = kit.single_mut() {
        if *stock != have {
            *stock = have;
        }
    }
}

/// L'enchantement de la porte royale repousse tout héros qui s'en approche trop.
fn run_ward(
    time: Res<Time>,
    room: Res<Room>,
    wards: Query<(&Position, &Ward), Without<Player>>,
    mut heroes: Query<&mut Position, With<Player>>,
) {
    let dt = time.delta_secs();
    for (center, ward) in &wards {
        for mut pos in &mut heroes {
            let away = pos.0 - center.0;
            if away.length() >= ward.radius {
                continue;
            }
            let direction = away.try_normalize().unwrap_or(Vec2::Y);
            let moved = room.move_and_collide(pos.0, direction * WARD_PUSH_SPEED * dt, HERO_HALF_SIZE);
            if moved != pos.0 {
                pos.0 = moved;
            }
        }
    }
}

/// Les pièges blessent qui marche dessus; le Rôdeur peut les désamorcer en maintenant la touche d'action.
fn run_traps(
    mut commands: Commands,
    time: Res<Time>,
    mut traps: Query<(Entity, &Position, &mut Trap, &mut Hold, &mut TrapCooldown, &mut TrapIdle, Has<Disarmed>), Without<Player>>,
    mut players: Query<(&Position, &HeroClass, &Interacting, &mut Health), (With<Player>, Without<Downed>)>,
) {
    let dt = time.delta_secs();
    for (entity, pos, mut trap, mut hold, mut cooldown, mut idle, disarmed) in &mut traps {
        // Un piège désarmé reste visible, pointes rentrées, pour de bon.
        if disarmed {
            if trap.armed {
                trap.armed = false;
            }
            continue;
        }
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        // Le Rôdeur désarme en maintenant l'action près du piège, qu'il soit réveillé ou endormi.
        let disarming = players.iter().any(|(p, class, interacting, _)| {
            interacting.0 && *class == HeroClass::Ranger && p.0.distance(pos.0) <= INTERACT_RANGE
        });
        if disarming {
            hold.0 += dt;
            if hold.0 >= TRAP_DISARM_TIME {
                commands.entity(entity).insert(Disarmed);
                if trap.armed {
                    trap.armed = false;
                }
                continue;
            }
        } else {
            hold.0 = (hold.0 - dt).max(0.0);
        }
        // Réveil à l'approche d'un héros, sommeil quand tout le monde s'est éloigné.
        let near = players.iter().any(|(p, ..)| p.0.distance(pos.0) <= TRAP_WAKE_RADIUS);
        if near {
            idle.0 = 0.0;
            if !trap.armed {
                trap.armed = true;
            }
        } else {
            idle.0 += dt;
            if idle.0 >= TRAP_SLEEP_DELAY && trap.armed {
                trap.armed = false;
            }
        }
        if !trap.armed || cooldown.0 > 0.0 {
            continue;
        }
        for (p, _, _, mut health) in &mut players {
            if p.0.distance(pos.0) <= TRAP_TRIGGER_RADIUS {
                health.current -= TRAP_DAMAGE;
                cooldown.0 = TRAP_COOLDOWN;
            }
        }
    }
}

/// Une porte à clé s'ouvre quand un héros de l'équipe est devant et que l'équipe a une clé (une clé est consommée).
fn open_locked_doors(
    mut commands: Commands,
    doors: Query<(Entity, &Position, &Door), Without<Player>>,
    mut heroes: Query<(&Position, &PartyId, &mut Keys, Has<Downed>), With<Player>>,
) {
    for (entity, pos, door) in &doors {
        if door.kind != DoorKind::Locked {
            continue;
        }
        let near = heroes
            .iter()
            .find(|(p, _, _, downed)| !downed && p.0.distance(pos.0) <= DOOR_OPEN_RANGE + TILE_SIZE / 2.0)
            .map(|(_, party, ..)| *party);
        let Some(party) = near else {
            continue;
        };
        if let Some((_, _, mut keys, _)) = heroes
            .iter_mut()
            .find(|(_, id, keys, _)| **id == party && keys.0 > 0)
        {
            keys.0 -= 1;
            commands.entity(entity).despawn();
            commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(0.3) });
        }
    }
}

/// Un héros qui reste dans une zone de dialogue lance la scène, si aucune autre n'est en cours.
fn run_dialogue_triggers(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<DialogueSession>,
    mut triggers: Query<(&Position, &mut DialogueTrigger)>,
    players: Query<&Position, (With<Player>, Without<Downed>, Without<DialogueTrigger>)>,
    monsters: Query<&Position, (With<Monster>, Without<Player>, Without<DialogueTrigger>)>,
) {
    if session.active.is_some() {
        return;
    }
    // Les scènes n'ont lieu que quand il n'y a plus d'ennemi à proximité: on ne fait pas avancer l'intrigue en plein combat.
    if monsters.iter().any(|m| players.iter().any(|p| p.0.distance(m.0) <= DIALOGUE_SAFE_RADIUS)) {
        return;
    }
    for (pos, mut trigger) in &mut triggers {
        if trigger.fired {
            continue;
        }
        if !players.iter().any(|p| p.0.distance(pos.0) <= trigger.radius) {
            continue;
        }
        trigger.delay -= time.delta_secs();
        if trigger.delay > 0.0 {
            continue;
        }
        trigger.fired = true;
        *session = DialogueSession {
            active: Some(trigger.node.to_string()),
            ..default()
        };
        info!("dialogue: {}", trigger.node);
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: DialogueBegin(trigger.node.to_string()),
        });
        return;
    }
}

fn tick_dialogue(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<DialogueSession>,
    clients: Query<(), With<ActiveHero>>,
) {
    if session.active.is_none() {
        return;
    }
    let dt = time.delta_secs();
    session.advance_cooldown = (session.advance_cooldown - dt).max(0.0);
    session.idle += dt;
    if session.idle > DIALOGUE_IDLE_TIMEOUT {
        warn!("dialogue abandoned after a long silence");
        // Les clients attendent encore cette scène: on la leur fait abandonner aussi.
        reset_dialogues(&mut commands, &mut session);
        return;
    }
    if !session.votes.is_empty() {
        session.vote_timer += dt;
        if session.vote_timer >= DIALOGUE_VOTE_TIMEOUT || session.votes.len() >= clients.iter().count() {
            resolve_votes(&mut commands, &mut session);
        }
    }
}

/// Le choix le plus voté l'emporte; en cas d'égalité, le premier de la liste.
fn resolve_votes(commands: &mut Commands, session: &mut DialogueSession) {
    let mut tally: HashMap<u32, usize> = HashMap::new();
    for choice in session.votes.values() {
        *tally.entry(*choice).or_default() += 1;
    }
    let winner = tally
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(choice, _)| choice)
        .unwrap_or(0);
    session.votes.clear();
    session.vote_timer = 0.0;
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: DialogueSelect(winner),
    });
}

fn dialogue_advance(
    _request: On<FromClient<DialogueRequestAdvance>>,
    mut commands: Commands,
    mut session: ResMut<DialogueSession>,
) {
    if session.active.is_none() || session.advance_cooldown > 0.0 {
        return;
    }
    session.advance_cooldown = DIALOGUE_ADVANCE_COOLDOWN;
    session.idle = 0.0;
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: DialogueAdvance,
    });
}

/// Purge les dialogues: la scène en cours est abandonnée, et celles qui allaient démarrer (un héros est dans leur zone) ne
/// partiront plus. Renvoie (une scène était en cours, c'était l'introduction qu'on n'avait pas encore vue).
fn purge_dialogues(
    session: &mut DialogueSession,
    intro_done: bool,
    triggers: &mut Query<(&Position, &mut DialogueTrigger)>,
    heroes: &[Vec2],
) -> (bool, bool) {
    let was_active = session.active.is_some();
    let skipped_intro = session.active.as_deref() == Some("intro") && !intro_done;
    *session = DialogueSession::default();
    for (pos, mut trigger) in triggers.iter_mut() {
        if !trigger.fired && heroes.iter().any(|h| h.distance(pos.0) <= trigger.radius) {
            trigger.fired = true;
        }
    }
    (was_active, skipped_intro)
}

/// Entrée maintenu: on purge la scène en cours et celles qui allaient démarrer. Une introduction purgée n'est plus rejouée.
fn dialogue_skip(
    _request: On<FromClient<DialogueSkip>>,
    mut commands: Commands,
    mut session: ResMut<DialogueSession>,
    mut campaign: ResMut<Campaign>,
    mut triggers: Query<(&Position, &mut DialogueTrigger)>,
    players: Query<&Position, (With<Player>, Without<DialogueTrigger>)>,
) {
    let heroes: Vec<Vec2> = players.iter().map(|p| p.0).collect();
    let (was_active, skipped_intro) = purge_dialogues(&mut session, campaign.0.intro_done, &mut triggers, &heroes);
    if skipped_intro {
        campaign.0.intro_done = true;
        campaign.write();
    }
    if was_active {
        commands.server_trigger(ToClients { targets: SendTargets::All, message: DialogueAbort });
    }
}

fn dialogue_vote(vote: On<FromClient<DialogueVote>>, mut session: ResMut<DialogueSession>) {
    let Some(client) = vote.client_id.entity() else {
        return;
    };
    if session.active.is_some() {
        session.idle = 0.0;
        session.votes.insert(client, vote.0);
    }
}

fn dialogue_finished(
    _done: On<FromClient<DialogueFinished>>,
    mut session: ResMut<DialogueSession>,
    mut campaign: ResMut<Campaign>,
) {
    if session.active.as_deref() == Some("intro") && !campaign.0.intro_done {
        campaign.0.intro_done = true;
        campaign.write();
    }
    *session = DialogueSession::default();
}

fn spawn_monster(commands: &mut Commands, kind: MonsterKind, pos: Vec2) {
    let stats = kind.stats();
    commands.spawn((
        Replicated,
        Monster(kind),
        Health::full(stats.max_hp),
        Position(pos),
        Hurtbox(stats.half_size),
        Cooldown::default(),
        Strikes::default(),
    ));
}

/// Direction à prendre pour aller de `pos` à `goal`: tout droit si la voie est libre, sinon en suivant
/// un itinéraire calculé à travers la carte (murs, couloirs, portes fermées).
fn steer(room: &Room, solids: &[(Vec2, Vec2)], route: &mut Route, pos: Vec2, goal: Vec2, dt: f32) -> Vec2 {
    if room.segment_clear(pos, goal, HERO_HALF_SIZE) {
        route.waypoints.clear();
        return (goal - pos).normalize_or_zero();
    }
    route.age += dt;
    if route.waypoints.is_empty() || route.goal.distance(goal) > ROUTE_GOAL_DRIFT || route.age > ROUTE_MAX_AGE {
        route.goal = goal;
        route.age = 0.0;
        route.waypoints = room.find_path(pos, goal, solids).unwrap_or_default();
    }
    // On saute les points déjà atteints, ou masqués par un raccourci direct.
    while let Some(first) = route.waypoints.first() {
        let reached = first.distance(pos) < 5.0;
        let shortcut = route.waypoints.len() >= 2 && room.segment_clear(pos, route.waypoints[1], HERO_HALF_SIZE);
        if reached || shortcut {
            route.waypoints.remove(0);
        } else {
            break;
        }
    }
    match route.waypoints.first() {
        Some(next) => (*next - pos).normalize_or_zero(),
        // Pas d'itinéraire (but inaccessible): on essaie quand même d'en approcher.
        None => (goal - pos).normalize_or_zero(),
    }
}

/// Les compagnons (héros non dirigés) suivent le héros dirigé, relèvent les alliés à terre et combattent seuls,
/// avec leur seule arme de base: les pouvoirs spéciaux restent réservés au héros que dirige le joueur.
fn hero_ai(
    time: Res<Time>,
    room: Res<Room>,
    doors: Query<(&Position, &Solid), Without<Player>>,
    mut heroes: Query<
        (
            &PartyId,
            &HeroClass,
            &Position,
            &mut MoveDir,
            &mut Facing,
            &mut Attacking,
            &mut Route,
        ),
        (With<Player>, Without<Controlled>, Without<Downed>),
    >,
    leaders: Query<(&PartyId, &Position), (With<Controlled>, Without<Downed>)>,
    downed: Query<(&PartyId, &Position), (With<Player>, With<Downed>)>,
    enemies: Query<&Position, (Or<(With<Monster>, With<Generator>)>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let solids: Vec<_> = doors.iter().map(|(p, s)| (p.0, s.0)).collect();
    for (party, class, pos, mut dir, mut facing, mut attacking, mut route) in &mut heroes {
        dir.0 = Vec2::ZERO;
        attacking.0 = false;
        let leader = leaders.iter().find(|(id, _)| *id == party).map(|(_, p)| p.0);

        // Un ennemi (monstre ou générateur) n'est engagé que s'il est visible (pas derrière un mur) et proche du groupe.
        let nearest = enemies
            .iter()
            .map(|e| (e.0.distance(pos.0), e.0))
            .filter(|(d, e)| {
                *d <= AI_AGGRO
                    && leader.is_none_or(|l| l.distance(*e) <= AI_GROUP_RADIUS)
                    && room.segment_clear(pos.0, *e, 1.0)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));

        if let Some((distance, target)) = nearest {
            let to = (target - pos.0).normalize_or_zero();
            facing.0 = to;
            match class.stats().attack {
                Attack::Melee { range, .. } => {
                    if distance > range * 0.7 {
                        dir.0 = to;
                    }
                    attacking.0 = distance <= range + 6.0;
                }
                Attack::Shot { .. } => {
                    // Les tireurs gardent leurs distances.
                    attacking.0 = distance <= AI_SHOT_REACH;
                    if distance < 45.0 {
                        dir.0 = -to;
                    } else if distance > 100.0 {
                        dir.0 = to;
                    }
                }
            }
            continue;
        }

        // Pas d'ennemi en vue: aller relever un allié à terre, sinon suivre le chef.
        let ally = downed
            .iter()
            .filter(|(id, _)| *id == party)
            .map(|(_, p)| p.0)
            .min_by(|a, b| a.distance(pos.0).total_cmp(&b.distance(pos.0)));
        if let Some(ally) = ally {
            if ally.distance(pos.0) > REVIVE_RANGE * 0.5 {
                dir.0 = steer(&room, &solids, &mut route, pos.0, ally, dt);
            }
        } else if let Some(leader) = leader {
            // Dans un couloir étroit la place prévue en formation tombe dans un mur: on suit alors le chef de près.
            let slot = leader + formation_offset(*class);
            let free = !room.overlaps_wall(slot, HERO_HALF_SIZE) && room.segment_clear(leader, slot, HERO_HALF_SIZE);
            let (goal, stop) = if free { (slot, 10.0) } else { (leader, 16.0) };
            if goal.distance(pos.0) > stop {
                dir.0 = steer(&room, &solids, &mut route, pos.0, goal, dt);
            } else {
                route.waypoints.clear();
            }
        }
        if let Some(d) = dir.0.try_normalize() {
            facing.0 = d;
        }
    }
}

/// Place de chaque compagnon autour du héros dirigé.
fn formation_offset(class: HeroClass) -> Vec2 {
    match class {
        HeroClass::Knight => Vec2::new(-18.0, -14.0),
        HeroClass::Sorcerer => Vec2::new(-30.0, 10.0),
        HeroClass::Ranger => Vec2::new(-18.0, 22.0),
        HeroClass::Forgebraise => Vec2::new(-34.0, -10.0),
    }
}

/// Un compagnon qui s'est perdu (coincé derrière un angle, ou trop loin) est ramené auprès du chef.
fn leash_followers(
    mut followers: Query<(&PartyId, &mut Position), (With<Player>, Without<Controlled>, Without<Downed>)>,
    leaders: Query<(&PartyId, &Position), (With<Controlled>, Without<Downed>)>,
) {
    for (party, mut pos) in &mut followers {
        if let Some((_, leader)) = leaders.iter().find(|(id, _)| *id == party) {
            if leader.0.distance(pos.0) > AI_LEASH {
                pos.0 = leader.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hero(world: &mut World, party: u32, at: Vec2, controlled: bool, hp: f32) -> Entity {
        let mut entity = world.spawn((
            Player,
            PartyId(party),
            HeroClass::Knight,
            Position(at),
            Health { current: hp, max: 100.0 },
            Gold::default(),
            Keys::default(),
            Interacting::default(),
        ));
        if controlled {
            entity.insert(Controlled);
        }
        entity.id()
    }

    fn app_with_time() -> App {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app
    }

    fn advance(app: &mut App, seconds: f32) {
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs_f32(seconds));
        app.update();
    }

    /// Une potion rend POTION_HEAL_PER_SECOND des PV maximum par seconde, pendant POTION_DURATION secondes, sans dépasser le maximum.
    #[test]
    fn regeneration_heals_over_time_then_stops() {
        let mut app = app_with_time();
        app.add_systems(Update, run_regen);
        let id = hero(app.world_mut(), 0, Vec2::ZERO, true, 10.0);
        app.world_mut().entity_mut(id).insert((Regen(POTION_DURATION), Regenerating));

        advance(&mut app, 1.0);
        let hp = app.world().get::<Health>(id).unwrap().current;
        assert!((hp - (10.0 + 100.0 * POTION_HEAL_PER_SECOND)).abs() < 0.01, "après 1 s: {hp}");

        advance(&mut app, POTION_DURATION);
        let health = app.world().get::<Health>(id).unwrap();
        let healed = health.current - 10.0;
        // L'effet dure POTION_DURATION en tout: pas un PV de plus.
        assert!((healed - 100.0 * POTION_HEAL_PER_SECOND * POTION_DURATION).abs() < 0.05, "total soigné: {healed}");
        assert!(app.world().get::<Regen>(id).is_none() && app.world().get::<Regenerating>(id).is_none());

        // Jamais au-delà du maximum.
        app.world_mut().entity_mut(id).insert((Regen(POTION_DURATION), Regenerating));
        app.world_mut().get_mut::<Health>(id).unwrap().current = 99.0;
        advance(&mut app, 3.0);
        assert_eq!(app.world().get::<Health>(id).unwrap().current, 100.0);
    }

    #[test]
    fn a_downed_hero_is_not_healed_and_loses_the_effect() {
        let mut app = app_with_time();
        app.add_systems(Update, run_regen);
        let id = hero(app.world_mut(), 0, Vec2::ZERO, true, 0.0);
        app.world_mut().entity_mut(id).insert((Regen(POTION_DURATION), Regenerating, Downed));
        advance(&mut app, 2.0);
        assert_eq!(app.world().get::<Health>(id).unwrap().current, 0.0);
        assert!(app.world().get::<Regen>(id).is_none());
    }

    /// Seul un héros dirigé ramasse la potion; elle régénère toute son équipe et aucune autre.
    #[test]
    fn potion_regenerates_the_whole_team_of_whoever_drinks_it() {
        let mut app = app_with_time();
        app.init_resource::<LevelStats>().add_systems(Update, collect_pickups);
        let spot = Vec2::new(40.0, 40.0);
        let leader = hero(app.world_mut(), 0, spot, true, 50.0);
        let companion = hero(app.world_mut(), 0, Vec2::new(-300.0, 0.0), false, 50.0);
        let other_team = hero(app.world_mut(), 1, Vec2::new(300.0, 0.0), true, 50.0);
        let potion = app.world_mut().spawn((Pickup(PickupKind::Potion), Position(spot))).id();
        app.update();
        assert!(app.world().get_entity(potion).is_err(), "la potion est bue");
        assert!(app.world().get::<Regen>(leader).is_some());
        assert!(app.world().get::<Regen>(companion).is_some(), "toute l'équipe est soignée, même loin");
        assert!(app.world().get::<Regen>(other_team).is_none(), "une autre équipe n'est pas soignée");

        // Un compagnon (non dirigé) qui passe dessus ne la ramasse pas.
        let mut app = app_with_time();
        app.init_resource::<LevelStats>().add_systems(Update, collect_pickups);
        let idle = hero(app.world_mut(), 0, spot, false, 50.0);
        let potion = app.world_mut().spawn((Pickup(PickupKind::Potion), Position(spot))).id();
        app.update();
        assert!(app.world().get_entity(potion).is_ok());
        assert!(app.world().get::<Regen>(idle).is_none());
    }

    /// Un château contient beaucoup de monstres d'avance, plusieurs générateurs par salle et des potions.
    #[test]
    fn a_new_level_is_populated_with_monsters_generators_and_potions() {
        for (seed, level) in [(1u64, 1u32), (2, 3), (3, 6), (4, 9)] {
            let dungeon = Dungeon::generate(seed);
            let fights = dungeon.layout.rooms.iter().filter(|r| r.kind == RoomKind::Combat).count();
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, &world);
                populate(&mut commands, &dungeon, seed, level, false, &mut Rng(seed | 1));
            }
            queue.apply(&mut world);
            let monsters = world.query::<&Monster>().iter(&world).count() as u32;
            let generators = world.query::<&Generator>().iter(&world).count();
            let potions = world.query::<&Pickup>().iter(&world).filter(|p| p.0 == PickupKind::Potion).count() as u32;
            assert!(monsters >= fights as u32 * monsters_per_room(level), "niveau {level}: {monsters} monstres pour {fights} salles");
            assert!(generators >= fights, "niveau {level}: {generators} générateurs pour {fights} salles");
            assert!(potions >= potions_in_level(level), "niveau {level}: {potions} potions");
        }
        assert!(monsters_per_room(1) >= 1 && monsters_per_room(99) <= 6);
        assert!(potions_in_level(9) > potions_in_level(1));
    }

    fn trap(world: &mut World, at: Vec2) -> Entity {
        world.spawn((Trap { armed: false }, Position(at), Hold::default(), TrapCooldown::default(), TrapIdle::default())).id()
    }

    fn move_hero(app: &mut App, hero: Entity, at: Vec2) {
        app.world_mut().get_mut::<Position>(hero).unwrap().0 = at;
    }

    fn is_armed(app: &App, trap: Entity) -> bool {
        app.world().get::<Trap>(trap).unwrap().armed
    }

    /// Un piège dort tant que personne n'est proche, se réveille à l'approche d'un héros et se rendort après son départ.
    #[test]
    fn trap_wakes_when_a_hero_comes_close_and_sleeps_again_when_it_leaves() {
        let mut app = app_with_time();
        app.add_systems(Update, run_traps);
        let t = trap(app.world_mut(), Vec2::ZERO);
        let hero = hero(app.world_mut(), 0, Vec2::new(300.0, 0.0), true, 100.0);

        advance(&mut app, 0.1);
        assert!(!is_armed(&app, t), "loin: le piège dort");

        move_hero(&mut app, hero, Vec2::new(TRAP_WAKE_RADIUS - 1.0, 0.0));
        advance(&mut app, 0.1);
        assert!(is_armed(&app, t), "à l'approche: pointes sorties");
        assert_eq!(app.world().get::<Health>(hero).unwrap().current, 100.0, "réveillé mais pas encore marché dessus");

        move_hero(&mut app, hero, Vec2::new(TRAP_WAKE_RADIUS + 20.0, 0.0));
        advance(&mut app, TRAP_SLEEP_DELAY * 0.5);
        assert!(is_armed(&app, t), "il ne se rendort pas tout de suite");
        advance(&mut app, TRAP_SLEEP_DELAY);
        assert!(!is_armed(&app, t), "personne autour: il se rendort");
    }

    #[test]
    fn an_awake_trap_hurts_a_hero_standing_on_it_but_a_sleeping_one_does_not() {
        let mut app = app_with_time();
        app.add_systems(Update, run_traps);
        let t = trap(app.world_mut(), Vec2::ZERO);
        let hero = hero(app.world_mut(), 0, Vec2::new(TRAP_WAKE_RADIUS - 1.0, 0.0), true, 100.0);
        advance(&mut app, 0.1);
        assert!(is_armed(&app, t));
        // Le héros marche sur le piège réveillé.
        move_hero(&mut app, hero, Vec2::new(1.0, 0.0));
        advance(&mut app, 0.1);
        assert_eq!(app.world().get::<Health>(hero).unwrap().current, 100.0 - TRAP_DAMAGE);
        // Pas de nouveau coup avant la fin du délai.
        advance(&mut app, 0.1);
        assert_eq!(app.world().get::<Health>(hero).unwrap().current, 100.0 - TRAP_DAMAGE);
        advance(&mut app, TRAP_COOLDOWN);
        assert_eq!(app.world().get::<Health>(hero).unwrap().current, 100.0 - 2.0 * TRAP_DAMAGE);
    }

    #[test]
    fn early_levels_are_easier_and_difficulty_reaches_full_strength() {
        assert!(ease(1) < ease(2) && ease(2) < ease(3));
        assert_eq!(ease(0), ease(1));
        assert_eq!(ease(10), 1.0);
    }

    #[test]
    fn the_level_summary_counts_time_and_damage_but_not_healing_nor_the_transition() {
        let mut app = app_with_time();
        app.init_resource::<LevelStats>().init_resource::<Transition>().add_systems(Update, track_stats);
        let id = hero(app.world_mut(), 0, Vec2::ZERO, true, 100.0);
        advance(&mut app, 0.1);
        app.world_mut().get_mut::<Health>(id).unwrap().current = 80.0;
        advance(&mut app, 0.1);
        app.world_mut().get_mut::<Health>(id).unwrap().current = 90.0;
        advance(&mut app, 0.1);
        {
            let stats = app.world().resource::<LevelStats>();
            assert!((stats.damage_taken - 20.0).abs() < 1e-3, "{}", stats.damage_taken);
            assert!(stats.seconds > 0.25, "{}", stats.seconds);
        }
        // Une fois l'équipe dans le portail, plus rien ne compte.
        app.world_mut().resource_mut::<Transition>().0 = Some(5.0);
        app.world_mut().get_mut::<Health>(id).unwrap().current = 10.0;
        advance(&mut app, 0.1);
        let stats = app.world().resource::<LevelStats>();
        assert!((stats.damage_taken - 20.0).abs() < 1e-3 && stats.seconds < 0.35);
    }

    /// Du début à la fin: le héros de la bonne classe actionne le mécanisme du niveau, puis la sortie s'ouvre.
    #[test]
    fn the_featured_hero_can_open_the_exit_on_levels_two_four_and_five() {
        for (level, class) in [(2, HeroClass::Forgebraise), (4, HeroClass::Sorcerer), (5, HeroClass::Knight)] {
            let mut app = networked_app();
            let mut save = SaveGame::new("test", GameMode::Solo, 77);
            save.level = level;
            save.intro_done = true;
            app.insert_resource(Rng(5));
            app.insert_resource(Campaign(save));
            app.insert_resource(DialogueSession::default());
            app.insert_resource(Level(level));
            app.init_resource::<LevelStats>().init_resource::<Transition>();
            app.add_systems(Startup, |mut commands: Commands, mut rng: ResMut<Rng>, campaign: Res<Campaign>, mut session: ResMut<DialogueSession>| {
                generate_level(&mut commands, &mut rng, &campaign, &mut session, []);
            });
            app.add_systems(Update, (run_interactions, update_mechanism_quests, open_exit).chain());
            app.update();
            // Plus de monstre ni de générateur; l'équipe a trouvé ce qu'il faut.
            let beasts: Vec<Entity> = {
                let world = app.world_mut();
                let mut query = world.query_filtered::<Entity, Or<(With<Monster>, With<Generator>)>>();
                query.iter(world).collect()
            };
            for beast in beasts {
                app.world_mut().despawn(beast);
            }
            let mut kit = app.world_mut().query::<&mut PuzzleKit>();
            for mut kit in kit.iter_mut(app.world_mut()) {
                kit.ingredients = INGREDIENTS_NEEDED;
                kit.gear = 1;
                kit.target = 1;
            }
            // Le mécanisme de la classe à l'honneur (le panneau, ou la porte à pousser).
            let wanted = match class {
                HeroClass::Forgebraise => InteractKind::Mechanism,
                HeroClass::Sorcerer => InteractKind::Rune,
                HeroClass::Ranger => InteractKind::Target,
                HeroClass::Knight => InteractKind::Push,
            };
            let at = {
                let world = app.world_mut();
                let mut query = world.query::<(&Position, &Interactable)>();
                query.iter(world).find(|(_, i)| i.kind == wanted).map(|(p, _)| p.0).expect("le mécanisme du niveau")
            };
            app.world_mut().spawn((
                Player,
                PartyId(0),
                class,
                Position(at),
                Health { current: 100.0, max: 100.0 },
                Gold::default(),
                Keys::default(),
                Controlled,
                Interacting(true),
            ));
            for _ in 0..12 {
                advance(&mut app, 0.5);
            }
            let open = {
                let world = app.world_mut();
                let mut query = world.query::<&LevelExit>();
                query.iter(world).all(|e| e.open)
            };
            assert!(open, "niveau {level}: la sortie doit être ouverte une fois le mécanisme de {class:?} accompli");
        }
    }

    /// Une scène d'arrivée qui attend des ennemis proches ne fige pas les héros: ils peuvent se battre.
    #[test]
    fn heroes_are_free_to_fight_while_an_intro_scene_waits_for_a_calm_moment() {
        let mut app = App::new();
        app.init_resource::<Transition>().init_resource::<DialogueSession>().add_systems(Update, hold_heroes);
        let hero = app
            .world_mut()
            .spawn((Player, Position(Vec2::ZERO), MoveDir(Vec2::X), Attacking(true), SpecialInput::default(), Interacting::default()))
            .id();
        app.world_mut().spawn((Monster(MonsterKind::Wolf), Position(Vec2::new(60.0, 0.0))));
        app.world_mut().spawn((Position(Vec2::ZERO), DialogueTrigger { node: "gate_intro", radius: 200.0, delay: 2.0, fired: false }));
        app.update();
        assert_eq!(app.world().get::<MoveDir>(hero).unwrap().0, Vec2::X, "un loup est là: on se bat");
        assert!(app.world().get::<Attacking>(hero).unwrap().0);
    }

    #[test]
    fn heroes_stay_put_until_the_intro_scene_is_over() {
        let mut app = App::new();
        app.init_resource::<Transition>().init_resource::<DialogueSession>().add_systems(Update, hold_heroes);
        let hero = app
            .world_mut()
            .spawn((Player, Position(Vec2::ZERO), MoveDir(Vec2::X), Attacking(true), SpecialInput { held: true, queued: true }, Interacting(true)))
            .id();
        let trigger = app
            .world_mut()
            .spawn((Position(Vec2::ZERO), DialogueTrigger { node: "intro", radius: 200.0, delay: 3.0, fired: false }))
            .id();
        app.update();
        assert_eq!(app.world().get::<MoveDir>(hero).unwrap().0, Vec2::ZERO, "retenus avant la scène");
        assert!(!app.world().get::<Attacking>(hero).unwrap().0);
        // La scène est jouée (ou passée): le déclencheur est consommé et les héros sont libres.
        app.world_mut().get_mut::<DialogueTrigger>(trigger).unwrap().fired = true;
        app.world_mut().get_mut::<MoveDir>(hero).unwrap().0 = Vec2::X;
        app.update();
        assert_eq!(app.world().get::<MoveDir>(hero).unwrap().0, Vec2::X);
        // Pendant la scène elle-même, ils restent retenus.
        app.world_mut().resource_mut::<DialogueSession>().active = Some("intro".to_string());
        app.update();
        assert_eq!(app.world().get::<MoveDir>(hero).unwrap().0, Vec2::ZERO);
    }

    #[test]
    fn a_downed_hero_does_not_wake_a_trap() {
        let mut app = app_with_time();
        app.add_systems(Update, run_traps);
        let t = trap(app.world_mut(), Vec2::ZERO);
        let hero = hero(app.world_mut(), 0, Vec2::new(5.0, 0.0), true, 0.0);
        app.world_mut().entity_mut(hero).insert(Downed);
        advance(&mut app, 0.5);
        assert!(!is_armed(&app, t));
    }

    /// Le Rôdeur désarme pour de bon: le piège reste visible, pointes rentrées, et ne se réveille plus.
    #[test]
    fn a_ranger_can_disarm_a_trap_for_good() {
        let mut app = app_with_time();
        app.add_systems(Update, run_traps);
        let t = trap(app.world_mut(), Vec2::ZERO);
        let ranger = app
            .world_mut()
            .spawn((Player, PartyId(0), HeroClass::Ranger, Position(Vec2::new(10.0, 0.0)), Health { current: 100.0, max: 100.0 }, Interacting(true)))
            .id();
        advance(&mut app, TRAP_DISARM_TIME + 0.2);
        assert!(app.world().get::<Disarmed>(t).is_some(), "désarmé");
        assert!(!is_armed(&app, t));
        // Même collé au piège, il ne se réveille plus et ne blesse pas.
        app.world_mut().get_mut::<Interacting>(ranger).unwrap().0 = false;
        move_hero(&mut app, ranger, Vec2::ZERO);
        advance(&mut app, 1.0);
        assert!(!is_armed(&app, t));
        assert_eq!(app.world().get::<Health>(ranger).unwrap().current, 100.0);
        // Il existe toujours: il n'a pas disparu.
        assert!(app.world().get_entity(t).is_ok());
    }

    #[test]
    fn traps_start_asleep_when_a_level_is_generated() {
        let mut total = 0;
        for seed in 1u64..40 {
            let dungeon = Dungeon::generate(seed);
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, &world);
                populate(&mut commands, &dungeon, seed, 2, false, &mut Rng(seed | 1));
            }
            queue.apply(&mut world);
            for trap in world.query::<&Trap>().iter(&world) {
                assert!(!trap.armed, "seed {seed}: un piège naît endormi");
                total += 1;
            }
        }
        assert!(total > 0, "des pièges apparaissent bien dans les niveaux");
    }

    fn populated(seed: u64, level: u32) -> (World, Dungeon) {
        let dungeon = Dungeon::generate(seed);
        let mut world = World::new();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, &world);
            populate(&mut commands, &dungeon, seed, level, false, &mut Rng(seed | 1));
        }
        queue.apply(&mut world);
        (world, dungeon)
    }

    /// Chaque château a ses trois obstacles (un par classe), la herse, l'enchantement, et les pièces à trouver.
    #[test]
    fn every_level_has_the_whole_royal_door_puzzle() {
        for seed in 1u64..60 {
            let (mut world, dungeon) = populated(seed, 1 + (seed % 4) as u32);
            let mut kinds: Vec<_> = world.query::<&Interactable>().iter(&world).map(|i| i.kind).collect();
            kinds.sort_by_key(|k| format!("{k:?}"));
            assert_eq!(kinds.len(), 4, "seed {seed}");
            for expected in [InteractKind::Target, InteractKind::Rune, InteractKind::Mechanism, InteractKind::Push] {
                assert!(kinds.contains(&expected), "seed {seed}: {expected:?} manque");
            }
            assert_eq!(world.query::<&Ward>().iter(&world).count(), 1);
            assert_eq!(world.query::<&Portcullis>().iter(&world).count(), 1);
            assert_eq!(world.query::<&PuzzleKit>().iter(&world).count(), 1);
            let count = |world: &mut World, kind: PickupKind| world.query::<&Pickup>().iter(world).filter(|p| p.0 == kind).count();
            assert_eq!(count(&mut world, PickupKind::Ingredient), INGREDIENTS_NEEDED as usize, "seed {seed}");
            assert_eq!(count(&mut world, PickupKind::GearPart), 1);
            assert_eq!(count(&mut world, PickupKind::TargetDisc), 1);
            // Les panneaux sont dans le mur (jamais au sol), et les pièces sur le sol.
            let room = &dungeon.room;
            let tile = |p: Vec2| {
                let origin = room.tile_center(0, 0);
                (((p.x - origin.x) / TILE_SIZE).round() as i32, ((p.y - origin.y) / TILE_SIZE).round() as i32)
            };
            for (pos, interactable) in world.query::<(&Position, &Interactable)>().iter(&world) {
                if interactable.kind != InteractKind::Push {
                    let (x, y) = tile(pos.0);
                    assert_eq!(room.tile(x, y), Tile::Wall, "seed {seed}: {:?} n'est pas dans un mur", interactable.kind);
                }
            }
            for (pos, pickup) in world.query::<(&Position, &Pickup)>().iter(&world) {
                if matches!(pickup.0, PickupKind::Ingredient | PickupKind::GearPart | PickupKind::TargetDisc) {
                    let (x, y) = tile(pos.0);
                    assert_eq!(room.tile(x, y), Tile::Floor, "seed {seed}: pièce dans un mur");
                }
            }
        }
    }

    /// Chaque scène que le serveur peut lancer existe dans le script de dialogue.
    #[test]
    fn every_dialogue_node_the_server_triggers_exists_in_the_script() {
        const YARN: &str = include_str!("../../client/assets/dialogue/castle.yarn");
        let mut used = std::collections::BTreeSet::new();
        for seed in 1u64..40 {
            let (mut world, _) = populated(seed, 1 + (seed % 3) as u32);
            for trigger in world.query::<&DialogueTrigger>().iter(&world) {
                used.insert(trigger.node);
            }
        }
        for node in ["royal_door", "alcove", "puzzle_rune", "puzzle_gears", "puzzle_target"] {
            assert!(used.contains(node), "la scène {node} n'est jamais déclenchée");
        }
        for node in used {
            assert!(YARN.contains(&format!("title: {node}")), "scène {node} déclenchée mais absente du script");
        }
    }

    /// Un nouveau niveau (ou un retour au début du niveau) purge la scène en cours, les votes en attente et les anciennes zones.
    #[test]
    fn a_new_level_purges_the_dialogue_session_and_the_old_triggers() {
        let mut app = networked_app();
        app.insert_resource(Rng(5));
        app.insert_resource(Campaign(SaveGame::new("test", GameMode::Solo, 77)));
        let mut session = DialogueSession { active: Some("royal_door".into()), idle: 40.0, vote_timer: 3.0, ..default() };
        session.votes.insert(Entity::PLACEHOLDER, 1);
        app.insert_resource(session);
        let old_trigger = app
            .world_mut()
            .spawn((Position(Vec2::ZERO), DialogueTrigger { node: "puzzle_rune", radius: 60.0, delay: 0.3, fired: false }))
            .id();
        app.add_systems(
            Update,
            move |mut commands: Commands, mut rng: ResMut<Rng>, campaign: Res<Campaign>, mut session: ResMut<DialogueSession>| {
                generate_level(&mut commands, &mut rng, &campaign, &mut session, [old_trigger]);
            },
        );
        app.update();
        let session = app.world().resource::<DialogueSession>();
        assert!(session.active.is_none(), "la scène du niveau précédent est abandonnée");
        assert!(session.votes.is_empty() && session.idle == 0.0 && session.vote_timer == 0.0);
        assert!(app.world().get_entity(old_trigger).is_err(), "l'ancienne zone de dialogue a disparu");
    }

    /// Sur chaque niveau de la campagne, une équipe qui a rempli l'objectif et se tient dans la sortie lance le bilan.
    #[test]
    fn every_campaign_level_can_be_left_through_its_exit() {
        for level in 1..=LEVEL_COUNT {
            let mut app = networked_app();
            let mut save = SaveGame::new("test", GameMode::Solo, 77);
            save.level = level;
            save.intro_done = true;
            app.insert_resource(Rng(5));
            app.insert_resource(Campaign(save));
            app.insert_resource(DialogueSession::default());
            app.insert_resource(Level(level));
            app.init_resource::<LevelStats>().init_resource::<Transition>();
            app.add_systems(Startup, |mut commands: Commands, mut rng: ResMut<Rng>, campaign: Res<Campaign>, mut session: ResMut<DialogueSession>| {
                generate_level(&mut commands, &mut rng, &campaign, &mut session, []);
            });
            app.add_systems(Update, (open_exit, run_exit).chain());
            app.update();
            let exit = {
                let world = app.world_mut();
                let mut query = world.query::<(&Position, &LevelExit)>();
                let found: Vec<_> = query.iter(world).map(|(p, e)| (p.0, e.open)).collect();
                assert_eq!(found.len(), 1, "niveau {level}: une seule sortie");
                found[0].0
            };
            // Un joueur connecté qui n'a pas encore validé le bilan.
            app.world_mut().spawn(ActiveHero(Entity::PLACEHOLDER));
            app.world_mut().spawn((
                Player,
                PartyId(0),
                HeroClass::Knight,
                Position(exit),
                Health { current: 100.0, max: 100.0 },
                Gold::default(),
                Keys::default(),
                Controlled,
                Interacting::default(),
            ));
            // L'objectif est rempli: les quêtes sont accomplies, et il ne reste plus de monstre ni de générateur.
            let beasts: Vec<Entity> = {
                let world = app.world_mut();
                let mut query = world.query_filtered::<Entity, Or<(With<Monster>, With<Generator>)>>();
                query.iter(world).collect()
            };
            for beast in beasts {
                app.world_mut().despawn(beast);
            }
            let mut quests = app.world_mut().query::<&mut Quest>();
            for mut quest in quests.iter_mut(app.world_mut()) {
                quest.progress = quest.goal;
            }
            // Les mécanismes du niveau sont accomplis (ce que fait l'équipe avec ses talents).
            let mut interactables = app.world_mut().query::<&mut Interactable>();
            for mut interactable in interactables.iter_mut(app.world_mut()) {
                interactable.done = true;
            }
            for _ in 0..4 {
                advance(&mut app, 1.0);
            }
            if app.world().resource::<Transition>().0.is_none() {
                let world = app.world_mut();
                let mut exits = world.query::<(&Position, &LevelExit)>();
                let state: Vec<_> = exits.iter(world).map(|(p, e)| (p.0, e.open, e.kind)).collect();
                let mut quests = world.query::<&Quest>();
                let q: Vec<_> = quests.iter(world).map(|q| (q.progress, q.goal)).collect();
                panic!("niveau {level}: sortie {state:?}, quêtes {q:?}, héros en {exit:?}");
            }
        }
    }

    /// Une scène de dialogue ne démarre pas tant qu'un monstre est à proximité d'un héros.
    #[test]
    fn dialogues_wait_until_no_enemy_is_near() {
        let mut app = networked_app();
        app.insert_resource(DialogueSession::default());
        app.add_systems(Update, run_dialogue_triggers);
        app.world_mut().spawn((Player, Position(Vec2::ZERO)));
        let monster = app.world_mut().spawn((Monster(MonsterKind::Wolf), Position(Vec2::new(80.0, 0.0)))).id();
        let trigger = app
            .world_mut()
            .spawn((Position(Vec2::ZERO), DialogueTrigger { node: "bridge_intro", radius: 200.0, delay: 0.1, fired: false }))
            .id();
        advance(&mut app, 1.0);
        assert!(!app.world().get::<DialogueTrigger>(trigger).unwrap().fired, "un loup rôde: pas de scène");
        assert!(app.world().resource::<DialogueSession>().active.is_none());
        app.world_mut().despawn(monster);
        advance(&mut app, 1.0);
        assert!(app.world().get::<DialogueTrigger>(trigger).unwrap().fired, "plus d'ennemi: la scène démarre");
        assert_eq!(app.world().resource::<DialogueSession>().active.as_deref(), Some("bridge_intro"));
    }

    /// Les niveaux 2 à 5 mettent chacun une classe à l'honneur; le niveau 1 n'a pas de mécanisme; le 6 est l'énigme complète.
    #[test]
    fn levels_two_to_five_each_feature_one_hero() {
        let expected = [
            (2, QuestKind::Repair, InteractKind::Mechanism, PickupKind::GearPart),
            (3, QuestKind::Release, InteractKind::Target, PickupKind::TargetDisc),
            (4, QuestKind::Unseal, InteractKind::Rune, PickupKind::Ingredient),
        ];
        for (level, quest, kind, supply) in expected {
            let def = level_def(level);
            assert_eq!(def.quest.map(|(k, _)| k), Some(quest), "niveau {level}");
            let panels: Vec<_> = def.things.iter().filter(|t| matches!(t, Thing::Panel { kind: k, .. } if *k == kind)).collect();
            assert_eq!(panels.len(), 1, "niveau {level}: un panneau {kind:?}");
            assert!(def.things.iter().any(|t| matches!(t, Thing::Pickup { kind: k, .. } if *k == supply)), "niveau {level}: de quoi l'actionner");
            assert!(def.things.iter().any(|t| matches!(t, Thing::Trigger { node, .. } if node.ends_with("intro"))), "niveau {level}: une scène d'arrivée");
            // Chaque scène nommée existe bien.
            for thing in &def.things {
                if let Thing::Trigger { node, .. } | Thing::Panel { node, .. } = thing {
                    assert!(dialogue_node(node).is_some(), "niveau {level}: scène inconnue {node}");
                }
            }
        }
        // Niveau 1: une introduction, sans mécanisme.
        assert!(!level_def(1).things.iter().any(|t| matches!(t, Thing::Panel { .. })));
        // Niveau 5: le Chevalier pousse la porte royale, sans garde ni herse; niveau 6: l'énigme complète.
        let knight = level_def(5);
        assert_eq!(knight.quest.map(|(k, _)| k), Some(QuestKind::Breach));
        assert!(!knight.things.iter().any(|t| matches!(t, Thing::Ward { .. } | Thing::Panel { .. } | Thing::Portcullis { .. })));
        assert!(knight.layout.connections.iter().any(|c| c.door == Some(DoorKind::Royal)));
        let full = level_def(6);
        assert!(full.things.iter().any(|t| matches!(t, Thing::Ward { .. })));
        assert_eq!(full.things.iter().filter(|t| matches!(t, Thing::Panel { .. })).count(), 3);
    }

    /// Une application de test qui sait envoyer des événements réseau (secousse d'écran...), sans connexion.
    fn networked_app() -> App {
        let mut app = app_with_time();
        app.add_plugins((StatesPlugin, RepliconPlugins, CastleSharedPlugin));
        app
    }

    fn puzzle_app() -> (App, Entity) {
        let mut app = networked_app();
        app.add_systems(Update, run_interactions);
        let kit = app.world_mut().spawn(PuzzleKit::default()).id();
        (app, kit)
    }

    fn panel(app: &mut App, kind: InteractKind, at: Vec2) -> Entity {
        app.world_mut().spawn((Interactable::new(kind), Position(at), Hold::default())).id()
    }

    fn class_hero(app: &mut App, class: HeroClass, at: Vec2) -> Entity {
        let id = hero(app.world_mut(), 0, at, true, 100.0);
        let mut entity = app.world_mut().entity_mut(id);
        entity.insert(class);
        entity.get_mut::<Interacting>().unwrap().0 = true;
        id
    }

    fn kit_of(app: &App, kit: Entity) -> PuzzleKit {
        *app.world().get::<PuzzleKit>(kit).unwrap()
    }

    /// Chaque obstacle demande sa classe ET ce qu'on a trouvé, qui est consommé; la porte attend les trois autres.
    #[test]
    fn each_obstacle_needs_its_class_and_what_was_found() {
        let (mut app, kit) = puzzle_app();
        let at = Vec2::new(100.0, 100.0);
        let rune = panel(&mut app, InteractKind::Rune, at);
        let gears = panel(&mut app, InteractKind::Mechanism, at);
        let target = panel(&mut app, InteractKind::Target, at);
        let door = panel(&mut app, InteractKind::Push, at);
        let progress = |app: &App, e: Entity| app.world().get::<Interactable>(e).unwrap().progress;
        let done = |app: &App, e: Entity| app.world().get::<Interactable>(e).unwrap().done;

        // Sans rien trouvé, ni l'Ensorceleur ni le Forgebraise ni le Rôdeur n'avancent.
        let sorcerer = class_hero(&mut app, HeroClass::Sorcerer, at);
        advance(&mut app, 3.0);
        assert_eq!(progress(&app, rune), 0.0, "pas d'ingrédients: la rune ne bouge pas");

        // Deux ingrédients sur trois ne suffisent pas.
        app.world_mut().get_mut::<PuzzleKit>(kit).unwrap().ingredients = INGREDIENTS_NEEDED - 1;
        advance(&mut app, 3.0);
        assert!(!done(&app, rune));
        // Trois: l'Ensorceleur dissipe, les ingrédients sont consommés.
        app.world_mut().get_mut::<PuzzleKit>(kit).unwrap().ingredients = INGREDIENTS_NEEDED + 1;
        advance(&mut app, InteractKind::Rune.hold_time() + 0.2);
        assert!(done(&app, rune));
        assert_eq!(kit_of(&app, kit).ingredients, 1, "trois ingrédients consommés, un de trop reste");

        // Le mauvais héros ne répare rien, même avec la pièce.
        app.world_mut().entity_mut(sorcerer).get_mut::<Interacting>().unwrap().0 = false;
        let knight = class_hero(&mut app, HeroClass::Knight, at);
        app.world_mut().get_mut::<PuzzleKit>(kit).unwrap().gear = 1;
        advance(&mut app, 4.0);
        assert!(!done(&app, gears), "un Chevalier ne répare pas le mécanisme");
        assert_eq!(kit_of(&app, kit).gear, 1);
        // La porte est verrouillée tant que la cible et le mécanisme ne sont pas faits.
        assert!(!done(&app, door), "la porte attend les autres obstacles");

        // Le Forgebraise répare avec la pièce.
        app.world_mut().entity_mut(knight).get_mut::<Interacting>().unwrap().0 = false;
        class_hero(&mut app, HeroClass::Forgebraise, at);
        advance(&mut app, InteractKind::Mechanism.hold_time() + 0.2);
        assert!(done(&app, gears));
        assert_eq!(kit_of(&app, kit).gear, 0);

        // Le Rôdeur fixe le disque: la cible est garnie mais pas encore touchée.
        app.world_mut().get_mut::<PuzzleKit>(kit).unwrap().target = 1;
        class_hero(&mut app, HeroClass::Ranger, at);
        advance(&mut app, InteractKind::Target.hold_time() + 0.2);
        let t = *app.world().get::<Interactable>(target).unwrap();
        assert!(t.mounted && !t.done, "disque fixé, flèche attendue");
        assert_eq!(kit_of(&app, kit).target, 0);
        assert!(!done(&app, door), "la cible n'est pas encore touchée");

        // Une fois la cible touchée (par une flèche), le Chevalier peut pousser la porte.
        app.world_mut().get_mut::<Interactable>(target).unwrap().done = true;
        let knight_again = class_hero(&mut app, HeroClass::Knight, at);
        let _ = knight_again;
        advance(&mut app, InteractKind::Push.hold_time() + 0.2);
        assert!(app.world().get_entity(door).is_err(), "la porte cède et disparaît");
    }

    /// Dissiper la rune lève l'enchantement.
    #[test]
    fn dispelling_the_rune_removes_the_ward() {
        let (mut app, kit) = puzzle_app();
        let at = Vec2::new(10.0, 10.0);
        panel(&mut app, InteractKind::Rune, at);
        let ward = app.world_mut().spawn((Ward { radius: WARD_RADIUS }, Position(Vec2::ZERO))).id();
        class_hero(&mut app, HeroClass::Sorcerer, at);
        app.world_mut().get_mut::<PuzzleKit>(kit).unwrap().ingredients = INGREDIENTS_NEEDED;
        advance(&mut app, 0.5);
        assert!(app.world().get_entity(ward).is_ok(), "pas encore");
        advance(&mut app, InteractKind::Rune.hold_time());
        assert!(app.world().get_entity(ward).is_err(), "l'enchantement est dissipé");
    }

    /// Ramasser une pièce la met dans la réserve commune de l'équipe.
    #[test]
    fn found_pieces_go_into_the_shared_kit() {
        let mut app = app_with_time();
        app.init_resource::<LevelStats>().add_systems(Update, collect_pickups);
        let kit = app.world_mut().spawn(PuzzleKit::default()).id();
        let spot = Vec2::new(5.0, 5.0);
        hero(app.world_mut(), 0, spot, true, 100.0);
        for kind in [PickupKind::Ingredient, PickupKind::Ingredient, PickupKind::GearPart, PickupKind::TargetDisc] {
            app.world_mut().spawn((Pickup(kind), Position(spot)));
        }
        for _ in 0..6 {
            app.update();
        }
        assert_eq!(kit_of(&app, kit), PuzzleKit { ingredients: 2, gear: 1, target: 1 });
    }

    /// Une flèche du Rôdeur sur la cible garnie lève la herse; rien d'autre ne le fait.
    #[test]
    fn a_rangers_arrow_on_the_mounted_target_raises_the_portcullis() {
        let dungeon = Dungeon::generate(7);
        let sites = dungeon.layout.puzzle_sites(&dungeon.room);
        let spot = sites.panels[0];
        let make = |class: HeroClass, mounted: bool| {
            let mut app = networked_app();
            app.insert_resource(dungeon.room.clone());
            app.add_systems(Update, move_projectiles);
            let target = app
                .world_mut()
                .spawn((Interactable { mounted, ..Interactable::new(InteractKind::Target) }, Position(spot)))
                .id();
            let herse = app.world_mut().spawn((Portcullis { vertical: true }, Position(sites.portcullis))).id();
            // La flèche part du sol devant le panneau, vers le haut (le panneau est dans le mur du haut).
            let arrow = app
                .world_mut()
                .spawn((
                    Projectile { class, dir: Vec2::Y },
                    Position(spot - Vec2::new(0.0, 24.0)),
                    Velocity(Vec2::Y * 260.0),
                    Damage(8.0),
                    Lifetime(1.0),
                ))
                .id();
            for _ in 0..12 {
                advance(&mut app, 1.0 / 64.0);
            }
            (app, target, herse, arrow)
        };
        let (app, target, herse, arrow) = make(HeroClass::Ranger, true);
        assert!(app.world().get::<Interactable>(target).unwrap().done, "cible touchée");
        assert!(app.world().get_entity(herse).is_err(), "herse levée");
        assert!(app.world().get_entity(arrow).is_err());
        // Une cible sans disque, ou un autre tireur, ne fait rien.
        let (app, target, herse, _) = make(HeroClass::Ranger, false);
        assert!(!app.world().get::<Interactable>(target).unwrap().done);
        assert!(app.world().get_entity(herse).is_ok());
        let (app, target, herse, _) = make(HeroClass::Sorcerer, true);
        assert!(!app.world().get::<Interactable>(target).unwrap().done, "un sort n'est pas une flèche");
        assert!(app.world().get_entity(herse).is_ok());
    }

    /// L'enchantement repousse un héros qui s'approche trop de la porte, et ignore celui qui est loin.
    #[test]
    fn the_ward_pushes_heroes_away_from_the_door() {
        let dungeon = Dungeon::generate(7);
        let sites = dungeon.layout.puzzle_sites(&dungeon.room);
        let outward = (sites.portcullis - sites.door).normalize();
        let mut app = app_with_time();
        app.insert_resource(dungeon.room.clone());
        app.add_systems(Update, run_ward);
        app.world_mut().spawn((Ward { radius: WARD_RADIUS }, Position(sites.door)));
        let near = hero(app.world_mut(), 0, sites.door + outward * 30.0, true, 100.0);
        let far = hero(app.world_mut(), 1, sites.door + outward * (WARD_RADIUS + 40.0), true, 100.0);
        let far_before = app.world().get::<Position>(far).unwrap().0;
        let before = app.world().get::<Position>(near).unwrap().0.distance(sites.door);
        for _ in 0..30 {
            advance(&mut app, 1.0 / 30.0);
        }
        let after = app.world().get::<Position>(near).unwrap().0.distance(sites.door);
        assert!(after > before + 10.0, "repoussé de {before} à {after}");
        assert_eq!(app.world().get::<Position>(far).unwrap().0, far_before, "loin de l'enchantement: rien ne bouge");
    }

    /// Maintenir Entrée purge la scène en cours et celles qui allaient démarrer, pas celles qui sont loin.
    #[test]
    fn purging_dialogues_cancels_the_current_scene_and_the_ones_about_to_start() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let here = Vec2::new(10.0, 0.0);
        let near = world.spawn((Position(here), DialogueTrigger { node: "royal_door", radius: 60.0, delay: 0.3, fired: false })).id();
        let far = world.spawn((Position(Vec2::new(900.0, 0.0)), DialogueTrigger { node: "puzzle_rune", radius: 60.0, delay: 0.3, fired: false })).id();
        let done = world.spawn((Position(here), DialogueTrigger { node: "intro", radius: 60.0, delay: 0.0, fired: true })).id();
        let mut session = DialogueSession { active: Some("intro".into()), ..default() };
        let (was_active, skipped_intro) = world
            .run_system_once(move |mut triggers: Query<(&Position, &mut DialogueTrigger)>| {
                let result = purge_dialogues(&mut session, false, &mut triggers, &[here]);
                assert!(session.active.is_none(), "la scène en cours est abandonnée");
                result
            })
            .unwrap();
        assert!(was_active && skipped_intro, "l'introduction purgée est signalée");
        assert!(world.get::<DialogueTrigger>(near).unwrap().fired, "la scène sur le point de partir est purgée");
        assert!(!world.get::<DialogueTrigger>(far).unwrap().fired, "une scène lointaine reste disponible");
        assert!(world.get::<DialogueTrigger>(done).unwrap().fired);
    }

    /// Un compagnon seul, avec les vraies collisions, doit atteindre chaque salle du château.
    #[test]
    fn follower_reaches_every_room() {
        let dt = 1.0 / 64.0;
        let speed = 75.0;
        for seed in 0..25 {
            let dungeon = Dungeon::generate(seed);
            let (room, layout) = (&dungeon.room, &dungeon.layout);
            for target in 0..layout.rooms.len() {
                let goal = tile_to_world(&dungeon.room, layout.rooms[target].center());
                let mut pos = tile_to_world(&dungeon.room, layout.rooms[layout.start].center());
                let mut route = Route::default();
                let mut arrived = false;
                for _ in 0..(60 * 64) {
                    if pos.distance(goal) < 12.0 {
                        arrived = true;
                        break;
                    }
                    let dir = steer(room, &[], &mut route, pos, goal, dt);
                    pos = room.move_and_collide(pos, dir * speed * dt, HERO_HALF_SIZE);
                }
                assert!(arrived, "seed {seed}: bloqué en route vers la salle {target}, à {pos}");
            }
        }
    }
}
