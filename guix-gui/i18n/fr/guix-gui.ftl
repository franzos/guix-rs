app-title = Guix GUI

tab-home = Accueil
tab-search = Rechercher
tab-installed = Installés
tab-updates = Mises à jour
tab-channels = Canaux
tab-system = Réglages
tab-about = À propos

settings-language = Langue
settings-language-system = Réglage du système

# -- shared --
common-refresh = Actualiser
common-cancel = Annuler
common-close = Fermer
common-dismiss = Ignorer
common-remove = Supprimer
common-open-settings = Ouvrir les réglages

# -- app shell / overlay --
app-discover-failed = Échec de la détection de guix
app-discovering = Détection de guix...
app-brand = Guix
app-lightbox-close = Fermer (Échap)
app-lightbox-no-image = pas d'image / échec
app-running = En cours...
app-copy = Copier
app-show-log = Afficher le journal
app-hide-log = Masquer le journal
app-done = Terminé.
app-failed-exit = Échec (sortie { $code }).
app-ended-no-summary = Terminé sans résumé de sortie.
app-op-title = { $label } (op n°{ $id })
app-cancel-pkexec-tooltip = Impossible d'annuler les opérations privilégiées — le noyau n'autorise pas l'envoi de signaux aux processus appartenant à root. Attends qu'elle se termine.
app-bootstrap-help =
    Échec de la reconfiguration : le Guix du système en cours ne reconnaît pas un
    module importé par ta configuration. Cela signifie généralement qu'un canal a
    été mis à jour depuis ta dernière reconfiguration et que le nouveau module
    n'est pas encore intégré au Guix du système.

    Amorce-le une fois manuellement :

        sudo guix system reconfigure -L { $load } { $cfg }

    Ensuite, ce bouton fonctionnera pour les mises à jour suivantes.
app-set-source-config-first = Définis d'abord le chemin de la config source dans l'onglet Système.
app-op-start-failed = Échec du démarrage de l'op : { $error }
app-restore-panicked = La tâche de restauration a paniqué : { $detail }
app-restore-failed = Échec de la restauration : { $detail }
app-discovery-client-failed = Échec du client de découverte : { $detail }

# -- op kinds --
op-install = Installation
op-remove = Suppression
op-upgrade = Mise à jour des paquets utilisateur
op-pull = Récupération du catalogue utilisateur
op-system-pull = Récupération du catalogue système
op-reconfigure = Reconfiguration du système

# -- stages --
stage-starting = Démarrage
stage-channel-update = Mise à jour des canaux
stage-computing-deriv = Calcul de la dérivation
stage-downloading = Téléchargement
stage-building = Construction
stage-profile = Mise à jour du profil
stage-done = Terminé
stage-failed = Échec
stage-build-failed = Échec de la construction : { $name }
stage-build-failed-log = Échec de la construction : { $name } (journal : { $log })

# -- categories --
category-graphics = Graphisme
category-audio-video = Audio et vidéo
category-office = Bureautique
category-development = Développement
category-engineering = Ingénierie
category-internet = Internet
category-game = Jeux

# -- home --
home-title = Accueil
home-subtitle = Un point de départ — des applications bien connues disponibles dans Guix. Ouvres-en une pour l'installer, ou utilise la recherche pour le catalogue complet.
home-installed-badge = installé

# -- search --
search-title = Rechercher
search-placeholder = Rechercher des paquets...
search-loading-catalog = Chargement du catalogue de paquets...
search-searching = Recherche...
search-results = { $count ->
    [one] { $count } résultat
   *[other] { $count } résultats
}
search-truncated = Affichage des { $shown } premiers sur ≥{ $total } { $total ->
    [one] correspondance
   *[other] correspondances
} ; affine ta requête.
search-error-label = Erreur de recherche :
search-copy-details = Copier les détails
search-select-prompt = Sélectionne un paquet pour voir les détails.
search-homepage = page d'accueil :
search-license = licence : { $license }
search-outputs = sorties : { $outputs }
search-install = Installer
search-remove = Supprimer
search-screenshots-flathub = Captures d'écran via Flathub ({ $id })
search-screenshots-debian = Captures d'écran via screenshots.debian.net
search-loading-media = Chargement des icônes / captures d'écran...
search-failed = Échec de la recherche.

# -- installed --
installed-title = Installés
installed-count = { $count ->
    [one] { $count } paquet installé
   *[other] { $count } paquets installés
}
installed-loading = Chargement...
installed-error = Erreur : { $error }

