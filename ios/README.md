# Castle sur iPhone et iPad

Le jeu iOS est le même client Rust que sur ordinateur. Deux différences seulement :

- **Commandes tactiles** (`client/src/touch.rs`). Joystick à gauche (il naît sous le pouce), Frapper / Pouvoir / Agir à
  droite, héros 1 à 4 et Pause en haut à droite, OK pendant les dialogues. Dans les menus : flèches, OK, Retour, Langue,
  Effacer. Une nouvelle partie reçoit un nom libre (« Castle 1 »...). Une manette Bluetooth (Xbox, PlayStation, MFi) marche aussi, comme sur ordinateur.
- **Fichiers.** Les sauvegardes vont dans `Documents/saves` de l'app (visibles dans l'app Fichiers) ; les assets sont
  copiés dans l'app.

## Compiler et installer

Il faut un Mac avec Xcode (15 ou plus récent) et Rust.

1. Installer Rust si besoin : `curl https://sh.rustup.rs -sSf | sh`. Les cibles iOS s'installent toutes seules au
   premier build (`rustup target add aarch64-apple-ios aarch64-apple-ios-sim` pour le faire à l'avance).
2. Ouvrir `ios/Castle.xcodeproj` dans Xcode.
3. Cible **Castle** › onglet **Signing & Capabilities** : choisir son équipe (un identifiant Apple gratuit suffit pour
   son propre iPhone). Si Xcode refuse l'identifiant `fr.mileyne.castle`, en mettre un autre, unique.
4. Brancher l'iPhone (ou choisir un simulateur), puis **Run** (⌘R). Le premier build compile Bevy pour iOS et prend
   plusieurs minutes.
5. Avec un compte gratuit, la première fois : sur l'iPhone, **Réglages › Général › VPN et gestion de l'appareil**, faire
   confiance au développeur. L'app est à réinstaller tous les 7 jours ; le compte développeur payant lève cette limite
   et ouvre TestFlight et l'App Store.

En configuration Debug, le code Rust est compilé avec le profil `dev` du dépôt (dépendances optimisées) ; pour jouer
dans de bonnes conditions, passer le schéma en **Release** (Product › Scheme › Edit Scheme › Run › Build Configuration).

## Essayer sur ordinateur

- `CASTLE_TOUCH=1 ./play.sh` affiche les commandes tactiles (il faut un écran tactile pour s'en servir) et les textes
  d'aide de la version tactile.

## À vérifier sur un vrai appareil

Cette version a été écrite sans pouvoir être compilée ni lancée sur iOS. Points à surveiller au premier essai :

- la place des boutons par rapport au HUD et aux menus (tout est dans `layout()` de `touch.rs`) ;
- la taille des textes de l'interface sur un petit écran ;
- le passage en arrière-plan : iOS suspend l'app, la partie en cours est alors perdue.

Pas encore d'icône : Xcode affiche l'icône par défaut. Pour en ajouter une, créer un catalogue d'assets avec un
`AppIcon` de 1024 × 1024 dans le projet.
