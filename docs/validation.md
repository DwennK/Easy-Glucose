# Validation locale — 17 novembre 2026

Machine : Mac Apple M4, macOS 27.2 (26B123). Rust 1.94.1. Aucune connexion à un compte Dexcom réel, aucun push ni workflow distant exécuté.

## Contrôles logiciels

- `cargo fmt --check` : réussi.
- `cargo clippy --locked --all-targets -- -D warnings` : réussi sur macOS ARM64.
- `cargo test --locked` : 18 tests réussis, dont renouvellement de session sur serveur HTTP local, rejet des dates invalides/futures, rupture des graphiques, conservation/correction de cache et séparation de la simulation.
- `cargo build --release --locked` : réussi sur macOS ARM64.
- Le contrôle croisé Windows depuis macOS a échoué dans le code C de `ring`, faute de SDK C Windows (`assert.h`). Ce résultat ne valide pas la compilation Windows. Le workflow comporte un runner Windows natif pour effectuer cette validation.
- macOS Intel : workflow configuré ; ni build ni exécution validés localement.

## Vérifications natives

Dashboard clair et sombre inspecté dans l’application native (1120 × 800 points, Retina). Les périodes 1 h / 6 h / 12 h ont été actionnées ; maximum et tracé changent avec la période. La rupture de données de simulation est visible sur 12 h.

Les widgets compact (256 × 154), normal (320 × 240) et étendu (410 × 300) ont été inspectés. Changement d’unité mmol/L ↔ mg/dL, thème et format vérifiés. Masquage via réglage, ouverture depuis dashboard, fermeture par Échap, fermeture du dashboard en conservant le widget, puis réouverture des paramètres par ⌘ + virgule depuis le widget : vérifiés. Les réglages et le widget ont été restaurés au redémarrage du bundle. Assistant initial observé sans compte.

Limite de l’outil natif : les contrôles du widget sans décoration ne sont pas exposés dans son arbre d’accessibilité, et l’action de clic a renvoyé `noWindowsAvailable` lorsque seul le widget était visible. Les raccourcis ont permis de vérifier ce parcours. Un essai manuel des boutons, du glisser et de VoiceOver sur cette fenêtre reste requis. Les commandes du tray sont implémentées, mais leur parcours souris complet n’a pas été contrôlé avec cet outil. Aucun identifiant réel n’a été saisi.

Les contrôles utilisent Computer Use pour l’application native. Playwright ne pilote pas les fenêtres egui ; il n’y a ni page web ni viewport mobile dans ce produit desktop.

## Performances

Après le parcours complet, dashboard visible en simulation et widget masqué : sur **30,014 s**, temps CPU **0,02 s**, soit **0,067 % d’un cœur** ; RSS finale **153,70 Mio**. Cette mémoire inclut le processus et les pages résidentes partagées/du pilote, et n’est pas une mesure de mémoire privée. Les deux courts relevés ne démontrent pas la consommation sur une journée entière.

Premier échantillon de 30,013 secondes en simulation, écran verrouillé : augmentation de temps CPU processus de 0,02 seconde, soit environ **0,067 % d’un cœur**, RSS finale **90,27 Mio**. Mesure via `ps`, script reproductible `scripts/measure_idle.py`. Il s’agit d’un relevé local court, pas d’une garantie ni d’une mesure sous Windows. Deux lancements release ont atteint le premier callback UI en **148,14 ms** et **147,51 ms** (`--diagnostics`). Cette mesure exclut la fin du rendu GPU et la présentation par le compositeur, et ne représente pas un démarrage à froid complet.

Le code redessine sur interaction, événement du service et mise à jour d’ancienneté toutes les 30 secondes. Pas de minuterie à 60 images/s, pas d’animation permanente. Aucune requête HTTP et aucun accès au trousseau dans les callbacks de rendu.

Le binaire release ARM64 mesure environ **6,5 Mio** ; le bundle avec icônes, documentation et licences environ **12 Mio**. Ses dépendances dynamiques pointent exclusivement vers des bibliothèques et frameworks système macOS.

## Validation restante avant diffusion publique

