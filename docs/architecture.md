# Architecture et décisions

## Référence étudiée le 17 novembre 2026

Analyse fonctionnelle, sans réutilisation du code Python de GlucoBar :

- [dexcom.py](https://github.com/jackfaz/glucobar/blob/main/glucobar/dexcom.py) : authentification du porteur en deux POST, région US/OUS/Japon, renouvellement unique de session, lecture de 288 mesures / 1440 minutes. Validation croisée avec [pydexcom const.py](https://github.com/gagebenne/pydexcom/blob/main/pydexcom/const.py) et [dexcom.py](https://github.com/gagebenne/pydexcom/blob/main/pydexcom/dexcom.py).
- [controller.py](https://github.com/jackfaz/glucobar/blob/main/glucobar/controller.py) : tâches réseau distinctes, rappels Tk périodiques et tick à la seconde. Easy Glucose utilise les événements egui et un minuteur d’ancienneté à 30 secondes, sans boucle de rendu permanente.
- [desktop.py](https://github.com/jackfaz/glucobar/blob/main/glucobar/desktop.py) : tracé Canvas et lissage. Ici, points uniquement, sans liaison interpolée, et sélection du véritable échantillon le plus proche dans le temps depuis toute la surface du tracé. La hauteur du pointeur ne change pas la sélection ; les égalités privilégient la mesure antérieure. Le repère vertical et l’infobulle utilisent le même échantillon, avec son heure réelle et ses limites de capteur. Un écart supérieur à 225 secondes est explicité. Aucun calcul intermédiaire de glycémie. Cette logique est partagée par le dashboard, l’aperçu et le widget ; la sortie du tracé masque la sélection.
- [macbar.py](https://github.com/jackfaz/glucobar/blob/main/glucobar/macbar.py) : valeur et tendance dans NSStatusBar. Ici, `tray-icon` fournit la même intégration native.
- [taskbar.py](https://github.com/jackfaz/glucobar/blob/main/glucobar/taskbar.py) : dépendance aux classes internes du shell Windows et maintien de position. Cette approche n’est pas reprise. L’alternative maintenable est un widget toujours visible et l’icône de notification avec infobulle.

Le protocole Share est non officiel et ne constitue pas l’API développeur Dexcom OAuth. Les constantes ont été vérifiées dans deux implémentations publiques, mais aucune connexion à un compte Dexcom réel n’a été effectuée sans identifiants. La région internationale est la valeur initiale pour les comptes suisses. Share doit être activé dans l’application officielle avec un abonné.

## Composition

- `core` : mesures UTC, unités, tendances, ancienneté, fusion bornée, simulation.
- `dexcom` : HTTPS avec délai maximum, réponses plafonnées, erreurs expurgées, session uniquement en mémoire. Redirections désactivées.
- `sources` : choix du fournisseur, client LibreLinkUp (région négociée, session renouvelée une fois, sélection explicite de personne) et client Nightscout/Nocturne (API v1 en lecture seule, jeton ou secret SHA-1). Voir [sources](sources.md).
- `services` : thread dédié, runtime Tokio à un thread, commandes et snapshots étiquetés par génération. Un résultat d’un compte précédent n’est pas affiché après changement de compte. Les accès disque, trousseau et démarrage automatique restent dans ce thread.
- `storage` : JSON écrit atomiquement, configuration sans identifiants, cache local borné à 30 jours / 43 200 échantillons par source (cadence d’une minute). Une empreinte SHA-256 lie le cache à l’identité de la connexion ; identifiants séparés par fournisseur dans Keychain / Credential Manager. L’ancien cache Dexcom est migré uniquement avec ses anciens identifiants. Un dossier QA personnalisé isole aussi le trousseau. Verrou d’instance dans le dossier de données.
- `ui` : thème partagé, composants SVG, dashboard, assistant initial, paramètres et viewport du widget.
- `platform` : tray natif et démarrage automatique. Sur macOS, clic gauche → aperçu egui transitoire, ancré sous le NSStatusItem avec les coordonnées logiques de son écran et borné à sa zone visible ; clic droit → menu natif. Échap et perte de focus ferment l’aperçu. Ses périodes sont indépendantes du dashboard et du widget. Pas de manipulation interne du shell.

La pile retenue est eframe/egui 0.33.3 + egui_plot 0.34.1, compatible avec Rust stable installé, avec rendu Glow/OpenGL et fonctionnalités minimales. Ce choix évite le moteur WGPU et son poids ; une validation de compatibilité GPU sur Windows reste nécessaire. Les polices système sont chargées au démarrage, avec repli vers les polices embarquées egui. Aucun frontend web, Electron ou Tauri ; tray-icon est une bibliothèque indépendante du framework Tauri.

## Exactitude et confidentialité

- Conversion d’affichage mmol/L = mg/dL ÷ 18, arrondie à une décimale ; mg/dL entier.
- Dates : WT/ST en millisecondes UTC pour Dexcom ; FactoryTimestamp UTC pour LibreLinkUp ; date UTC en millisecondes ou dateString RFC3339 pour Nightscout. Aucun remplacement par l’heure de récupération ni supposition sur une heure locale ambiguë.
- Une date future de plus de 60 secondes, une valeur hors enveloppe du fournisseur ou un payload mal formé rejette le lot entier.
- Dexcom : les sentinelles 39 et 401 mg/dL s’affichent comme `< 40` et `> 400`. Libre : limite explicite seulement avec indicateur fournisseur et valeur à la borne du capteur. Nightscout : les valeurs 39/401 restent exactes, sans supposer un capteur Dexcom. Aucune limite ne participe à une variation chiffrée.
- Ancienneté à partir de 10 minutes ; perte réseau et fraîcheur de mesure restent deux concepts distincts.
- Aucun remplissage des trous, pas de moyenne ou d’extrapolation inventée. La variation requiert deux mesures distantes de 45 secondes à 7,5 minutes.
- Rafraîchissement nominal toutes les 60 secondes pour LibreLinkUp et Nightscout/Nocturne ; Dexcom aligné sur dernière mesure + 310 secondes, minimum 60 secondes quand elle tarde, manuel limité à 30 secondes. Backoff de 30 à 900 secondes ; 30 minutes après limitation du service. Limitation mémorisée séparément par fournisseur. Identifiants refusés, conditions à accepter ou choix de personne : pas de tentative automatique répétée.
- Ni identifiants, ni session, ni corps de réponse, ni URL contenant une session dans les logs applicatifs.
- Simulation en mémoire, clairement identifiée dans dashboard/widget/barre de menus, jamais écrite dans le cache réel.
- Le cache contient des données de santé en clair dans le dossier utilisateur. Permissions 0700 du dossier sur macOS ; fichier temporaire privé. La protection au repos dépend aussi de FileVault/BitLocker et du compte OS. Pas de télémétrie.

## Interface et limites choisies

Fenêtre principale native redimensionnable (minimum 900 × 660), widget sans décoration en trois tailles, placement mémorisé et bouton de recentrage. Fond opaque volontaire pour la lisibilité : aucune option de transparence inactive. Menus du tray natifs pour la compatibilité et l’accessibilité. Le nombre est affiché comme titre sur macOS ; Windows utilise une icône et une infobulle plutôt qu’un nombre illisible dans 16 pixels.

Pas d’alarme médicale. L’application n’est pas un dispositif médical et ne doit pas guider un traitement. Les alarmes et applications officielles du capteur restent indispensables.

### Opacité des fenêtres macOS

Le démarrage demande un contexte OpenGL compatible avec la transparence, car eframe partage cette configuration avec le panneau arrondi. Dès la première frame, la commande `Transparent(false)` cible uniquement la fenêtre principale : AppKit rétablit son fond opaque, y compris la barre de titre. Le panneau conserve `with_transparent(true)` et le fond de rendu transparent ; ses coins restent évidés sans rectangle noir. Ne pas désactiver la transparence initiale du contexte partagé ni rendre globalement opaque `clear_color`.
