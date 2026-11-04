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
| Moteur | **Bevy 0.20** (Rust, ECS) | |
| Réseau | aucun: jeu solo (le mode coopératif en réseau a été retiré) | Les anciens évènements réseau sont de simples évènements locaux |
| Structure | Workspace Cargo: `shared`, `client` | La simulation (`client/src/sim/`) tourne dans le monde du client |
| Rendu | 2D, tuiles de 16 px, échelle entière; héros rendus en 96 px | Caméra qui suit le héros; sprites provisoires générés par `tools/gen_sprites.py` (planches dans `client/assets/art/`), à remplacer par du pixel-art dessiné au même découpage |
| Localisation | **Fluent** via `bevy_fluent` 0.15 | Fichiers `.ftl` par langue dans `client/assets/locales/<langue>/`; FR et EN au départ |
| Dialogues | Format maison en **RON** (`client/assets/dialogue/castle.ron`) + interpréteur `client/src/dialogue_script.rs` | Scènes = répliques `Line(orateur, id)` et `Choice`; les textes sont les clés Fluent `dlg-<id>` (Yarn Spinner abandonné: sa dépendance bloquait la montée de version de Bevy) |
| Niveaux | Génération procédurale par salles assemblées (à confirmer) | Alternative: niveaux écrits à la main (Tiled) |
| iOS | Même client Rust, projet Xcode dans `ios/` | Commandes tactiles qui simulent les touches (`client/src/touch.rs`); voir `ios/README.md` |

### Règles de localisation (à respecter dès le premier jour)

- **Aucun texte affiché n'est écrit en dur dans le code.** Le code ne manipule que des clés (`class-knight`, `quest-hunt-minotaur`...). Voir `name_key` et `special_key` dans `shared/src/lib.rs`.
- **Le serveur n'envoie jamais de texte**, seulement des clés et des paramètres. Chaque joueur d'une même partie peut ainsi jouer dans sa propre langue.
- Les messages à variables (pluriels, genres, nombres) passent par Fluent, pas par du `format!`.
- Les dialogues sont des scènes RON: les répliques ont un identifiant stable, et chaque langue a sa table de chaînes. Les choix de dialogue renvoient au serveur un identifiant, pas un texte.
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
- **Passifs :** détecte les secrets, régénération lente de mana.
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

**Boss final : Nareff.** La forteresse entière prend vie et utilise contre les joueurs toutes les créatures, pièges et salles rencontrés durant leurs précédentes aventures.

Thème central : quatre héros essaient de comprendre la volonté d'un château vivant qui les manipule depuis le début.

---

## La campagne en cinq actes

Vingt niveaux fixes (`shared/levels/01.ron` à `20.ron`), en cinq actes de quatre niveaux; le quatrième de chaque acte est un combat de boss. Chaque niveau a son heure du jour et sa lumière (`client/assets/lighting.ron`, réglable avec `./lumiere.sh`), et **commence dans le décor où le précédent s'est terminé**: la clairière de départ reprend la matière des murs et du sol de la région d'où l'on vient (le premier niveau, lui, commence dans une clairière). Le soleil ne revient qu'à la fin du jeu.