# -- updates --
updates-title = Mises à jour
updates-your-packages-blurb = Gère tes paquets au niveau utilisateur.
updates-your-packages = Tes paquets
updates-fetch-latest = Récupérer le dernier catalogue
updates-update-my-packages = Mettre à jour mes paquets
updates-system-blurb = Applique ta configuration système. Nécessite une authentification administrateur.
updates-system = Système
updates-source-config = Config source : { $path }
updates-source-config-unset = Config source : (non définie — ouvre les réglages pour choisir)
updates-fetch-system = Récupérer le catalogue système
updates-update-system = Mettre à jour le système
# DO NOT TRANSLATE — literal shell command shown as a tooltip
updates-update-system-tip = pkexec guix system reconfigure
updates-confirm-reconfigure = Confirmer la reconfiguration du système
updates-reconfigure-blurb = Exécution en tant que root via pkexec. Vérifie les chemins ci-dessous — chacun sera chargé par Guile avec les privilèges root.
updates-config = Config :
updates-load-paths-none = Chemins de chargement (-L) : (aucun)
updates-load-paths = { $count ->
    [one] Chemins de chargement (-L), { $count } entrée :
   *[other] Chemins de chargement (-L), { $count } entrées :
}
updates-confirm-reconfigure-btn = Confirmer la reconfiguration
updates-loading-channels = Chargement des canaux...
updates-error-channels = Erreur lors du chargement des canaux : { $error }
updates-last-pulled = Dernière récupération : { $age }.
updates-last-pulled-never = Dernière récupération : jamais.
updates-channels-none = Canaux : (aucun détecté).
updates-channels = Canaux : { $list }.
updates-channel-no-commit = (pas de commit)
updates-last-pulled-root = Dernière récupération (root) : { $age }.
updates-last-pulled-root-never = Dernière récupération (root) : jamais.
updates-last-reconfigured = Dernière reconfiguration : { $age }.
updates-last-reconfigured-never = Dernière reconfiguration : jamais (pas un hôte Guix System ?).

# -- about --
about-title = À propos
about-version = Version { $version }
about-tagline = Interface de bureau pour le gestionnaire de paquets Guix.
about-authors = Auteurs
about-source = Source et contributions
about-source-blurb = Les rapports de bogues et les pull requests sont les bienvenus.
about-license = Licence
about-license-line = Guix GUI est publié sous la licence publique générale GNU v3.0.
about-license-detail = Tu peux le redistribuer et le modifier selon les termes de cette licence. Consulte le fichier LICENSE dans le dépôt pour le texte complet.
about-third-party = Données tierces
about-third-party-blurb = Les icônes et captures d'écran des applications sont récupérées auprès de services externes lorsque tu actives les métadonnées tierces dans les réglages. Les marques, icônes et captures d'écran restent la propriété de leurs projets respectifs.
about-channel-discovery = Découverte de canaux
about-channel-discovery-blurb = Le sous-mode Découvrir de l'onglet Canaux parcourt les canaux et paquets Guix indexés par toys.whereis.social. Sur option ; nécessite le réseau. Le catalogue et ses contributeurs restent la propriété de leurs projets respectifs.
about-built-with = Construit avec
about-built-with-detail = Les licences des crates individuelles sont listées dans leurs dépôts respectifs.

# -- progress overlay --
progress-last = Dernier : { $line }
progress-running = En cours ({ $count }) :
progress-counts = { $built }/{ $started } construits, { $dl_done }/{ $dl_started } téléchargés ({ $mb } MB)
progress-build-line = { "  - " }{ $name } [building]
progress-build-item = { "  - " }{ $status ->
    [done] { $name } [done]
    [failed] { $name } [FAILED]
   *[other] { $name } [building]
}
progress-finished = Terminé ({ $done } réussis, { $failed } échoués) :
progress-and-more = ... et { $count } de plus
progress-active-downloads = Téléchargements actifs ({ $count }) :
progress-completed-downloads = Téléchargements terminés ({ $count }) :
progress-starting = Démarrage...
progress-failed = Échec.
progress-stage-ellipsis = { $stage }...