- Compte Dexcom réel : authentification, données G7, région internationale suisse, veille/réveil, expiration et perte réseau.
- Windows 11 et Mac Intel : build natif, interactions, tray, DPI, consommation et démarrage.
- Lancement automatique et permissions du trousseau sur une identité signée stable.
- Parcours complet VoiceOver/Narrator et changement de configuration d’écrans.
- Developer ID, notarisation Apple et signature Windows. La signature ad hoc locale ne valide pas Gatekeeper pour un téléchargement public.

## Livraison et nettoyage

Bundle final macOS ARM64 ouvert sur l’assistant de configuration, sans simulation active et sans compte connecté. Signature ad hoc vérifiée par `codesign --verify --deep --strict` ; cette vérification ne constitue pas une notarisation. Sources conservées localement, aucun commit, push ou déploiement distant.

Après validation, suppression du `target/` créé pour cette tâche, des notices intermédiaires déjà intégrées au bundle, du cache Python des scripts et de la copie de toolchain 1.94.1 installée pendant la tâche (identique au stable déjà disponible). Le projet utilise le stable existant ; la CI épingle 1.94.1. Aucun processus ne tenait de fichier dans `target/` au nettoyage. Espace alloué récupéré mesuré avec `du` : **2306.23 Mio**, soit **2.25 Gio**. Le bundle courant, les sources, Cargo.lock, les ressources, les dépendances partagées et les données utilisateur sont conservés.


## Refonte de l’interface — seconde validation du 17 novembre

La version courante remplace la barre latérale par deux destinations, Ma glycémie et Réglages. Les réglages conservent trois onglets fixes : Affichage, Compte Dexcom et Options avancées. Boutons délimités au repos, choix actifs cochés, interrupteurs Oui/Non et formulaires contrastés dans les deux thèmes. Les options courantes tiennent dans la fenêtre native standard de 1120 × 800 points.