| Acte | Niveau | Lieu | Objectif | Lumière |
|---|---|---|---|---|
| **1. La forêt de brume** | 1 | L'aube dans la brume | Chasser 12 loups | Aube grise, brume dense |
| | 2 | Le sous-bois détrempé | Le Forgebraise répare la passerelle | Matinée pluvieuse |
| | 3 | Le fortin du garde-chasse | Le Rôdeur lève la herse, deux gardes prisonniers | Crépuscule naissant |
| | 4 | La meute (boss: le Warg) | L'Ensorceleur brise le sceau | Nuit noire; Samélia est enlevée (`samelia_taken`) |
| **2. Le monde des elfes** | 5 | Les racines | Chasse, bêtes des elfes | Aube verte très sombre, les objets rayonnent |
| | 6 | Le marais luminescent | Le Forgebraise remet l'écluse en marche | Plus vive, bioluminescente |
| | 7 | La rivière et le pont de corde | Le Chevalier tend le pont | Heure bleue |
| | 8 | La clairière du trône vert (boss: le Jardinier Noir) | Délivrer Samélia, qui ouvre la Porte des Racines | Clairière froide et verte |
| **3. Les portes de Nareff** | 9 | Le chemin du château | Le Rôdeur lève le pont-levis | Gris d'avant l'aube |
| | 10 | La cour d'honneur | Le Chevalier force la grande porte | Brasiers |
| | 11 | Les cryptes | Le Chevalier pousse la porte royale | Torches |
| | 12 | Le tombeau du roi (boss: Grogoth, le Roi Sans Visage) | Vaincre le boss | Torches, brume froide |
| **4. Les entrailles** | 13 | Les jardins maudits | L'énigme complète à quatre classes, délivrer deux prisonniers | Lueur mourante |
| | 14 | La bibliothèque vivante | Chasse | Chandelles |
| | 15 | La forge infernale | Délivrer deux prisonniers | Rouge de forge |
| | 16 | Le cœur de la forge (boss: le Dragon de la Forge) | Vaincre le boss | Lave |
| **5. Le cœur de Nareff** | 17 | La tour astrale | Chasse | Étoiles |
| | 18 | Les appartements royaux | Retrouver des reliques | Lueurs de souvenirs |
| | 19 | Les galeries royales | Objectif du niveau | Sombre |
| | 20 | Le cœur de Nareff (boss: Vaelor, en attendant le boss final) | Délivrer deux prisonniers, vaincre Vaelor | Foudre noire |

**Samélia** enlevée à la fin de l'acte 1, délivrée au bout de l'acte 2, reste ensuite aux portes de Nareff (`gate_farewell`) et ne revient qu'à l'acte 5 (`heart_reveal`). Ses lanternes, offertes au prologue, faiblissent pendant qu'elle est captive (niveaux 5 à 8).

