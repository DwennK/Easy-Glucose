# Easy Glucose

Un compagnon glycémique natif pour macOS et Windows, écrit en Rust avec eframe/egui. Dashboard interactif, widget flottant, barre de menus macOS et zone de notification Windows.

**Consultation secondaire uniquement.** Easy Glucose n’est pas un dispositif médical. Il ne remplace pas les alarmes et applications officielles du capteur et ne doit pas guider une décision thérapeutique.

## Démarrage

Prérequis : Rust stable (1.94.1 utilisé pour la validation), outils de compilation Xcode sur macOS ; Visual Studio Build Tools avec C++ et Windows SDK sur Windows. Python 3 pour les scripts de packaging uniquement, jamais pour exécuter l’application.

```sh
cargo run --release
cargo run --release -- --demo
cargo run --release -- --demo --diagnostics
```

Dans **Réglages → Sources de données**, choisissez votre fournisseur :

- **Dexcom Share** : région (International pour un compte suisse), identifiant et mot de passe du porteur. Share doit être activé avec au moins un abonné.
- **FreeStyle Libre · LibreLinkUp** : e-mail et mot de passe du compte LibreLinkUp ayant accepté le partage. Si plusieurs personnes sont suivies, choisissez explicitement celle à afficher. Les conditions éventuelles doivent être acceptées dans l’application officielle.
- **Nightscout / Nocturne** : adresse HTTPS du site et jeton de lecture. Le jeton est facultatif pour un site public. Le mode « API secret » reste disponible si votre serveur utilise cette authentification.

Les identifiants de chaque fournisseur sont enregistrés séparément dans le trousseau macOS / Windows Credential Manager après une première récupération réussie. Changer de fournisseur retrouve sa connexion enregistrée. Les caches sont séparés et liés au compte, à la personne LibreLinkUp ou au site et à son jeton. Une tentative de connexion refusée ne remplace pas les identifiants précédemment enregistrés.

Le bouton de simulation permet d’explorer sans compte. La simulation est explicitement indiquée dans toutes les surfaces et ne remplace jamais le cache réel. Quittez-la pour rétablir le compte enregistré. Ne transmettez pas votre mot de passe dans un chat ou un fichier de configuration.

Une mesure ancienne reste affichée avec son âge et un avertissement. Le statut réseau n’implique pas que les mesures sont fraîches. Chaque point du graphique représente une mesure réelle ; les interruptions restent vides. Choisissez une période de 1 à 24 h et déplacez le pointeur dans le graphique pour lire la mesure la plus proche dans le temps, quelle que soit sa hauteur. Le point et son heure réelle sont mis en évidence ; un écart important est signalé, sans inventer de mesure. Le cadrage reste stable et s’élargit si une source fournit des valeurs au-delà de la plage habituelle de Dexcom.

Deux écrans : **Ma glycémie** et **Réglages**. Les choix actifs sont cochés, les boutons sont encadrés et les activations utilisent des interrupteurs Oui/Non. Trois onglets restent accessibles en haut des réglages : **Affichage** (apparence, unité, mini-fenêtre), **Sources de données** et **Options avancées** (repères, démarrage, fermeture et conservation locale). La transparence est volontairement remplacée par un fond opaque fiable. Le widget se déplace depuis son titre ; son bouton ↗ ouvre Ma glycémie. Échap masque le widget ; ⌘/Ctrl + virgule ouvre les paramètres depuis celui-ci. Sur macOS, un clic gauche sur la valeur de la barre de menus ouvre un aperçu avec la glycémie, le graphique et les périodes de 1 à 24 h. Un clic ailleurs, un second clic sur la valeur ou Échap le ferme. Les boutons de l’aperçu ouvrent l’app ou les réglages ; le clic droit conserve le menu classique (app, réglages, widget, quitter). Le widget flottant reste indépendant. Sur Windows, le menu de notification conserve son fonctionnement habituel.

## Vérification

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

Les tests couvrent conversions, tendances, timestamps, sentinelles, ancienneté, fusion de cache, parsing strict, erreurs, transitions d’état et renouvellement de session via un serveur HTTP local. Les tests couvrent aussi les trois sources, le choix de personne, les méthodes d’authentification Nightscout, la migration Dexcom et l’isolation du cache. Aucun test n’utilise d’identifiant réel ni de fournisseur distant.

## Bundle local

```sh
python3 scripts/licenses.py
python3 scripts/package.py
```

Sur macOS, ouvrez `dist/Easy Glucose.app`, ou déplacez ce bundle dans Applications avant d’activer le lancement automatique. Sur Windows, les mêmes scripts avec `python` produisent `dist/Easy-Glucose-Windows/Easy Glucose.exe`, portable ; conservez son dossier et créez un raccourci si souhaité. Les icônes sont déjà fournies ; `scripts/icons.py` permet leur régénération.

Une seule instance est autorisée par dossier de données. Pour des tests isolés, définissez `EASY_GLUCOSE_DATA_DIR` vers un dossier temporaire. Ce réglage isole configuration, cache et espace de noms du trousseau ; `--demo` démarre sans connexion à un compte.

## Distribution

Le workflow `.github/workflows/build.yml` vérifie et compile Windows x64, macOS ARM64 et macOS Intel. Un tag `v*` crée une **release en brouillon** avec les archives, après réussite des trois jobs. Aucun dépôt distant ni release n’est créé par la simple présence de ce fichier.

Les bundles macOS locaux sont signés ad hoc, **sans certificat Developer ID ni notarisation Apple** : Gatekeeper peut bloquer une archive téléchargée. L’exécutable Windows n’est pas signé et SmartScreen peut afficher un avertissement. Voir [distribution](docs/distribution.md) pour le circuit de signature.

## Données et architecture

Configuration et cache résident dans le dossier de données utilisateur (`~/Library/Application Support/ch.Peaks.Easy Glucose` sur macOS, dossier LocalAppData déterminé par `directories` sous Windows). L’application ne journalise ni mots de passe ni jetons. Les mesures locales ne sont pas chiffrées par l’application : leur protection dépend des permissions du compte et du chiffrement du disque. L’effacement local n’efface pas les données chez le fournisseur.

Voir [connexion des sources](docs/sources.md), [architecture](docs/architecture.md), [validation](docs/validation.md) et [licences tierces](THIRD_PARTY.md).

Les connexions Share et LibreLinkUp utilisent des protocoles non officiels étudiés dans des implémentations publiques. Nightscout et Nocturne utilisent l’API de lecture v1 compatible Nightscout. La compatibilité avec un compte réel, les changements de protocole et le fonctionnement sur chaque OS doivent être validés avant une diffusion publique.
