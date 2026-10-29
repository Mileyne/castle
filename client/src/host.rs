//! Lancement du serveur local: le client démarre lui-même le serveur d'une partie, et l'arrête à sa fermeture.
//!
//! Le serveur reste un programme à part (`castle_server`, placé à côté du client), prêt à servir un jour de serveur
//! autonome. Solo et coopératif utilisent le même programme: seul le mode change, et avec lui l'ouverture au réseau.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use castle_shared::DEFAULT_PORT;
use castle_shared::save::GameMode;

/// Le serveur lancé par ce client: il est arrêté quand le client se ferme (ou quand la ressource disparaît).
#[derive(Resource)]
pub struct LocalServer(Child);

impl Drop for LocalServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Laisse au serveur le temps d'ouvrir son port avant que le client ne s'y connecte.
const SERVER_STARTUP: Duration = Duration::from_millis(1500);

/// Connexion à ouvrir dès que `at` est passé.
#[derive(Resource)]
pub struct ConnectAt {
    pub addr: SocketAddr,
    pub at: Instant,
}

impl ConnectAt {
    pub fn now(addr: SocketAddr) -> Self {
        Self { addr, at: Instant::now() }
    }

    pub fn after_server_start(addr: SocketAddr) -> Self {
        Self { addr, at: Instant::now() + SERVER_STARTUP }
    }
}

/// Démarre le serveur de la partie `save`. L'erreur est la clé du message à afficher.
pub fn start_local_server(mode: GameMode, save: &str) -> Result<LocalServer, &'static str> {
    // Port déjà pris: un autre serveur (ou une autre partie) tourne déjà.
    if UdpSocket::bind(("0.0.0.0", DEFAULT_PORT)).is_err() {
        return Err("menu-port-busy");
    }
    let name = if cfg!(windows) { "castle_server.exe" } else { "castle_server" };
    let path = std::env::current_exe().map_err(|_| "menu-server-missing")?.with_file_name(name);
    Command::new(&path)
        .args(["--mode", mode.arg(), "--save", save])
        .env("CASTLE_EMBEDDED", "1")
        // Le serveur s'arrête quand ce tube se ferme, même si le client est tué sans fermeture propre.
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(LocalServer)
        .map_err(|err| {
            eprintln!("serveur local introuvable ({}): {err}", path.display());
            "menu-server-missing"
        })
}

/// Adresse de cette machine sur le réseau local, à donner aux autres joueurs. Aucun paquet n'est envoyé: on
/// demande seulement au système par quelle interface il sortirait.
pub fn lan_address() -> Option<SocketAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip: IpAddr = socket.local_addr().ok()?.ip();
    Some(SocketAddr::new(ip, DEFAULT_PORT))
}
