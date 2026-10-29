# Castle

Jeu multijoueur en ligne inspiré de *Gauntlet*, ambiance RPG pixelisée à l'ancienne.

## Consignes de départ

- Jeu nommé **Castle**, inspiré de Gauntlet.
- **Multijoueur et connecté.**
- Ambiance **RPG pixel-art à l'ancienne**.
- Choisir un **moteur de jeu adéquat**, avec le **maximum de code en Rust**.
- Le concept ci-dessous sert de trame de départ.

## Décisions techniques

| Sujet | Choix | Remarque |
|---|---|---|
| Moteur | **Bevy 0.19** (Rust, ECS) | 0.20 existe mais replicon/renet ne le supportent pas encore |
| Réseau | `bevy_replicon` 0.44 + `bevy_replicon_renet` 0.20 | Serveur autoritaire, les clients envoient leurs entrées |
| Structure | Workspace Cargo: `shared`, `server`, `client` | Serveur headless, client avec rendu |
| Rendu | 2D, tuiles de 16 px, échelle entière; héros rendus en 96 px | Caméra qui suit le héros; sprites provisoires générés par `tools/gen_sprites.py` (planches dans `client/assets/art/`), à remplacer par du pixel-art dessiné au même découpage |
| Localisation | **Fluent** via `bevy_fluent` 0.15 | Fichiers `.ftl` par langue dans `client/assets/locales/<langue>/`; FR et EN au départ |
| Dialogues | **Yarn Spinner** via `bevy_yarnspinner` 0.9 | Scripts `.yarn` dans `client/assets/dialogues/`, tables de chaînes traduisibles par langue |
| Niveaux | Génération procédurale par salles assemblées (à confirmer) | Alternative: niveaux écrits à la main (Tiled) |

### Règles de localisation (à respecter dès le premier jour)

- **Aucun texte affiché n'est écrit en dur dans le code.** Le code ne manipule que des clés (`class-knight`, `quest-hunt-minotaur`...). Voir `name_key` et `special_key` dans `shared/src/lib.rs`.
- **Le serveur n'envoie jamais de texte**, seulement des clés et des paramètres. Chaque joueur d'une même partie peut ainsi jouer dans sa propre langue.
- Les messages à variables (pluriels, genres, nombres) passent par Fluent, pas par du `format!`.
- Les dialogues sont des scripts Yarn: les répliques ont un identifiant stable, et chaque langue a sa table de chaînes. Les choix de dialogue renvoient au serveur un identifiant, pas un texte.
- La langue se choisit dans les options, avec détection de la langue du système par défaut.
- Prévoir dès le départ: police avec accents et caractères étendus, interface qui supporte des textes plus longs, et une police pixel-art adaptée aux langues non latines si on les ajoute plus tard (japonais, chinois, russe).
- Les noms de lieux, de boss et d'artefacts sont aussi des clés, jamais des chaînes dans les données.

---

## Concept

Depuis des siècles, le monde est protégé par les Châteaux-Anciens, d'immenses forteresses magiques construites pour emprisonner des artefacts et des entités surnaturelles.

Mais le plus grand d'entre eux, le **Castle Noir**, est tombé. Son cœur magique s'est fissuré et a ouvert des portails vers différentes régions du château :

- les cryptes oubliées ;
- les jardins maudits ;
- les bibliothèques vivantes ;
- les forges infernales ;
- les tours astrales ;
- les appartements royaux hantés.

Chaque expédition consiste à pénétrer dans le château pour accomplir une quête, récupérer un artefact, vaincre un boss, sauver un prisonnier ou refermer une faille magique.

**Le château se reconfigure entre deux parties**, ce qui donne une forte rejouabilité.

## Les 4 héros fondateurs

Chaque personnage doit être immédiatement identifiable à l'écran et utile en coopération.

### 1. Le Chevalier — Tank / protecteur
- **Caractère :** loyal, courageux, têtu, méfiant envers la magie.
- **Attaque principale :** épée et bouclier.
- **Pouvoir spécial : Rempart** — mur protecteur réduisant les dégâts reçus par l'équipe.
- **Passifs :** plus de points de vie, résistance à la peur, peut forcer certaines portes.
- **Évolutions :** Paladin lumineux, Gardien de fer, Croisé du roi.

