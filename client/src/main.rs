//! Programme `castle_client` (ordinateur).

fn main() {
    // Outils de niveaux (sans fenêtre):
    //   `castle_client --export-levels <dossier> [--force] [--only 1,2,3]`: écrit les niveaux de départ de la campagne;
    //   `castle_client --decorate <dossier> [--force]`: sème le décor au sol des niveaux.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let force = args.iter().any(|a| a == "--force");
    if let Some(i) = args.iter().position(|a| a == "--export-levels") {
        let dir = args.get(i + 1).expect("usage: castle_client --export-levels <dossier> [--force] [--only 1,2,3]");
        // `--only 1,2,3` limite l'export à ces niveaux.
        let only: Option<Vec<u32>> = args
            .iter()
            .position(|a| a == "--only")
            .and_then(|i| args.get(i + 1))
            .map(|list| list.split(',').filter_map(|n| n.trim().parse().ok()).collect());
        castle_client::sim::export_levels(std::path::Path::new(dir), force, only.as_deref()).expect("export des niveaux");
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--decorate") {
        let dir = args.get(i + 1).expect("usage: castle_client --decorate <dossier> [--force]");
        castle_client::sim::decorate_levels(std::path::Path::new(dir), force).expect("décor des niveaux");
        return;
    }
    castle_client::run();
}
