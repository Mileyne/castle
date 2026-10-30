//! Serveur dédié de Castle, en programme autonome (ou lancé par le client sur ordinateur).

use castle_server::{Options, run};

fn main() {
    // Lancé par le client: on s'arrête dès que son tube d'entrée se ferme (client fermé ou tué).
    if std::env::var_os("CASTLE_EMBEDDED").is_some() {
        std::thread::spawn(|| {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            std::process::exit(0);
        });
    }
    run(Options::from_args(), None);
}
