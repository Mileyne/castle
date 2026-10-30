# Castle sur Apple TV

Même client Rust que sur iPhone, compilé pour tvOS. Rust n'a pas de support tvOS dans les dépendances du jeu (winit, wgpu...
n'y voient qu'un Unix générique), alors `aarch64-tvos-as-ios.json` déclare une cible tvOS qui se dit `os = "ios"`: les
dépendances prennent leur code iOS. `cc.sh` et `linker.sh` remettent les objets C et l'édition de liens sur tvOS. Il faut la
chaîne d'outils `nightly` de rustup (`-Zbuild-std`) et Xcode.

1. Appairer l'Apple TV dans Xcode (Window › Devices and Simulators) et choisir son équipe dans `Castle.xcodeproj`.
2. `xcodebuild -project Castle.xcodeproj -scheme Castle -configuration Release -destination 'id=<UDID>' -allowProvisioningUpdates build`
   (ou Run dans Xcode), puis `xcrun devicectl device install app --device <UDID> build/Build/Products/Release-appletvos/Castle.app`.

Il n'y a pas d'écran tactile: on joue à la manette (lue par GameController, voir `client/src/gamecontroller.rs`).
