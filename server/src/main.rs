//! Serveur dédié de Castle, en programme autonome (ou lancé par le client sur ordinateur).

use castle_server::{Options, run};

fn main() {
    // `castle_server --export-levels <dossier> [--force]`: écrit les niveaux de départ de la campagne.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--export-levels") {
        let dir = args.get(i + 1).expect("usage: castle_server --export-levels <dossier> [--force] [--only 1,2,3]");
        // `--only 1,2,3` limite l'export à ces niveaux.
        let only: Option<Vec<u32>> = args
            .iter()
            .position(|a| a == "--only")
            .and_then(|i| args.get(i + 1))
            .map(|list| list.split(',').filter_map(|n| n.trim().parse().ok()).collect());
        castle_server::export_levels(std::path::Path::new(dir), args.iter().any(|a| a == "--force"), only.as_deref()).expect("export des niveaux");
        return;
    }
    // Lancé par le client: on s'arrête dès que son tube d'entrée se ferme (client fermé ou tué).
    if std::env::var_os("CASTLE_EMBEDDED").is_some() {
        std::thread::spawn(|| {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            std::process::exit(0);
        });
    }
    run(Options::from_args(), None);
}
