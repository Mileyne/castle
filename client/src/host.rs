//! Lancement unique: le client démarre lui-même le serveur local, et l'arrête à sa fermeture.
//!
//! Le serveur reste un programme à part (`castle_server`, placé à côté du client), prêt à servir un jour de serveur
//! autonome. Le client ne le lance que si aucune adresse n'est donnée et si personne n'écoute déjà sur le port.

use std::net::UdpSocket;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use castle_shared::DEFAULT_PORT;

/// Le serveur lancé par ce client: il est arrêté quand le client se ferme.
pub struct LocalServer(Child);

impl Drop for LocalServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Démarre le serveur local si nécessaire. `None` si on se connecte à un autre serveur, si un serveur tourne déjà
/// sur ce port, ou si le programme serveur est introuvable.
pub fn start_local_server() -> Option<LocalServer> {
    // Une adresse explicite désigne un serveur distant.
    if std::env::args().skip(1).any(|a| !a.starts_with("--")) || std::env::args().any(|a| a == "--no-server") {
        return None;
    }
    // Port déjà pris: un serveur tourne déjà, on s'y connecte.
    if UdpSocket::bind(("0.0.0.0", DEFAULT_PORT)).is_err() {
        return None;
    }
    let name = if cfg!(windows) { "castle_server.exe" } else { "castle_server" };
    let path = std::env::current_exe().ok()?.with_file_name(name);
    let child = match Command::new(&path)
        .env("CASTLE_EMBEDDED", "1")
        // Le serveur s'arrête quand ce tube se ferme, même si le client est tué sans fermeture propre.
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!("serveur local introuvable ({}): {err}", path.display());
            return None;
        }
    };
    // Laisse au serveur le temps d'ouvrir son port avant que le client ne se connecte.
    std::thread::sleep(Duration::from_millis(1500));
    Some(LocalServer(child))
}
