# Sources de données

L’onglet **Sources de données** affiche un sélecteur de fournisseur et uniquement les champs nécessaires. Une seule source est active à la fois. Revenir à une source restaure ses identifiants enregistrés et son cache correspondant ; se déconnecter supprime uniquement cette source et son historique local.

## LibreLinkUp

Utiliser le compte de suivi LibreLinkUp, ayant accepté une invitation du porteur. E-mail et mot de passe sont nécessaires. Une seule personne est sélectionnée automatiquement ; plusieurs personnes nécessitent un choix explicite. Une personne précédemment sélectionnée qui disparaît n’est jamais remplacée silencieusement.

Le client commence sur le serveur européen, suit la région fournie par Abbott et négocie la version minimale si nécessaire. Il transmet le SHA-256 de l’identifiant utilisateur dans Account-Id, conserve le jeton de session en mémoire et renouvelle une session expirée une seule fois. Les conditions et étapes de connexion doivent être terminées dans LibreLinkUp, jamais acceptées automatiquement ici.

Lecture de la connexion et du graphique, sans écriture. ValueInMgPerDl est utilisé quelle que soit l’unité affichée par le compte. FactoryTimestamp est interprété en UTC ; le champ Timestamp local seul est refusé. Les codes TrendArrow Libre sont distincts de Dexcom. Historique initial limité à ce que renvoie LibreLinkUp, puis conservation locale ; aucune mesure manquante n’est inventée.

Références de protocole étudiées le 17 novembre 2026 : [GlucoBar LibreLinkUp](https://github.com/jackfaz/glucobar/blob/main/glucobar/librelinkup.py), [configuration GlucoBar](https://github.com/jackfaz/glucobar/blob/main/glucobar/config.py), [nightscout-librelink-up](https://github.com/timoschlueter/nightscout-librelink-up/blob/master/src/index.ts), [librelinkup-api](https://github.com/luancyrne/librelinkup-api). Protocole non officiel susceptible de changer.

## Nightscout / Nocturne

Renseigner l’adresse HTTPS du site et un jeton autorisant la lecture des mesures. Nocturne utilise le même connecteur via son API compatible Nightscout v1. Un site public peut fonctionner sans jeton. Le mode API secret envoie le SHA-1 du secret dans l’en-tête api-secret ; le mode jeton utilise le paramètre token. Aucune écriture de mesure, traitement ou profil.

Le client lit `/api/v1/entries/sgv.json` sur les dernières 24 heures, avec un plafond de 2 000 entrées et un rafraîchissement d’une minute. Il accepte un préfixe de chemin et une adresse se terminant par `/api/v1`. Les URL contenant des identifiants, paramètres ou fragments sont refusées : saisir le jeton dans son champ dédié. HTTP est autorisé uniquement sur la boucle locale pour les essais ; les redirections HTTP ne sont pas suivies. Une redirection de domaine nécessite donc l’adresse HTTPS finale du serveur.

Références : [API entries Nightscout](https://github.com/nightscout/cgm-remote-monitor/blob/master/lib/api/entries/index.js), [compatibilité Nocturne](https://github.com/nightscout/nocturne), [guide Nocturne](https://nocturne.run/guides/connect-your-data-to-nocturne).

## Données et validation

Configuration sans mot de passe ni jeton. Trousseau séparé par fournisseur ; caches séparés, liés par empreinte au compte/personne ou au site/jeton. Aucun secret, corps de réponse ou URL contenant un jeton dans les erreurs affichées ou les logs applicatifs. Les erreurs d’authentification et demandes d’action suspendent les nouvelles tentatives automatiques. Un changement de source efface immédiatement les mesures affichées avant de charger celles de la source choisie.

Les serveurs HTTP de test sont locaux et utilisent exclusivement des données fictives. La connexion à des comptes réels LibreLinkUp, Nightscout, Nocturne et Dexcom reste à vérifier avec leurs propriétaires. La compatibilité Nocturne repose sur son API v1 annoncée et les fixtures de contrat ; aucune instance Nocturne réelle n’a été utilisée.