### 2. L'Ensorceleur — Dégâts magiques
- **Caractère :** curieuse, sarcastique, ambitieuse, fascinée par le château.
- **Attaque principale :** projectiles magiques.
- **Pouvoir spécial : Tempête Arcane** — explosion de magie touchant toute une zone.
- **Passifs :** détecte les secrets, révèle les pièges magiques, régénération lente de mana.
- **Évolutions :** Sorcière runique, Archimage, Tempête vivante.

### 3. Le Rôdeur — Distance / exploration
- **Caractère :** pragmatique, solitaire, ironique, grand observateur.
- **Attaque principale :** arc ou arbalète.
- **Pouvoir spécial : Pluie de Flèches** — grande zone de dégâts.
- **Passifs :** détection des passages secrets, bonus de critique, désamorçage de pièges.
- **Évolutions :** Chasseur royal, Ombre des bois, Maître traqueur.

### 4. Le Forgebraise — Support offensif
- **Caractère :** joyeux, impulsif, amateur d'explosifs, grand artisan.
- **Attaque principale :** marteau runique.
- **Pouvoir spécial : Tourelle automatique** — déploie une machine défensive.
- **Passifs :** réparation d'objets, fabrication de bombes, accès à certaines salles mécaniques.
- **Évolutions :** Maître ingénieur, Alchimiste de guerre, Forge-tempête.

## Mécanique de coopération

Certaines situations nécessitent plusieurs classes, pour que ce soit de la vraie coopération et non quatre personnages qui frappent les mêmes monstres.

**Exemple, la porte royale :**
1. le Rôdeur trouve le levier caché ;
2. l'Ensorceleur dissipe une rune ;
3. le Forgebraise répare le mécanisme ;
4. le Chevalier pousse la porte.

## Boss principaux

| Boss | Description | Pouvoirs |
|---|---|---|
| **Le Roi Sans Visage** | Ancien souverain du château | Vole le visage des héros, copie leurs compétences |
| **La Bibliothécaire Éternelle** | Tour remplie de livres vivants | Invoque les connaissances interdites, modifie les règles du combat en cours |
| **Le Jardinier Noir** | Fusion entre homme et végétal | Transforme l'arène en forêt hostile, invoque des racines géantes |
| **Le Dragon de la Forge** | Gardien des profondeurs | Lave en fusion, armure quasi indestructible |

## Types de quêtes