# -- system / settings --
system-title = Réglages
system-current-config = Config système actuelle : { $path }
system-not-guix = Pas sur Guix System : { $error }
system-checking-config = Vérification de la config système actuelle...
system-no-config-banner = Aucun fichier de configuration système détecté dans /etc/config.scm ou /etc/system.scm. Saisis ci-dessous le chemin de ta configuration .scm.
system-validate = Valider
system-config-heading = Config système
system-config-blurb = Chemin vers ta configuration système .scm modifiable.
system-config-placeholder = /home/you/dotfiles/config.scm
system-validation-empty = Le chemin est vide.
system-validation-missing = Le chemin n'existe pas : { $path }
system-validation-not-file = Le chemin n'est pas un fichier régulier : { $path }
system-validation-ok = OK : { $path }
system-load-paths-heading = Chemins de chargement supplémentaires
system-load-paths-blurb = Répertoires additionnels à parcourir pour résoudre les imports Scheme.
system-load-paths-auto = Auto : { $path }
system-load-paths-auto-unset = Auto : (définis la config système ci-dessus)
system-load-paths-placeholder = /chemin/vers/modules/supplementaires
system-add = + Ajouter
system-section-system = SYSTÈME
system-channels-heading = Canaux
system-channels-blurb = Gère les canaux au niveau utilisateur dans l'onglet dédié.
system-channels-none = Aucun canal configuré.
system-channels-configured = { $count ->
    [one] { $count } canal configuré.
   *[other] { $count } canaux configurés.
}
system-channels-unknown = Canaux configurés : —
system-open-channels = Ouvrir l'onglet Canaux
system-channels-source-heading = Chemin source des canaux utilisateur
system-channels-source-blurb = Remplacement pour ~/.config/guix/channels.scm. Requis lorsque le chemin par défaut est géré par `guix home` (résout dans /gnu/store).
system-channels-source-placeholder = /home/you/dotfiles/channels.scm (laisser vide pour la valeur par défaut)
system-use-default = Utiliser la valeur par défaut
system-section-user-channels = CANAUX UTILISATEUR
system-metadata-heading = Icônes et captures d'écran
system-metadata-blurb = Récupère les icônes et captures d'écran depuis des catalogues tiers pour les résultats de recherche sélectionnés. Sur option ; nécessite un accès réseau.
system-metadata-enable = Activer les métadonnées tierces
system-metadata-flathub = Flathub (flathub.org)
system-metadata-debian = screenshots.debian.net
system-cache-heading = Cache
system-cache-blurb = Les icônes et captures d'écran sont enregistrées sur le disque pendant un an au maximum. Vide-le si une icône semble incorrecte en amont.
system-cache-dir = Répertoire de cache : { $path }
system-cache-dir-none = Répertoire de cache : (aucun répertoire de cache XDG trouvé — utilisation en mémoire uniquement)
system-clear-cache = Vider le cache
system-clearing-cache = Vidage du cache...
system-cache-cleared = Cache vidé.
system-cache-clear-failed = Échec du vidage du cache : { $error }
system-discovery-heading = Découverte
system-discovery-toggle = Parcourir les canaux et paquets depuis toys.whereis.social
system-discovery-blurb = Sur option. Nécessite un accès réseau. Désactivée, la découverte n'apparaît nulle part dans l'application.
system-desktop-refresh = Actualiser le menu des applications après l'installation d'apps
system-desktop-refresh-desc = Reconstruit le menu du bureau pour que les applications nouvellement installées apparaissent immédiatement (KDE, XFCE, MATE, LXQt). Désactivez si vous préférez actualiser manuellement.
system-section-metadata = MÉTADONNÉES