Le graphique s’inspire des conventions du [guide officiel Dexcom G7, chapitre Écrans d’affichage](https://s3.us-west-2.amazonaws.com/dexcompdf/G7/AW00046-05_UG_G7_OUS_en_MMOL.pdf) : points neutres, dernière mesure agrandie, zone grise, repère haut jaune, repère bas rouge et axe vertical à droite. Les repères restent ceux configurés localement, ne sont pas synchronisés avec les alertes Dexcom et ne déclenchent aucune alarme. Aucun point interpolé. La période se choisit avec les boutons de 1 à 24 h ; le cadrage est fixe.

Vérifié dans le bundle natif macOS ARM64, avec un dossier QA isolé et des mesures simulées :

- Dashboard clair mmol/L et sombre mg/dL, période 6 h puis 12 h avec interruption visible.
- Trois onglets, formulaire du compte, navigation fixe pendant le défilement et retour à Affichage sans reprendre le défilement des options avancées.
- Choix d’unité et d’apparence ; incrément puis décrément du repère bas (70 → 71 → 70 mg/dL).
- Widget moyen et petit en sombre, grand en clair ; fermeture par Échap et retour aux réglages par ⌘ + virgule.
- `cargo fmt`, Clippy tous les targets sans avertissement, 18 tests et build release réussis pendant cette révision. Clippy et build release réexécutés après le retour des onglets.

Les limites de vérification du widget sans décoration, des comptes Dexcom réels et des autres systèmes restent celles indiquées plus haut. Les mesures de performance précédentes n’ont pas été répétées pour cette refonte. Aucun compte réel utilisé, aucun push ni déploiement.

Après cette seconde validation, le `target/` régénéré a été supprimé après vérification de son absence d’utilisation : **1644.87 Mio** récupérés. Le seul bundle courant est conservé dans `dist/`, avec les sources, dépendances et données intactes.


## Sources supplémentaires — troisième validation du 17 novembre

Ajout de LibreLinkUp et du connecteur commun Nightscout / Nocturne. L’onglet « Compte Dexcom » devient « Sources de données ». Un choix de fournisseur adapte le formulaire ; les identifiants et caches restent séparés. Les menus se ferment après sélection. La restauration affiche une connexion en cours pendant l’accès au trousseau.

Contrôles : 29 tests (23 unitaires et 6 tests de fiabilité), Clippy tous les targets sans avertissement, formatage et build release macOS ARM64. Les nouveaux tests couvrent les tendances et timestamps Libre, la sélection de personne, le renouvellement unique de session, les conditions à accepter, la négociation de région/version, les trois modes d’accès Nightscout (public, jeton, API secret), le refus des redirections, la migration Dexcom et l’isolation des caches. Une tentative de remplacement refusée conserve les identifiants et le cache précédents, y compris après un changement de durée de conservation.

Contrôle natif dans un dossier QA isolant également le trousseau : trois fournisseurs accessibles, formulaire LibreLinkUp, activation du bouton après saisie, remise à zéro des champs lors du changement, jeton facultatif sur un site public et secret obligatoire dans le mode API secret. Connexion Nightscout complète à un serveur HTTP sur 127.0.0.1 avec mesure fictive, statut connecté et affichage de la valeur convertie dans le dashboard. Aucun secret réel saisi par l’agent.

Le remplacement d’un bundle signé ad hoc peut déclencher une demande macOS d’accès au trousseau. Cela a été constaté avec une entrée de test après recompilation. Computer Use refuse l’accès à SecurityAgent ; aucune protection du trousseau n’a été contournée. Une identité de signature stable reste nécessaire avant diffusion. Les comptes réels LibreLinkUp, Nightscout et Nocturne n’ont pas été testés ; les essais du protocole Nocturne utilisent son contrat API v1 compatible. Les limites Windows, Intel, accessibilité et signature indiquées plus haut restent applicables.


## Aperçu depuis la barre de menus macOS

Clic gauche sur la valeur : fenêtre transitoire de 460 × 480 points, valeur, tendance, ancienneté, statut/source et graphique de 1 à 24 h. Clic droit : menu natif avec accès à l’app, aux réglages, au widget et à Quitter. L’aperçu est indépendant du widget et ne change pas la période du dashboard. Position calculée en points depuis l’écran du NSStatusItem, bornée à la zone visible ; fermeture par perte de focus, Échap, croix ou deuxième clic.

31 tests réussis (dont placement avec coordonnées négatives de second écran et transitions de focus), Clippy sans avertissement et build release macOS ARM64. Aucun essai physique multi-écran réalisé.

Le clic réel dans la barre de menus a été effectué et confirmé par l’utilisateur ; le panneau a ensuite été inspecté par Computer Use avec données fictives. Périodes 3 h et 12 h vérifiées, interruption des mesures visible ; « Ouvrir l’app » referme l’aperçu et retrouve le dashboard avec sa période 6 h intacte. Les contrôles du panneau sans décoration ne sont pas exposés individuellement par l’outil d’accessibilité ; les clics par coordonnées fonctionnent lorsque le bureau est accessible.

Le premier rendu présentait un fond noir derrière les coins arrondis, signalé dans une capture utilisateur. Correction : configuration OpenGL compatible alpha, fenêtre d’aperçu transparente et couleur d’effacement entièrement transparente ; les surfaces de contenu restent opaques.


Correction du placement après retour visuel utilisateur : l’ancrage utilise le rectangle du bouton cliquable converti en coordonnées écran, et non celui de sa fenêtre système. Le panneau est centré horizontalement sous ce bouton, à 4 points sous son bord inférieur, avec rabattement aux bords de l’écran. La position extérieure est réappliquée après création pour éviter l’ajustement initial AppKit. Le test géométrique couvre aussi le centrage exact hors bord d’écran. Le rendu corrigé des coins a été inspecté en simulation ; fermeture par Échap vérifiée.


## Densité du panneau de la barre de menus

À la demande de l’utilisateur, panneau réduit de 460 × 480 à 420 × 420 points. Boutons de période : 48 × 26 au lieu de 74 × 42 points ; actions du bas et fermeture à 26 points de haut, texte 12 points. Marges et en-tête réduits, valeur toujours mise en évidence. Le graphique récupère la place libérée par les commandes. Réglages et dashboard conservent leurs dimensions.