- **Chasse :** éliminer une créature précise (ex. tuer le Minotaure des sous-sols).
- **Récupération :** trouver un objet (ex. retrouver la Couronne de Verre).
- **Sauvetage :** libérer des prisonniers (ex. sauver trois érudits perdus dans la bibliothèque maudite).
- **Défense :** protéger un point stratégique (ex. défendre le rituel de fermeture d'un portail).
- **Expédition longue :** mission de 45 à 60 minutes (ex. atteindre le sommet de la Tour Astrale).

## Artefacts légendaires

Chaque partie peut permettre de trouver un artefact unique.

- **Lame de l'Aube :** dégâts contre les morts-vivants.
- **Miroir des Mille Reflets :** crée un double du héros.
- **Couronne Brisée :** améliore les capacités de groupe.
- **Cœur du Château :** artefact ultime au centre du jeu.

## Twist narratif

Le château est lui-même vivant. Au fil des quêtes, les héros découvrent que :

- le château pense ;
- le château les observe ;
- le château choisit quels héros laisser progresser ;
- le véritable ennemi n'est pas un démon mais le château lui-même.

**Boss final : Castle.** La forteresse entière prend vie et utilise contre les joueurs toutes les créatures, pièges et salles rencontrés durant leurs précédentes aventures.

Thème central : quatre héros essaient de comprendre la volonté d'un château vivant qui les manipule depuis le début.

---

## Plan de développement

Chaque phase se termine par quelque chose de jouable et testable.

### Phase 0 — Fondations *(fait)*
- [x] Choix du moteur et de la pile réseau
- [x] Workspace `shared` / `server` / `client` qui compile
- [x] Types partagés: `HeroClass`, `PlayerInput`
- [x] Dépendances de localisation et de dialogues, fichiers de langue FR/EN d'exemple
- [ ] CI de base (`cargo check`, `clippy`, `fmt`)

### Lancer le jeu

```sh
cargo run -p castle_server              # serveur sur le port UDP 5000 (ou: cargo run -p castle_server -- 6000)
cargo run -p castle_client              # client, se connecte à 127.0.0.1:5000
cargo run -p castle_client -- IP:PORT   # client vers un autre serveur
```
Dans le client: **ZQSD/WASD ou flèches** pour se déplacer, **Espace** pour attaquer, **E** pour le pouvoir spécial, **F** (maintenue) pour agir, **Entrée** pour avancer dans un dialogue (1-4 pour voter), **1 à 4** pour prendre la main sur un des quatre héros (Chevalier, Ensorceleur, Rôdeur, Forgebraise), **F1** pour changer de langue (FR/EN). Votre héros a un liseré blanc.

### Phase 1 — Prototype réseau *(terminée, à valider en jeu)*
Objectif: 2 à 4 joueurs se déplacent dans une même salle.
- [x] Serveur headless avec boucle à pas fixe
- [x] Client: fenêtre, caméra pixel-perfect, connexion à une adresse
- [x] Envoi des entrées, déplacement autoritaire côté serveur
- [x] Réplication des joueurs, interpolation côté client
- [x] Lobby minimal avec choix de classe (placeholders colorés)
- [x] Plugin de localisation: chargement des `.ftl`, langue du système, changement de langue à chaud
- [x] Première interface (lobby) entièrement traduite en FR et EN, sans texte en dur

### Fait en plus, hors phases
- [x] Équipe de quatre héros par joueur: on en dirige un à la fois (touches 1 à 4), les trois autres suivent en formation, combattent seuls avec leur arme de base (mêlée ou tir à distance; les pouvoirs spéciaux sont réservés au héros dirigé), relèvent les alliés à terre et ramassent le butin. Ils trouvent leur chemin dans le château (A* sur la grille de tuiles, raccourcis en ligne droite, portes fermées prises en compte), et dans les couloirs étroits ils collent au héros dirigé. Le contrôle passe tout seul à un autre héros si le héros dirigé tombe. Or et clés sont communs à l'équipe.
- [x] Corps solides: héros, monstres et générateurs ne se traversent plus (les héros se traversent entre eux)
- [x] Caméra qui suit le héros, échelle entière (pixels nets), bornée par la salle
- [x] Salle de test agrandie (64x40 tuiles) pour le défilement
- [x] Direction artistique manga: tous les sprites sont rendus par `tools/sprite_forge` (Rust), un petit moteur qui éclaire des volumes (sphères, capsules, boîtes, cônes) par paliers avec ombres hachurées, reflets brillants et contour d'encre teinté. Héros (grands yeux, mèches, écharpes et capes qui flottent), ennemis (squelette, chauve-souris) et objets sont rendus en 96 px, animés (repos, marche à 8 images dont la phase suit la distance parcourue), depuis 8 directions pour les personnages; les armes restent dans la bonne main. Tuiles de sol, de dessus et de face de mur en 48 px, avec un style et des décors propres aux six régions
- [x] Troisième jeu d'art, le **mode médiéval** (visages des sprites paramétrés d'après les photos: cheveux, barbe, yeux, sourcils, nez, bouche, lunettes rectangulaires du Rôdeur), inspiré de quatre portraits fournis (dossier `photos/`): le Chevalier (tabard crème à bandes rouges et lion, mailles et plates, écu en amande, épée longue), l'Ensorceleur (chapeau violet à pointe retombante et broche solaire, robe brodée de lunes et d'étoiles, bâton torsadé à anneau et orbe), le Rôdeur (cape verte à capuche, broche en feuille, carquois à plumes, brassards gravés, grand arc recourbé) et le Forgebraise (casque de fer à nasal, tablier de cuir, ceinture à boucle d'enclume avec tenailles, énorme masse). Proportions plus réalistes; les ennemis, objets et décors reprennent le style manga
- [x] Écran titre: l'illustration `photos/castle.jpg` (copiée en `client/assets/title.png`) couvre la fenêtre. Entrée/clic reprend la partie en cours sur le serveur, N demande une nouvelle partie (événement `NewGame`: nouveau château, héros au départ, or et clés à zéro; vaut pour tous les joueurs connectés). Le jeu et le dialogue d'intro attendent le choix. Le héros actif est dessiné au premier plan
- [x] Annonce de niveau: écran noir « Niveau N » + nom de la région (fondu de 0,8 s après 2,2 s), au lancement de la partie (une fois l'écran titre fermé) et à chaque passage de portail. Le niveau est numéroté par le serveur (`Expedition.level`, 1 au départ ou en nouvelle partie, +1 par portail); les touches de jeu et le dialogue attendent la fin de l'annonce
- [x] Lancement unique: `./play.sh` (ou `castle_client`) démarre aussi le serveur local, qui s'arrête avec le client; avec une adresse en argument ou `--no-server`, le client se connecte à un serveur distant. Le serveur reste un programme à part, prêt pour le futur serveur autonome multi-clients
- [x] Un seul style: le mode médiéval. Les jeux manga et sanglant restent générés mais ne sont plus chargés; F2 et les options `--gore`/`--medieval` sont désactivés
- [x] Vignettes des héros: recadrages directs des photos de référence, détourés (fond effacé, bord ovale adouci), affichés en colonne à gauche de l'écran (`photos/`, identiques dans tous les styles; coordonnées dans `sprite_forge/src/main.rs`), affichées dans le panneau d'équipe et à côté du nom de celui qui parle dans les dialogues
- [x] L'Ensorceleuse devient **l'Ensorceleur**, dans les trois styles, les textes et les dialogues
- [x] Second jeu d'art, le **mode sanglant** (sombre, gore, personnages loufoques à la Gorillaz): héros à énormes têtes, yeux dépareillés, grandes bouches recousues et dentées, corps filiformes et armes macabres (épée rouillée et bouclier à crâne, bâton-colonne vertébrale, arc en côtes, merlin-hachoir); ennemis éventrés (boyaux, œil qui pend); objets macabres (autel au cœur battant, œil au portail, clé en os); décor aux murs ornés de crânes, de chaînes et de crochets de boucher; flaques et ossements au sol; éclairage plus sombre et rougeâtre; giclées de sang et mares quand on frappe ou qu'un ennemi meurt. Choix au lancement (`--gore`, `--medieval` ou `CASTLE_THEME=...`) et bascule en direct avec **F2**, qui passe de manga à sanglant, puis à médiéval. Les deux thèmes sont générés par `sprite_forge` dans `client/assets/art/manga/` et `art/gore/`
- [x] Animations d'arme articulées (cinématique inverse des bras, poses clés par classe, 6 images): coup d'épée avec traînée de lame, sort avec cercle magique et orbe qui grossit, tir à l'arc avec corde tendue et flèche encochée, coup de marteau à deux mains avec onde de choc. Le serveur signale chaque coup (compteur répliqué) et l'effet (dégât, flèche partie) tombe à l'instant de la frappe
- [x] Éclairage: une couche sombre sur la vue, trouée par des halos lisses autour des héros et des sources lumineuses (générateurs, portail, tourelles), via un shader
- [x] Usure du sol: de petites taches sombres et fissures posées sur le sol à l'échelle du monde (non liées à une tuile), propres à chaque région
- [x] Projectiles orientés (orbe magique / crâne enflammé, flèche / flèche d'os, carreau / morceau de viande) avec traînée, et effets de coups (croissant d'épée, onde de marteau), dans les deux styles, rendus par `sprite_forge`. Plus aucun script Python: tout l'art est généré en Rust
- [x] Sols en grandes dalles de pierre irrégulières et murs en gros blocs de château, dessinés en motifs de 4x4 et 4x1 tuiles dont les joints traversent les bordures de tuiles (plus d'effet carrelage). En mode sanglant, plus de flaques par tuile: seules les grandes flaques posées par le jeu restent
- [x] Colorimétrie selon le héros dirigé: le décor prend la couleur de sa tenue (gris pour le Chevalier, violet pour l'Ensorceleur, vert pour le Rôdeur, orange pour le Forgebraise) par une teinte multipliée sur les tuiles (les personnages gardent leurs couleurs) et un voile sur toute l'image (qui désature en gris pour le Chevalier), avec une transition fluide quand on change de héros. Le pourtour de la vue est assombri en plus
- [x] Flou léger du décor loin du héros dirigé: quatre versions de chaque tuile (nette, puis trois niveaux de flou) choisies par anneaux de distance autour de la caméra (lissée), avec un flou qui boucle sur les bords des grands motifs pour éviter les coutures
- [x] Ambiance de donjon plus sombre dans les deux modes (pénombre plus épaisse, pierres assombries), dessus des murs en très grosses pierres presque noires (plus de damier). Interface: plus de marqueur ni de barres de vie au-dessus des héros; la barre de mana rejoint le panneau d'équipe
- [x] Pierres crédibles: dalles et blocs taillés à la main (arêtes chanfreinées et usées, surface au grain fin, piqûres, crevasses)
- [x] Animation de passage entre les modes (F2): sang qui coule en bandes vers le mode sanglant, éclat clair vers le mode manga, échange des sprites au plus fort et petit à-coup
- [x] Tremblement d'écran léger (environ une seconde), signalé par le serveur: générateur détruit, étape d'énigme résolue, porte royale qui cède, porte à clé ouverte
- [x] Zoom d'ambiance: la caméra se rapproche en douceur (x1,4) quand le héros dirigé est dans un passage étroit entre deux salles

### Phase 2 — Combat de base
Objectif: tuer des monstres à plusieurs.
- [x] Tuiles, murs, collisions (salle de test partagée serveur/clients)
- [x] Attaque principale des 4 classes
- [x] PV, dégâts, mort et réanimation (à terre, relevé par un allié proche; tous à terre = salle réinitialisée)
- [x] 2 types de monstres avec IA simple (squelette, chauve-souris), générateurs de monstres façon Gauntlet
- [x] Butin: clés, nourriture, pièces (les clés ne servent pas encore: portes en phase 3)

### Phase 2b — Dialogues *(en grande partie fait)*
Objectif: les personnages parlent, dans la langue de chaque joueur.
- [x] Boîte de dialogue pixel-art, texte progressif, choix (portrait à venir avec le vrai pixel-art)
- [x] Intégration Yarn Spinner côté client, avancement des dialogues piloté par le serveur
- [x] Dialogues partagés: tous les joueurs voient la même scène, chacun dans sa langue
- [x] Règle de vote ou de décision quand un choix engage le groupe
- [ ] Répliques de caractère propres à chaque héros (réactions, remarques entre eux)
- [ ] Outil de vérification: clés manquantes dans une langue, textes trop longs

### Phase 3 — Pouvoirs et coopération
Objectif: les classes ont chacune un rôle.
- [x] Pouvoirs spéciaux: Rempart, Tempête Arcane, Pluie de Flèches, Tourelle (touche E)
- [x] Mana et temps de recharge
- [x] Passifs (détection de secrets et de pièges par l'Ensorceleur et le Rôdeur, désamorçage par le Rôdeur, porte royale poussée par le Chevalier; portes à clé)
- [x] Premier puzzle coopératif: la porte royale (levier caché, rune, mécanisme, puis poussée)

### Phase 4 — Le château procédural
Objectif: une expédition complète et rejouable.
- [x] Château procédural: grille 4x3 de salles de tailles variées reliées par des couloirs, générée depuis une graine partagée serveur/clients (tests sur 200 graines)
- [ ] Les 6 régions avec identité visuelle et ennemis propres (noms et teintes de tuiles faits; tuiles, ennemis et boss propres à venir)
- [x] Portail de sortie derrière la porte royale (la sortie est une impasse); porte à clé devant le trésor
- [x] Reconfiguration du château: quand tous les héros debout entrent dans le portail, un nouveau château et une nouvelle région sont tirés

### Phase 5 — Quêtes et boss
Objectif: de vrais objectifs de mission.
- [x] Système de quêtes (première version): un objectif par niveau, en rotation chasse (6 monstres, +2 tous les 3 niveaux) → récupération (3 reliques) → sauvetage (2 prisonniers, à rester près d'eux 1,2 s). Entité répliquée `Quest`, affichée en haut de l'écran; le portail de sortie n'apparaît qu'une fois l'objectif rempli (secousse d'écran). Si plus aucun monstre ni générateur ne subsiste, la chasse est réputée accomplie. Sprites des reliques et prisonniers provisoires. À venir: défense, expédition longue
- [ ] Les 4 boss avec leurs mécaniques
- [ ] Artefacts légendaires
- [ ] Récompenses de fin de mission

### Phase 5b — Modes de jeu et sauvegardes *(première version, menu à valider en jeu)*
- [x] Parties nommées, listées séparément en solo et en coopératif (`shared/src/save.rs`): fichier RON par partie dans le dossier de données de l'utilisateur (`~/Library/Application Support/Castle/saves` sous macOS, `CASTLE_SAVE_DIR` pour le changer), écriture atomique, copie `.bak`, version de format
- [x] Sauvegarde à chaque entrée de niveau, pas en cours de salle: elle garde le niveau, l'état du générateur aléatoire à l'entrée (qui régénère le même château), l'or et les clés de chaque équipe, et si l'intro a été vue
- [x] Équipe entièrement à terre: retour au début du niveau (même château, monstres revenus, or et clés de l'entrée)
- [x] Menu titre: solo, coopératif (héberger / rejoindre par adresse IP du réseau local), quitter; liste des parties avec « nouvelle partie », suppression avec confirmation. Le client lance le serveur de la partie (`--mode solo|coop --save NOM`); solo n'écoute que `127.0.0.1` pour un joueur, coopératif écoute le réseau local (4 joueurs) et l'hôte voit l'adresse à communiquer. Une adresse en argument rejoint directement un serveur
- [ ] Les équipes retrouvent leur bourse dans l'ordre d'arrivée; une identité de joueur persistante (UUID) sera nécessaire dès qu'il y aura de l'XP par héros
- [x] Menu de pause (Échap): reprendre, ou abandonner le niveau et revenir au menu principal (la progression depuis l'entrée du niveau est perdue). En solo le serveur est figé pendant la pause; en coopératif non, et l'hôte qui quitte déconnecte les autres
- [ ] Sauvegarde en cours de niveau, reconnexion après coupure

### Phase 6 — Progression
- [ ] Niveaux, XP, évolutions de classe (3 par héros)
- [ ] Sauvegarde du profil (compte, progression)
- [ ] Équilibrage

### Phase 7 — Direction artistique et audio
- [ ] Pixel-art définitif: héros, monstres, tuiles, interface
- [ ] Animations, effets de particules
- [ ] Musique et effets sonores
- [ ] Polices pixel-art couvrant les langues prévues
- [ ] Traductions relues pour les langues au lancement

### Phase 8 — Le twist: Castle, boss final
- [ ] Le château observe: événements et dialogues qui le laissent deviner
- [ ] Mémoire de la partie (créatures, pièges et salles rencontrés)
- [ ] Combat final contre Castle, qui réutilise ce que les joueurs ont affronté

### Phase 9 — Mise en ligne
- [ ] Serveurs dédiés, matchmaking ou codes de salle
- [ ] Gestion des déconnexions et reconnexions
- [ ] Anti-triche de base (déjà facilité par le serveur autoritaire)
- [ ] Builds macOS, Windows, Linux (et web via WASM à évaluer)
- [ ] Playtests et corrections

## Questions ouvertes

1. Génération des salles: procédurale (recommandé) ou niveaux faits à la main ?
2. Assets: placeholders puis pixel-art maison, ou pack existant ?
3. Jeu uniquement en ligne, ou aussi en coopération locale / hôte + invités ?
4. Nombre maximal de joueurs par partie (4 pour coller aux 4 héros, ou plus) ?
5. Quelles langues au lancement (FR et EN prévues) et lesquelles ensuite ? Les langues non latines changent le choix des polices.
6. Dialogues: faut-il du doublage ou seulement du texte ? Cela change le travail audio par langue.
7. Plusieurs joueurs peuvent-ils prendre la même classe ?