# -- channels --
channels-title = Canaux
channels-intro = Les canaux sont des sources de paquets pour Guix. Ajouter un canal te permet d'installer les logiciels qu'il fournit. En supprimer un signifie que ses paquets cessent de recevoir des mises à jour.
channels-section-user = CANAUX UTILISATEUR
channels-submode-installed = Installés
channels-submode-discover = Découvrir
channels-default-path = ~/.config/guix/channels.scm (par défaut)
channels-store-managed = géré par le store (lecture seule)
channels-writable = modifiable
channels-file = Fichier : { $path }
channels-confirm-restore = Confirmer la restauration
channels-restore-last = Restaurer la dernière sauvegarde
channels-cant-edit-title = Ce fichier ne peut pas être modifié ici
channels-cant-edit-blurb = Ton channels.scm est géré par `guix home` (ou un autre outil) et ne peut pas être modifié directement. Pour utiliser guix-gui pour les changements de canaux, définis un fichier modifiable dans { $settings_tab } → { $channels_tab }.
channels-saving = Enregistrement...
channels-pull-then-install = Récupérer, puis installer { $pkg }
channels-pull-only = Récupérer seulement
channels-pull-now = Récupérer maintenant
channels-keep-changes = Conserver les changements
channels-pull-failed-shadow = Échec de la récupération — bogue d'occultation de canal (#74396).
channels-pull-failed = Échec de la récupération.
channels-rollback-blurb = Ton channels.scm contient les nouveaux changements mais Guix n'a pas pu les récupérer. Restaurer le channels.scm précédent ?
channels-rollback-none = Aucun channels.scm précédent à restaurer.
channels-restore-previous = Restaurer le précédent
channels-empty-title = Aucun channels.scm trouvé
channels-empty-blurb = Ajoute un canal ci-dessous pour en créer un. Le fichier se trouve dans ~/.config/guix/channels.scm par défaut.
channels-error = Erreur
channels-loading = Chargement de channels.scm...
channels-count = { $count ->
    [one] { $count } canal
   *[other] { $count } canaux
}
channels-none-in-file = Aucun canal dans ce fichier.
channels-inherited-title = Également tirés par tes canaux
channels-inherited-blurb = Ceux-ci proviennent des canaux ci-dessus et sont gérés par eux.
channels-branch = branche : { $branch }
channels-commit = commit : { $commit }
channels-introduction = introduction : { $fpr }
channels-no-fingerprint = (pas d'empreinte)
channels-introduction-none = introduction : (aucune)
channels-remove-title = Supprimer le canal `{ $name }` ?
channels-remove-intro = { $count ->
    [one] { $count } paquet installé provient de ce canal :
   *[other] { $count } paquets installés proviennent de ce canal :
}
channels-remove-explainer = Ceux-ci continueront de fonctionner mais ne recevront plus de mises à jour après la suppression du canal.
channels-remove-cmd-label = Pour les désinstaller en même temps que le canal :
channels-remove-anyway = Supprimer le canal quand même
channels-locked = verrouillé
channels-see-warning = voir l'avertissement ci-dessous
channels-confirm-remove = Confirmer la suppression
channels-add-heading = Ajouter un canal
channels-add-blurb = Tous les champs sont stockés tels quels ; le commit d'introduction et l'empreinte sont requis.
channels-add-name = Nom
channels-add-name-placeholder = ex. nonguix
channels-add-url = URL
channels-add-url-placeholder = https://gitlab.com/nonguix/nonguix
channels-add-branch = Branche
channels-add-branch-placeholder = master (optionnel)
channels-add-commit = Commit
channels-add-commit-placeholder = hash de commit (optionnel)
channels-add-intro-commit = Commit d'introduction
channels-add-intro-commit-placeholder = hash du commit d'introduction
channels-add-intro-fpr = Empreinte d'introduction
channels-add-intro-fpr-placeholder = empreinte OpenPGP (ex. 2A39 3FFF 68F4 EF7A 3D29 ...)
channels-add-btn = Ajouter le canal
channels-discover-placeholder = Rechercher des paquets ou des canaux...
channels-searching = Recherche...
channels-package-results = { $count ->
    [one] { $count } résultat de paquet
   *[other] { $count } résultats de paquets
}
channels-from = de { $channel }
channels-packages = Paquets
channels-channels-heading = Canaux
channels-no-synopsis = (pas de synopsis)
channels-install = Installer
channels-add-and-install = Ajouter le canal et installer
channels-loading-discover = Chargement des canaux...
channels-no-introduced = Aucun canal introduit n'a été retourné.
channels-available = { $count ->
    [one] { $count } canal disponible
   *[other] { $count } canaux disponibles
}
channels-already-added = déjà ajouté
channels-add = Ajouter
channels-pkgs = { $count ->
    [one] { $count } pkg
   *[other] { $count } pkgs
}
channels-svcs = { $count ->
    [one] { $count } svc
   *[other] { $count } svcs
}
channels-intro-dash = intro : —
channels-intro-short = intro : { $fpr }...
channels-set-writable-tooltip = Définis un fichier modifiable dans les réglages
channels-confirm-add-title = Confirmer l'ajout du canal
channels-confirm-add-blurb = Ceci ajoutera le canal à ton channels.scm et validera le fichier avant l'enregistrement.
channels-provenance = Provenance
channels-supplied-by = Fourni par { $source }
channels-trust-warning = Une fois ajouté, chaque `guix pull` exécute le code Guile de cette source en ton nom. Vérifie le commit d'introduction et l'empreinte ci-dessous par rapport aux valeurs publiées par le canal lui-même avant de l'ajouter.
channels-field-name = nom
channels-field-url = url
channels-field-branch = branche
channels-field-commit = commit
channels-field-intro-commit = commit d'intro
channels-field-intro-fpr = empreinte d'intro

# -- channels status messages --
channels-restored = Restauré depuis la sauvegarde.
channels-added-install-prompt = Canal ajouté. Récupérer, puis installer { $pkg } ?
channels-updated = Canaux mis à jour. Récupère maintenant pour obtenir le nouveau catalogue.
channels-no-file-loaded = Aucun channels.scm chargé ; actualise d'abord l'onglet.
channels-store-managed-error = channels.scm dans { $path } est géré par le store. Définis un remplacement de chemin source modifiable dans les réglages.
channels-no-backup = Aucun fichier de sauvegarde présent.
channels-form-name-required = Le nom est requis.
channels-form-url-required = L'URL est requise.
channels-form-intro-required = Le commit d'introduction et l'empreinte sont requis.
channels-vanished-after-write = channels.scm a disparu après l'écriture — actualise l'onglet.
