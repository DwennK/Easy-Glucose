# Compilation, installation et distribution

## Plateformes

- macOS Apple Silicon : `cargo build --release --locked --target aarch64-apple-darwin`.
- macOS Intel : `cargo build --release --locked --target x86_64-apple-darwin` après `rustup target add x86_64-apple-darwin`. Une compilation croisée ne prouve pas l’exécution sur un Mac Intel.
- Windows x64 : exécuter les checks et le build sur Windows avec Visual Studio C++ Build Tools ; cible `x86_64-pc-windows-msvc`.

`python scripts/licenses.py`, puis `python scripts/package.py --target CIBLE --archive` fabriquent un bundle `.app` ou un dossier portable Windows, et une archive ZIP avec les licences. L’exécutable ne dépend pas de Python installé chez l’utilisateur.

Les ressources originales de marque sont dans `assets/`. Le script d’icônes est reproductible et n’utilise aucune ressource tierce. La police système est lue sur la machine cible, pas redistribuée.

Installer le bundle macOS dans Applications avant d’activer le lancement automatique. Sous Windows, extraire le dossier dans un emplacement stable (par exemple un dossier utilisateur Applications), puis créer un raccourci de l’exécutable. Cette version propose une distribution portable, pas encore un installateur MSI avec désinstallation gérée.

## GitHub Actions

La matrice utilise `windows-2025`, `macos-15` (ARM64) et `macos-15-intel`. Les labels proviennent de la [documentation GitHub des runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners). Les tests de protocole utilisent localhost, sans secrets. Les artefacts expirent après 14 jours. Les tags créent des releases en brouillon pour permettre signature et contrôle avant publication.

## Signature macOS

La signature ad hoc du packaging permet une exécution locale ; elle ne fournit aucune identité de développeur ni approbation de Gatekeeper. Pour une diffusion :

1. Disposer d’un compte Apple Developer et d’un certificat **Developer ID Application** dans un trousseau dédié de CI.
2. Signer le bundle final avec runtime renforcé et horodatage :
   ```sh
   codesign --force --options runtime --timestamp --sign "Developer ID Application: NOM (TEAMID)" "dist/Easy Glucose.app"
   codesign --verify --deep --strict --verbose=2 "dist/Easy Glucose.app"
   ```
3. Créer le ZIP après signature avec `ditto -c -k --keepParent`.
4. Soumettre via `xcrun notarytool submit ARCHIVE.zip --keychain-profile PROFIL --wait`. Stocker les identifiants Apple dans un profil sécurisé, jamais dans le dépôt ou les logs.
5. Après acceptation : `xcrun stapler staple "dist/Easy Glucose.app"`, vérifier avec `spctl --assess --type execute --verbose=2`, recréer l’archive et tester sur un Mac distinct.

Référence : [Apple — Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution). Les secrets de signature et la notarisation ne sont pas configurés dans cette première livraison.

Sous Windows, signer le binaire final avec un certificat de signature de code et un service d’horodatage adapté avant publication. Ne pas promettre l’absence d’avertissement SmartScreen sans essais réels.

## Contrôle avant publication

Tester la connexion Share avec un compte de test autorisé et les régions nécessaires ; reprise après veille, hors ligne, expiration de session, déconnexion et redémarrage. Vérifier les permissions du trousseau sur un bundle signé stable. Tester Windows 11 et un Mac Intel réels, DPI multiples, écrans débranchés, accès clavier et lecteur d’écran. Mesurer consommation au repos, mémoire et démarrage sur chacune des cibles ; aucune mesure Apple Silicon ne vaut preuve pour Windows ou Intel.