À faire pour boucler cette conception: modèles propres du Jardinier Noir et du Dragon (aujourd'hui le troll, verdi ou rougi), ennemis végétaux de l'acte 2 (aujourd'hui des goules), lumière colorée (vert des elfes, rouge de la forge), le boss final Nareff, et les niveaux 14, 17 à 19 à écrire à la main (ce sont des châteaux générés).

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
cargo run -p castle_client              # le jeu
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
- [x] Lancement unique: `./play.sh` (ou `castle_client`) lance le jeu; la simulation tourne dans le même monde que l'affichage (plus de serveur ni de réseau: le mode coopératif réseau a été retiré).
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
- [x] Zoom d'équipe: quand les quatre héros sont groupés, la caméra se rapproche jusqu'à les cadrer tous (jusqu'à x2,2 en plus de l'échelle de base) et se centre sur le groupe; dès qu'ils s'éloignent trop pour tenir dans la vue, elle revient en douceur. Constantes `TEAM_ZOOM_MAX`, `TEAM_MARGIN` dans `client/src/camera.rs`

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
- [x] Dialogues côté client (interpréteur RON, ex-Yarn Spinner), avancement des dialogues piloté par le serveur
- [x] Dialogues partagés: tous les joueurs voient la même scène, chacun dans sa langue
- [x] Règle de vote ou de décision quand un choix engage le groupe
- [ ] Répliques de caractère propres à chaque héros (réactions, remarques entre eux)
- [ ] Outil de vérification: textes trop longs pour l'interface

### Phase 3 — Pouvoirs et coopération
Objectif: les classes ont chacune un rôle.
- [x] Pouvoirs spéciaux: Rempart, Tempête Arcane, Pluie de Flèches, Tourelle (touche E)
- [x] Mana et temps de recharge
- [x] Passifs (détection des leviers et mécanismes cachés par l'Ensorceleur et le Rôdeur, désamorçage des pièges par le Rôdeur, porte royale poussée par le Chevalier; portes à clé)
- [x] Pièges toujours visibles: endormis (pointes rentrées) tant que personne n'est à moins de 56 px, ils se réveillent (pointes sorties, 12 dégâts à qui marche dessus) à l'approche d'un héros debout et se rendorment 1,2 s après son départ. Le Rôdeur peut les désarmer pour de bon en maintenant l'action; ils restent alors visibles et inoffensifs
- [x] Puzzle coopératif de la **porte royale**, refondu: trois obstacles posés comme des **panneaux dans le mur** de l'antichambre (jamais au sol), chacun **dans la couleur de la classe qui doit le débloquer**, plus la porte elle-même:
  - **Violet, Ensorceleur**: un enchantement repousse tout héros à moins de 72 px de la porte. Il faut trouver **3 ingrédients** (cristaux violets, dispersés dans le château) et les verser dans le panneau de runes (maintenir F 1,5 s) pour dissiper la garde
  - **Orange, Forgebraise**: un mécanisme grippé (barre tordue, rouille). Il faut trouver la **pièce de rechange** (engrenage orange) puis la monter (maintenir F 2,5 s); les rouages tournent ensuite
  - **Vert, Rôdeur**: une herse barre le couloir devant la porte. Il faut trouver le **disque de cible** (vert), le fixer au support vide (maintenir F 0,8 s), puis le **toucher d'une flèche** du Rôdeur (un sort ne compte pas): la herse se lève
  - **Acier bleu, Chevalier**: une fois les trois levés, il pousse la porte (maintenir F 3 s, force et courage)
  Les pièces trouvées forment une réserve commune à l'équipe, affichée sous l'objectif (ingrédients, pièce, cible, en violet, orange et vert; « OK » une fois l'obstacle vaincu). Dialogues: la porte annonce les trois obstacles et les couleurs, chaque panneau a sa propre scène. Option de développement `CASTLE_DEBUG_START=royal` pour apparaître dans l'antichambre
- [x] Les dialogues non terminés sont **purgés à chaque changement de niveau et à chaque retour au début du niveau** (le serveur le signale à tous les clients, et chaque client le refait de son côté dès qu'un nouveau château arrive), ainsi qu'après 90 s sans activité: plus de scène fantôme qui reparaît bloquée au niveau suivant
- [x] Dialogues: **maintenir Entrée 0,7 s purge** la scène en cours et celles qui allaient démarrer (un héros dans leur zone); pour tous les joueurs, une introduction purgée n'est plus rejouée
- [x] Outil de vérification des textes: des tests contrôlent que chaque réplique existe en français et en anglais, sans clé orpheline, et que chaque scène lancée par le serveur existe dans le script

### Phase 4 — Le château procédural
Objectif: une expédition complète et rejouable.
- [x] Château procédural: grille 4x3 de salles de tailles variées reliées par des couloirs, générée depuis une graine partagée serveur/clients (tests sur 200 graines)
- [ ] Les 6 régions avec identité visuelle et ennemis propres (noms et teintes de tuiles faits; tuiles, ennemis et boss propres à venir)
- [x] Portail de sortie derrière la porte royale (la sortie est une impasse); porte à clé devant le trésor
- [x] Reconfiguration du château: quand tous les héros debout entrent dans le portail, un nouveau château et une nouvelle région sont tirés

### Phase 5 — Quêtes et boss
Objectif: de vrais objectifs de mission.
- [x] Système de quêtes (première version): un objectif par niveau, en rotation chasse (6 monstres, +2 tous les 3 niveaux) → récupération (3 reliques) → sauvetage (2 prisonniers, à rester près d'eux 1,2 s). Entité répliquée `Quest`, affichée en haut de l'écran; le portail de sortie n'apparaît qu'une fois l'objectif rempli (secousse d'écran). Si plus aucun monstre ni générateur ne subsiste, la chasse est réputée accomplie. Sprites des reliques et prisonniers provisoires. À venir: défense, expédition longue
- [x] Trois monstres de plus: le **gobelin** (petit, vif, fragile: 18 PV), l'**orque** (massif, lent, hache: 70 PV, 14 dégâts) et la **goule** (décharnée, rapide: 40 PV). Ils apparaissent progressivement via les générateurs (`MonsterKind::spawn_table`): gobelin dès le niveau 1, goule au niveau 2, orque au niveau 3. Sprites générés par `sprite_forge` (`monsters_more.rs`), éditables dans l'éditeur d'art
- [x] Équilibrage de la difficulté: PV des héros augmentés d'environ 75 % (Chevalier 260, Ensorceleur 150, Rôdeur 180, Forgebraise 200); beaucoup plus d'ennemis: 2 à 6 monstres posés d'avance dans chaque salle de combat (selon le niveau), 2 à 3 générateurs par salle au lieu de 1 à 2, apparition toutes les 2,2 s au lieu de 3,5 s, et jusqu'à 28 monstres vivants au lieu de 12
- [x] **Potion de régénération**: 2 à 5 par niveau (salles de combat, plus une dans la salle au trésor) et 4 % de chance sur un monstre tué. Seul le héros dirigé la ramasse; elle rend 6 % des PV maximum par seconde pendant 8 s à tous les héros de son équipe (pas aux héros à terre), qui pulsent en vert pendant l'effet
- [ ] Les 4 boss avec leurs mécaniques
- [ ] Artefacts légendaires
- [ ] Récompenses de fin de mission

### Phase 5b — Modes de jeu et sauvegardes *(première version, menu à valider en jeu)*
- [x] Parties nommées, (`shared/src/save.rs`): fichier RON par partie dans le dossier de données de l'utilisateur (`~/Library/Application Support/Castle/saves` sous macOS, `CASTLE_SAVE_DIR` pour le changer), écriture atomique, copie `.bak`, version de format
- [x] Sauvegarde à chaque entrée de niveau, pas en cours de salle: elle garde le niveau, l'état du générateur aléatoire à l'entrée (qui régénère le même château), l'or et les clés de chaque équipe, et si l'intro a été vue
- [x] Équipe entièrement à terre: retour au début du niveau (même château, monstres revenus, or et clés de l'entrée)
- [x] Menu titre: jouer, réglages, quitter; liste des parties avec « nouvelle partie », suppression avec confirmation. Une partie est un fichier de sauvegarde nommé.
- [ ] Les équipes retrouvent leur bourse dans l'ordre d'arrivée; une identité de joueur persistante (UUID) sera nécessaire dès qu'il y aura de l'XP par héros
- [x] Menu de pause (Échap): reprendre, ou abandonner le niveau et revenir au menu principal (la progression depuis l'entrée du niveau est perdue). La simulation est figée pendant la pause.
- [ ] Sauvegarde en cours de niveau, reconnexion après coupure

### Éditeur d'art (`./edit.sh`, outil `tools/sprite_editor`, egui)
- [x] **Tuiles**: peinture pixel (crayon, gomme qui rend l'original, pipette, remplissage, pinceau, symétrie, annuler/rétablir, grille des tuiles, aperçu en répétition) sur chaque groupe de tuiles de chaque région. Seules les tuiles modifiées sont enregistrées, en PNG de 48x48 dans `tools/sprite_forge/art_overrides/<thème>/tiles/`, et recouvrent la tuile générée (et ses versions floutées): relancer `sprite_forge` ne les écrase jamais
- [x] **Sprites** (héros, monstres, objets): taille du modèle; pour les héros et le squelette, proportions du corps (13 mesures); pour les héros médiévaux, traits du visage (yeux, sourcils, nez, bouche, mâchoire, oreilles, barbe, rides, lunettes); pour chaque matière (toutes les parties d'une même couleur d'origine): couleur, taille, brillance, lueur, avec surlignage en magenta. Aperçu en direct dans les 8 directions et les animations de repos, de marche et d'attaque. Réglages dans `art_overrides/<thème>/models.ron`, appliqués par-dessus le code des modèles
- [x] **Équilibrage** (onglet, option `--equilibrage`): toutes les valeurs de jeu réglables sont réunies dans `shared/balance.ron` (`shared/src/balance.rs`): points de vie, vitesse, mana, coups et tirs de chaque héros; points de vie, dégâts, cadence et vue de chaque monstre; tables d'apparition par niveau et par région, nombre de monstres et de potions par niveau, générateurs; butin; objets ramassés; pièges; pouvoirs; compagnons; règles de relève et de sortie. Champs modifiés en orange avec retour à la valeur enregistrée, « Essayer dans le jeu » (`CASTLE_BALANCE_FILE=<fichier>` lance le jeu avec un autre fichier sans recompiler). Hors de ce fichier: ce qui est structure (genre d'attaque de chaque classe, pouvoirs, mécaniques d'énigme) et les réglages des boss (`sim/boss.rs`)
- [x] **Vue à la première personne** (F4, `client/src/firstperson.rs`): la caméra se place à hauteur des yeux du héros dirigé (perspective de 78°), la souris tourne la vue et l'incline (elle est captée pendant la partie, rendue dans la pause), les touches de direction vont dans le sens de la vue; le héros dirigé est caché. `CASTLE_FIRST_PERSON=1` démarre dans cette vue. Essai de la vue: les coups partent encore dans la dernière direction de marche. Les murs y sont deux fois plus hauts (`WALL_FACTOR`), les compagnons trop proches de la caméra sont cachés, et les forêts ont un ciel nocturne (`client/src/sky.rs`: dôme sans éclairage, étoiles de brillances variées et nuages bleutés en brume, texture calculée à la première demande); les autres lieux gardent le noir pour l'instant
- [x] **Eau et ponts**: deux terrains de plus dans les niveaux, `~` (eau) et `=` (pont) dans `rows`. L'eau ne se franchit pas à pied (héros, compagnons, monstres), sauf sur un pont; les tirs et le regard passent au-dessus. La vérification d'un niveau signale une sortie ou un objet coupé de l'eau. Le jeu les dessine lui-même (ondes, écume, planches); outils « ≈ Eau » et « ▤ Pont » dans l'éditeur de niveaux. L'eau du jeu est une surface animée par un shader, éclairée comme le sol (ambiance, soleil et lanternes du niveau: elle ne brille pas d'elle-même) et dessinée par pixels de la taille de ceux du sol (`assets/shaders/water.wesl`, `client/src/decor3d/water.rs`): vagues, reflet du ciel plus fort quand on la voit en rase-motte, éclats du soleil, lignes de lumière, étincelles, rives claires et écume. Le sol « Pont » est un **pont à corde** en 3D (planches, poteaux et deux cordes du côté de l'eau, dans les couleurs du bois du jeu). Le **pont cassé** (sortie « pont ») est un **trou béant** au milieu du pont (de l'eau noire bordée de bouts de planches brisés), de deux tuiles de large sur deux de long: il se pose sur le bloc de deux sur deux qui contient le plus de pont (l'éditeur et le jeu le calent tout seuls, même visé à une tuile près), se tourne selon l'eau qui le borde, et barre le passage tant que la sortie n'est pas ouverte; réparé, des planches neuves le comblent
- [x] **Éditeur de niveaux: sortie, sélection et liens.** Outil « ⚑ Sortie »: un clic pose la sortie (une seule par niveau) avec l'apparence choisie (escalier, barrière, pont cassé, pont-levis, invisible); sans sortie posée elle est au centre de la zone de sortie, ou toute la zone avec « La zone entière est la sortie ». L'outil « Sélection / Déplacer » montre à droite les réglages de l'élément (type, position, scène de dialogue, rayon et délai d'un déclencheur, herse verticale, Samélia prisonnière...), avec « Dupliquer » et « Supprimer », et la liste de tous les éléments du niveau. Les panneaux d'énigme, la herse, l'enchantement, Samélia et les déclencheurs de scène sont dans la palette. Les **liens** (`LevelDef::links`) sont déduits des règles du jeu, pas écrits dans le fichier: pièce, ingrédients et disque vers leur panneau, rune vers l'enchantement, cible vers la herse, herse, enchantement et panneaux vers la porte royale (les accès qu'ils débloquent), reliques, prisonniers et boss vers l'objectif de la sortie (en bleu). Un obstacle qui barre un accès avant la sortie (le pont cassé du niveau 2) est un élément à part, `Barrier` (palette « Obstacle »): il s'ouvre avec l'objectif, les panneaux et l'objectif y mènent, et ce n'est jamais la sortie. La sortie n'est pas un déblocage: on la prend en atteignant sa zone, le déblocage des accès intervient plus tôt dans la carte. Ils sont dessinés en flèches sur la carte (en doré pour l'élément sélectionné) et listés dans l'inspecteur, où un clic mène à l'autre extrémité. La vérification signale un panneau auquel il manque de quoi agir, une herse ou un enchantement que rien ne défait, une scène inconnue.
- [x] **Éditeur de niveaux 3D** (programme séparé `tools/level_editor`, `./editeur.sh`, binaire `castle_editor`, qui utilise `castle_client` comme bibliothèque; le jeu ne contient plus son code ni `bevy_egui`): l'éditeur de niveaux est **le jeu lui-même** (même rendu, mêmes lumières, eau animée, ciel, éléments en 3D) avec des panneaux egui par-dessus (`bevy_egui`), et non plus un dessin à plat. Le monde est figé, sans musique, sans interface du jeu, sans météo; le niveau modifié remplace celui du jeu à chaud (`castle_shared::override_level`, `sim::ReloadLevel`), au plus quatre fois par seconde pendant un geste. Caméra libre: molette pour le zoom, bouton du milieu (ou Alt + clic) pour tourner, Maj en plus pour se déplacer, flèches ou ZQSD pour se déplacer, E pour tourner, « Recentrer ». Les outils visent le sol de la scène (rayon de la caméra du décor): sélection et déplacement, mur, sol, eau, pont, décor, éléments (sortie comprise), effacer, zone; pinceau, rectangle et remplissage; Ctrl+Z / Ctrl+Maj+Z, Ctrl+S, Suppr. Zones, portes, liens, sortie, sélection et pinceau sont tracés sur le sol; les noms des zones et de l'élément sélectionné à l'écran. Disposition: le niveau édité se choisit dans la liste déroulante de la barre du haut (avec annuler, rétablir, enregistrer, essai); les outils sont des icônes sur deux colonnes dans le panneau de gauche (sélectionner / déplacer, effacer, mur, sol, eau, pont, décor, éléments, zone), et l'outil choisi affiche dessous sa sous-palette: forme et taille du pinceau et matière pour le mur et le sol (la matière donne aussi leur décor aux tuiles peintes, ou se change seule), sortes de décor, éléments à poser par groupes (dont la sortie du niveau, l'obstacle, l'énigme et les scènes), genre de zone. Panneau de droite: réglages de l'élément sélectionné et ses liens, sortie, liste des éléments, objectif, zones, portes, vérification. « Essayer dans le jeu » enregistre et lance le jeu sur ce niveau. `--niveau=N` ouvre ce niveau. Les onglets Sprites, Tuiles et Équilibrage restent dans `./edit.sh` (`sprite_editor`), qui n'a plus d'onglet Niveaux.
- [x] « Appliquer au jeu » enregistre puis régénère les planches du thème dans `client/assets/art/` (tuiles, décor, marques au sol, projectiles, objets; relancer le jeu pour les voir). Héros, monstres, grands motifs et sorties sont des modèles 3D dessinés en mémoire, sans planche; `portraits.png` est retouché à la main et n'est jamais réécrit s'il existe. Sans retouche ni réglage, la génération reste identique à l'octet près
- [x] Couvre-chef amovible pour les quatre héros (bassinet du Chevalier, chapeau de l'Ensorceleur, chapeau de feutre à plume du Rôdeur, casque du Forgebraise; style médiéval), et zoom de l'aperçu des sprites: jusqu'à x32, Ctrl+molette ou pincement centré sur le curseur, glisser pour se déplacer, rendu détaillé x2 ou x4, raccourcis « Tout voir » et « Tête »; options `--modele <clé>` et `--tete`
- [ ] Couvre-chefs amovibles des styles manga et sanglant
- [ ] Éditeur de volumes complet (déplacer, ajouter ou supprimer chaque sphère, capsule ou cône); visage des héros des styles manga et sanglant; retouche des objets au pixel près

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

### Phase 8 — Le twist: Nareff, boss final
- [ ] Le château observe: événements et dialogues qui le laissent deviner
- [ ] Mémoire de la partie (créatures, pièges et salles rencontrés)
- [ ] Combat final contre Nareff, qui réutilise ce que les joueurs ont affronté

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
