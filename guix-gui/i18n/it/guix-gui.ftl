app-title = Guix GUI

tab-home = Home
tab-search = Cerca
tab-installed = Installati
tab-updates = Aggiornamenti
tab-channels = Canali
tab-system = Impostazioni
tab-about = Informazioni

settings-language = Lingua
settings-language-system = Predefinita di sistema

# -- shared --
common-refresh = Aggiorna
common-cancel = Annulla
common-close = Chiudi
common-dismiss = Ignora
common-remove = Rimuovi
common-open-settings = Apri Impostazioni

# -- app shell / overlay --
app-discover-failed = Rilevamento di guix non riuscito
app-discovering = Rilevamento di guix...
app-brand = Guix
app-lightbox-close = Chiudi (Esc)
app-lightbox-no-image = nessuna immagine / non riuscita
app-running = In esecuzione...
app-copy = Copia
app-show-log = Mostra log
app-hide-log = Nascondi log
app-done = Fatto.
app-failed-exit = Non riuscito (uscita { $code }).
app-ended-no-summary = Terminato senza riepilogo di uscita.
app-op-title = { $label } (op #{ $id })
app-cancel-pkexec-tooltip = Impossibile annullare le operazioni privilegiate — il kernel non consente di inviare segnali ai processi di root. Attendi il completamento.
app-bootstrap-help =
    Riconfigurazione non riuscita: il Guix di sistema in esecuzione non riconosce un modulo
    importato dalla tua configurazione. Di solito significa che un canale è stato aggiornato dopo
    l'ultima riconfigurazione e il nuovo modulo non è ancora integrato nel Guix di sistema.

    Esegui il bootstrap una volta manualmente:

        sudo guix system reconfigure -L { $load } { $cfg }

    Dopodiché questo pulsante funzionerà per gli aggiornamenti successivi.
app-set-source-config-first = Imposta prima il percorso della configurazione sorgente nella scheda Sistema.
app-op-start-failed = Avvio dell'operazione non riuscito: { $error }
app-restore-panicked = Il task di ripristino è andato in panic: { $detail }
app-restore-failed = Ripristino non riuscito: { $detail }
app-discovery-client-failed = Client di rilevamento non riuscito: { $detail }

# -- op kinds --
op-install = Installazione
op-remove = Rimozione
op-upgrade = Aggiornamento dei pacchetti utente
op-pull = Recupero del catalogo utente
op-system-pull = Recupero del catalogo di sistema
op-reconfigure = Riconfigurazione del sistema

# -- stages --
stage-starting = Avvio
stage-channel-update = Aggiornamento dei canali
stage-computing-deriv = Calcolo della derivazione
stage-downloading = Download
stage-building = Compilazione
stage-profile = Aggiornamento del profilo
stage-done = Fatto
stage-failed = Non riuscito
stage-build-failed = Compilazione non riuscita: { $name }
stage-build-failed-log = Compilazione non riuscita: { $name } (log: { $log })

# -- categories --
category-graphics = Grafica
category-audio-video = Audio e video
category-office = Ufficio
category-development = Sviluppo
category-engineering = Ingegneria
category-internet = Internet
category-game = Giochi

# -- home --
home-title = Home
home-subtitle = Un punto di partenza — applicazioni note disponibili in Guix. Aprine una per installarla, oppure usa Cerca per il catalogo completo.
home-installed-badge = installato

# -- search --
search-title = Cerca
search-placeholder = Cerca pacchetti...
search-loading-catalog = Caricamento del catalogo dei pacchetti...
search-searching = Ricerca in corso...
search-results = { $count ->
    [one] { $count } risultato
   *[other] { $count } risultati
}
search-truncated = Mostrati i primi { $shown } di ≥{ $total } { $total ->
    [one] corrispondenza
   *[other] corrispondenze
}; affina la ricerca.
search-error-label = Errore di ricerca:
search-copy-details = Copia dettagli
search-select-prompt = Seleziona un pacchetto per vederne i dettagli.
search-homepage = homepage:
search-license = licenza: { $license }
search-outputs = output: { $outputs }
search-install = Installa
search-remove = Rimuovi
search-screenshots-flathub = Screenshot tramite Flathub ({ $id })
search-screenshots-debian = Screenshot tramite screenshots.debian.net
search-loading-media = Caricamento di icone / screenshot...
search-failed = Ricerca non riuscita.

# -- installed --
installed-title = Installati
installed-count = { $count ->
    [one] { $count } pacchetto installato
   *[other] { $count } pacchetti installati
}
installed-loading = Caricamento...
installed-error = Errore: { $error }

# -- updates --
updates-title = Aggiornamenti
updates-your-packages-blurb = Gestisci i tuoi pacchetti a livello utente.
updates-your-packages = I tuoi pacchetti
updates-fetch-latest = Recupera l'ultimo catalogo
updates-update-my-packages = Aggiorna i miei pacchetti
updates-system-blurb = Applica la configurazione di sistema. Richiede l'autenticazione come amministratore.
updates-system = Sistema
updates-source-config = Configurazione sorgente: { $path }
updates-source-config-unset = Configurazione sorgente: (non impostata — apri Impostazioni per sceglierla)
updates-fetch-system = Recupera il catalogo di sistema
updates-update-system = Aggiorna il sistema
# DO NOT TRANSLATE — literal shell command shown as a tooltip
updates-update-system-tip = pkexec guix system reconfigure
updates-confirm-reconfigure = Conferma la riconfigurazione del sistema
updates-reconfigure-blurb = Esecuzione come root tramite pkexec. Verifica i percorsi qui sotto — ognuno verrà caricato da Guile con privilegi di root.
updates-config = Configurazione:
updates-load-paths-none = Percorsi di caricamento (-L): (nessuno)
updates-load-paths = { $count ->
    [one] Percorsi di caricamento (-L), { $count } voce:
   *[other] Percorsi di caricamento (-L), { $count } voci:
}
updates-confirm-reconfigure-btn = Conferma riconfigurazione
updates-loading-channels = Caricamento dei canali...
updates-error-channels = Errore nel caricamento dei canali: { $error }
updates-last-pulled = Ultimo recupero: { $age }.
updates-last-pulled-never = Ultimo recupero: mai.
updates-channels-none = Canali: (nessuno rilevato).
updates-channels = Canali: { $list }.
updates-channel-no-commit = (nessun commit)
updates-last-pulled-root = Ultimo recupero (root): { $age }.
updates-last-pulled-root-never = Ultimo recupero (root): mai.
updates-last-reconfigured = Ultima riconfigurazione: { $age }.
updates-last-reconfigured-never = Ultima riconfigurazione: mai (non è un host Guix System?).

# -- about --
about-title = Informazioni
about-version = Versione { $version }
about-tagline = Interfaccia desktop per il gestore di pacchetti Guix.
about-authors = Autori
about-source = Codice sorgente e contributi
about-source-blurb = Le segnalazioni di bug e le pull request sono benvenute.
about-license = Licenza
about-license-line = Guix GUI è rilasciato sotto la GNU General Public License v3.0.
about-license-detail = Puoi ridistribuirlo e modificarlo secondo i termini di tale licenza. Consulta il file LICENSE nel repository per il testo completo.
about-third-party = Dati di terze parti
about-third-party-blurb = Le icone delle applicazioni e gli screenshot vengono recuperati da servizi esterni quando abiliti i metadati di terze parti nelle Impostazioni. Marchi, icone e screenshot restano proprietà dei rispettivi progetti.
about-channel-discovery = Rilevamento dei canali
about-channel-discovery-blurb = La modalità Scopri della scheda Canali sfoglia i canali e i pacchetti Guix indicizzati da toys.whereis.social. È facoltativa; richiede la rete. Il catalogo e i suoi contributori restano proprietà dei rispettivi progetti.
about-built-with = Realizzato con
about-built-with-detail = Le licenze dei singoli crate sono elencate nei rispettivi repository.

# -- progress overlay --
progress-last = Ultimo: { $line }
progress-running = In esecuzione ({ $count }):
progress-counts = { $built }/{ $started } compilati, { $dl_done }/{ $dl_started } scaricati ({ $mb } MB)
progress-build-line = { "  - " }{ $name } [building]
progress-build-item = { "  - " }{ $status ->
    [done] { $name } [done]
    [failed] { $name } [FAILED]
   *[other] { $name } [building]
}
progress-finished = Terminato ({ $done } completati, { $failed } non riusciti):
progress-and-more = ... e altri { $count }
progress-active-downloads = Download attivi ({ $count }):
progress-completed-downloads = Download completati ({ $count }):
progress-starting = Avvio...
progress-failed = Non riuscito.
progress-stage-ellipsis = { $stage }...

# -- system / settings --
system-title = Impostazioni
system-current-config = Configurazione di sistema attuale: { $path }
system-not-guix = Non su Guix System: { $error }
system-checking-config = Verifica della configurazione di sistema attuale...
system-no-config-banner = Nessun file di configurazione di sistema rilevato in /etc/config.scm o /etc/system.scm. Inserisci qui sotto il percorso della tua configurazione .scm.
system-validate = Convalida
system-config-heading = Configurazione di sistema
system-config-blurb = Percorso della tua configurazione di sistema .scm modificabile.
system-config-placeholder = /home/you/dotfiles/config.scm
system-validation-empty = Il percorso è vuoto.
system-validation-missing = Il percorso non esiste: { $path }
system-validation-not-file = Il percorso non è un file regolare: { $path }
system-validation-ok = OK: { $path }
system-load-paths-heading = Percorsi di caricamento aggiuntivi
system-load-paths-blurb = Directory aggiuntive in cui cercare durante la risoluzione degli import Scheme.
system-load-paths-auto = Automatico: { $path }
system-load-paths-auto-unset = Automatico: (imposta la configurazione di sistema qui sopra)
system-load-paths-placeholder = /path/to/extra/modules
system-add = + Aggiungi
system-section-system = SISTEMA
system-channels-heading = Canali
system-channels-blurb = Gestisci i canali a livello utente nella scheda dedicata.
system-channels-none = Nessun canale configurato.
system-channels-configured = { $count ->
    [one] { $count } canale configurato.
   *[other] { $count } canali configurati.
}
system-channels-unknown = Canali configurati: —
system-open-channels = Apri la scheda Canali
system-channels-source-heading = Percorso sorgente dei canali utente
system-channels-source-blurb = Override per ~/.config/guix/channels.scm. Necessario quando il percorso predefinito è gestito da `guix home` (si risolve in /gnu/store).
system-channels-source-placeholder = /home/you/dotfiles/channels.scm (lascia vuoto per il valore predefinito)
system-use-default = Usa predefinito
system-section-user-channels = CANALI UTENTE
system-metadata-heading = Icone e screenshot
system-metadata-blurb = Recupera icone e screenshot da cataloghi di terze parti per i risultati di ricerca selezionati. Facoltativo; richiede l'accesso alla rete.
system-metadata-enable = Abilita i metadati di terze parti
system-metadata-flathub = Flathub (flathub.org)
system-metadata-debian = screenshots.debian.net
system-cache-heading = Cache
system-cache-blurb = Le icone e gli screenshot vengono salvati su disco fino a un anno. Svuotala se un'icona appare sbagliata a monte.
system-cache-dir = Directory della cache: { $path }
system-cache-dir-none = Directory della cache: (nessuna directory cache XDG trovata — uso solo la memoria)
system-clear-cache = Svuota la cache
system-clearing-cache = Svuotamento della cache...
system-cache-cleared = Cache svuotata.
system-cache-clear-failed = Svuotamento della cache non riuscito: { $error }
system-discovery-heading = Rilevamento
system-discovery-toggle = Sfoglia canali e pacchetti da toys.whereis.social
system-discovery-blurb = Facoltativo. Richiede l'accesso alla rete. Quando è disattivato, il rilevamento non appare da nessuna parte nell'app.
system-section-metadata = METADATI

# -- channels --
channels-title = Canali
channels-intro = I canali sono sorgenti di pacchetti per Guix. Aggiungere un canale ti permette di installare il software che fornisce. Rimuoverne uno significa che i suoi pacchetti smettono di ricevere aggiornamenti.
channels-section-user = CANALI UTENTE
channels-submode-installed = Installati
channels-submode-discover = Scopri
channels-default-path = ~/.config/guix/channels.scm (predefinito)
channels-store-managed = gestito dallo store (sola lettura)
channels-writable = scrivibile
channels-file = File: { $path }
channels-confirm-restore = Conferma ripristino
channels-restore-last = Ripristina l'ultimo backup
channels-cant-edit-title = Questo file non può essere modificato qui
channels-cant-edit-blurb = Il tuo channels.scm è gestito da `guix home` (o da un altro strumento) e non può essere modificato direttamente. Per usare guix-gui per le modifiche ai canali, imposta un file scrivibile in { $settings_tab } → { $channels_tab }.
channels-saving = Salvataggio...
channels-pull-then-install = Esegui il pull, poi installa { $pkg }
channels-pull-only = Solo pull
channels-pull-now = Esegui il pull ora
channels-keep-changes = Mantieni le modifiche
channels-pull-failed-shadow = Pull non riuscito — bug di shadowing dei canali (#74396).
channels-pull-failed = Pull non riuscito.
channels-rollback-blurb = Il tuo channels.scm contiene le nuove modifiche ma Guix non è riuscito a recuperarle. Ripristinare il channels.scm precedente?
channels-rollback-none = Nessun channels.scm precedente da ripristinare.
channels-restore-previous = Ripristina il precedente
channels-empty-title = Nessun channels.scm trovato
channels-empty-blurb = Aggiungi un canale qui sotto per crearne uno. Il file si trova in ~/.config/guix/channels.scm per impostazione predefinita.
channels-error = Errore
channels-loading = Caricamento di channels.scm...
channels-count = { $count ->
    [one] { $count } canale
   *[other] { $count } canali
}
channels-none-in-file = Nessun canale in questo file.
channels-inherited-title = Inclusi anche dai tuoi canali
channels-inherited-blurb = Questi provengono dai canali qui sopra e sono gestiti da essi.
channels-branch = branch: { $branch }
channels-commit = commit: { $commit }
channels-introduction = introduzione: { $fpr }
channels-no-fingerprint = (nessuna fingerprint)
channels-introduction-none = introduzione: (nessuna)
channels-remove-title = Rimuovere il canale `{ $name }`?
channels-remove-intro = { $count ->
    [one] { $count } pacchetto installato proviene da questo canale:
   *[other] { $count } pacchetti installati provengono da questo canale:
}
channels-remove-explainer = Questi continueranno a funzionare ma non riceveranno aggiornamenti dopo la rimozione del canale.
channels-remove-cmd-label = Per disinstallarli insieme al canale:
channels-remove-anyway = Rimuovi comunque il canale
channels-locked = bloccato
channels-see-warning = vedi l'avviso qui sotto
channels-confirm-remove = Conferma rimozione
channels-add-heading = Aggiungi un canale
channels-add-blurb = Tutti i campi vengono memorizzati testualmente; il commit di introduzione e la fingerprint sono obbligatori.
channels-add-name = Nome
channels-add-name-placeholder = es. nonguix
channels-add-url = URL
channels-add-url-placeholder = https://gitlab.com/nonguix/nonguix
channels-add-branch = Branch
channels-add-branch-placeholder = master (opzionale)
channels-add-commit = Commit
channels-add-commit-placeholder = hash del commit (opzionale)
channels-add-intro-commit = Commit di introduzione
channels-add-intro-commit-placeholder = hash del commit di introduzione
channels-add-intro-fpr = Fingerprint di introduzione
channels-add-intro-fpr-placeholder = fingerprint OpenPGP (es. 2A39 3FFF 68F4 EF7A 3D29 ...)
channels-add-btn = Aggiungi canale
channels-discover-placeholder = Cerca pacchetti o canali...
channels-searching = Ricerca in corso...
channels-package-results = { $count ->
    [one] { $count } risultato di pacchetto
   *[other] { $count } risultati di pacchetto
}
channels-from = da { $channel }
channels-packages = Pacchetti
channels-channels-heading = Canali
channels-no-synopsis = (nessuna sinossi)
channels-install = Installa
channels-add-and-install = Aggiungi canale e installa
channels-loading-discover = Caricamento dei canali...
channels-no-introduced = Nessun canale introdotto è stato restituito.
channels-available = { $count ->
    [one] { $count } canale disponibile
   *[other] { $count } canali disponibili
}
channels-already-added = già aggiunto
channels-add = Aggiungi
channels-pkgs = { $count ->
    [one] { $count } pkg
   *[other] { $count } pkgs
}
channels-svcs = { $count ->
    [one] { $count } svc
   *[other] { $count } svcs
}
channels-intro-dash = intro: —
channels-intro-short = intro: { $fpr }...
channels-set-writable-tooltip = Imposta un file scrivibile nelle Impostazioni
channels-confirm-add-title = Conferma aggiunta canale
channels-confirm-add-blurb = Questo aggiungerà il canale al tuo channels.scm e convaliderà il file prima di salvare.
channels-provenance = Provenienza
channels-supplied-by = Fornito da { $source }
channels-trust-warning = Una volta aggiunto, ogni `guix pull` eseguirà codice Guile da questa sorgente con i tuoi permessi. Verifica il commit di introduzione e la fingerprint qui sotto rispetto ai valori pubblicati dal canale stesso prima di aggiungerlo.
channels-field-name = nome
channels-field-url = url
channels-field-branch = branch
channels-field-commit = commit
channels-field-intro-commit = commit di introduzione
channels-field-intro-fpr = fingerprint di introduzione

# -- channels status messages --
channels-restored = Ripristinato dal backup.
channels-added-install-prompt = Canale aggiunto. Eseguire il pull e poi installare { $pkg }?
channels-updated = Canali aggiornati. Esegui il pull ora per recuperare il nuovo catalogo.
channels-no-file-loaded = Nessun channels.scm caricato; aggiorna prima la scheda.
channels-store-managed-error = il channels.scm in { $path } è gestito dallo store. Imposta un override del percorso sorgente scrivibile nelle Impostazioni.
channels-no-backup = Nessun file di backup presente.
channels-form-name-required = Il nome è obbligatorio.
channels-form-url-required = L'URL è obbligatorio.
channels-form-intro-required = Il commit di introduzione e la fingerprint sono obbligatori.
channels-vanished-after-write = channels.scm è scomparso dopo la scrittura — aggiorna la scheda.
