app-title = Guix GUI

tab-home = Start
tab-search = Suche
tab-installed = Installiert
tab-updates = Updates
tab-channels = Kanäle
tab-system = Einstellungen
tab-about = Über

settings-language = Sprache
settings-language-system = Systemstandard

# -- shared --
common-refresh = Aktualisieren
common-cancel = Abbrechen
common-close = Schließen
common-dismiss = Ausblenden
common-remove = Entfernen
common-open-settings = Einstellungen öffnen

# -- app shell / overlay --
app-discover-failed = Guix konnte nicht gefunden werden
app-discovering = Suche nach Guix...
app-brand = Guix
app-lightbox-close = Schließen (Esc)
app-lightbox-no-image = kein Bild / fehlgeschlagen
app-running = Läuft...
app-copy = Kopieren
app-show-log = Log anzeigen
app-hide-log = Log ausblenden
app-done = Fertig.
app-failed-exit = Fehlgeschlagen (Exit { $code }).
app-ended-no-summary = Beendet ohne Exit-Zusammenfassung.
app-op-title = { $label } (Op #{ $id })
app-cancel-pkexec-tooltip = Privilegierte Operationen lassen sich nicht abbrechen — der Kernel erlaubt kein Signalisieren von Prozessen, die root gehören. Warte, bis sie fertig sind.
app-bootstrap-help =
    Reconfigure fehlgeschlagen: das laufende System-Guix kennt ein Modul nicht,
    das deine Config importiert. Das bedeutet meist, dass ein Kanal nach deinem
    letzten Reconfigure aktualisiert wurde und das neue Modul noch nicht im
    System-Guix steckt.

    Einmal manuell bootstrappen:

        sudo guix system reconfigure -L { $load } { $cfg }

    Danach funktioniert dieser Button für künftige Updates.
app-set-source-config-first = Lege zuerst den Pfad zur Quell-Config im System-Tab fest.
app-op-start-failed = Op konnte nicht gestartet werden: { $error }
app-restore-panicked = Wiederherstellungs-Task abgestürzt: { $detail }
app-restore-failed = Wiederherstellung fehlgeschlagen: { $detail }
app-discovery-client-failed = Discovery-Client fehlgeschlagen: { $detail }

# -- op kinds --
op-install = Installiere
op-remove = Entferne
op-upgrade = Aktualisiere Benutzerpakete
op-pull = Hole Benutzerkatalog
op-system-pull = Hole Systemkatalog
op-reconfigure = Konfiguriere System neu

# -- stages --
stage-starting = Starte
stage-channel-update = Aktualisiere Kanäle
stage-computing-deriv = Berechne Derivation
stage-downloading = Lade herunter
stage-building = Baue
stage-profile = Aktualisiere Profil
stage-done = Fertig
stage-failed = Fehlgeschlagen
stage-build-failed = Build fehlgeschlagen: { $name }
stage-build-failed-log = Build fehlgeschlagen: { $name } (Log: { $log })

# -- categories --
category-graphics = Grafik
category-audio-video = Audio & Video
category-office = Büro
category-development = Entwicklung
category-engineering = Technik
category-internet = Internet
category-game = Spiele

# -- home --
home-title = Start
home-subtitle = Ein Einstiegspunkt — bekannte Anwendungen, die in Guix verfügbar sind. Öffne eine zum Installieren oder nutze die Suche für den vollständigen Katalog.
home-installed-badge = installiert

# -- search --
search-title = Suche
search-placeholder = Pakete suchen...
search-loading-catalog = Lade Paketkatalog...
search-searching = Suche...
search-results = { $count ->
    [one] { $count } Ergebnis
   *[other] { $count } Ergebnisse
}
search-truncated = Zeige die ersten { $shown } von ≥{ $total } { $total ->
    [one] Treffer
   *[other] Treffern
}; verfeinere deine Suche.
search-error-label = Suchfehler:
search-copy-details = Details kopieren
search-select-prompt = Wähle ein Paket, um Details zu sehen.
search-homepage = Homepage:
search-license = Lizenz: { $license }
search-outputs = Outputs: { $outputs }
search-install = Installieren
search-remove = Entfernen
search-screenshots-flathub = Screenshots via Flathub ({ $id })
search-screenshots-debian = Screenshots via screenshots.debian.net
search-loading-media = Lade Icons / Screenshots...
search-failed = Suche fehlgeschlagen.

# -- installed --
installed-title = Installiert
installed-count = { $count ->
    [one] { $count } installiertes Paket
   *[other] { $count } installierte Pakete
}
installed-loading = Lade...
installed-error = Fehler: { $error }

# -- updates --
updates-title = Updates
updates-your-packages-blurb = Verwalte deine Pakete auf Benutzerebene.
updates-your-packages = Deine Pakete
updates-fetch-latest = Neuesten Katalog holen
updates-update-my-packages = Meine Pakete aktualisieren
updates-system-blurb = Wende deine Systemkonfiguration an. Erfordert Admin-Authentifizierung.
updates-system = System
updates-source-config = Quell-Config: { $path }
updates-source-config-unset = Quell-Config: (nicht gesetzt — öffne Einstellungen zum Auswählen)
updates-fetch-system = Systemkatalog holen
updates-update-system = System aktualisieren
# DO NOT TRANSLATE — literal shell command shown as a tooltip
updates-update-system-tip = pkexec guix system reconfigure
updates-confirm-reconfigure = System-Reconfigure bestätigen
updates-reconfigure-blurb = Läuft als root via pkexec. Überprüfe die Pfade unten — jeder wird von Guile mit root-Rechten geladen.
updates-config = Config:
updates-load-paths-none = Load-Pfade (-L): (keine)
updates-load-paths = { $count ->
    [one] Load-Pfade (-L), { $count } Eintrag:
   *[other] Load-Pfade (-L), { $count } Einträge:
}
updates-confirm-reconfigure-btn = Reconfigure bestätigen
updates-loading-channels = Lade Kanäle...
updates-error-channels = Fehler beim Laden der Kanäle: { $error }
updates-last-pulled = Zuletzt geholt: { $age }.
updates-last-pulled-never = Zuletzt geholt: nie.
updates-channels-none = Kanäle: (keine gefunden).
updates-channels = Kanäle: { $list }.
updates-channel-no-commit = (kein Commit)
updates-last-pulled-root = Zuletzt geholt (root): { $age }.
updates-last-pulled-root-never = Zuletzt geholt (root): nie.
updates-last-reconfigured = Zuletzt neu konfiguriert: { $age }.
updates-last-reconfigured-never = Zuletzt neu konfiguriert: nie (kein Guix-System-Host?).

# -- about --
about-title = Über
about-version = Version { $version }
about-tagline = Desktop-Frontend für den Guix-Paketmanager.
about-authors = Autoren
about-source = Quellcode & Beiträge
about-source-blurb = Fehlerberichte und Pull-Requests sind willkommen.
about-license = Lizenz
about-license-line = Guix GUI wird unter der GNU General Public License v3.0 veröffentlicht.
about-license-detail = Du darfst es unter den Bedingungen dieser Lizenz weitergeben und verändern. Den vollständigen Text findest du in der LICENSE-Datei im Repository.
about-third-party = Daten von Drittanbietern
about-third-party-blurb = Anwendungs-Icons und Screenshots werden von externen Diensten geholt, wenn du Metadaten von Drittanbietern in den Einstellungen aktivierst. Marken, Icons und Screenshots bleiben Eigentum der jeweiligen Projekte.
about-channel-discovery = Kanal-Discovery
about-channel-discovery-blurb = Der Discover-Untermodus im Kanäle-Tab durchsucht Guix-Kanäle und -Pakete, die von toys.whereis.social indexiert werden. Opt-in; erfordert Netzwerk. Der Katalog und seine Mitwirkenden bleiben Eigentum der jeweiligen Projekte.
about-built-with = Erstellt mit
about-built-with-detail = Die Lizenzen der einzelnen Crates sind in deren jeweiligen Repositories aufgeführt.

# -- progress overlay --
progress-last = Zuletzt: { $line }
progress-running = Läuft ({ $count }):
progress-counts = { $built }/{ $started } gebaut, { $dl_done }/{ $dl_started } heruntergeladen ({ $mb } MB)
progress-build-line = { "  - " }{ $name } [building]
progress-build-item = { "  - " }{ $status ->
    [done] { $name } [done]
    [failed] { $name } [FAILED]
   *[other] { $name } [building]
}
progress-finished = Abgeschlossen ({ $done } fertig, { $failed } fehlgeschlagen):
progress-and-more = ... und { $count } weitere
progress-active-downloads = Aktive Downloads ({ $count }):
progress-completed-downloads = Abgeschlossene Downloads ({ $count }):
progress-starting = Starte...
progress-failed = Fehlgeschlagen.
progress-stage-ellipsis = { $stage }...

# -- system / settings --
system-title = Einstellungen
system-current-config = Aktuelle Systemkonfiguration: { $path }
system-not-guix = Kein Guix-System: { $error }
system-checking-config = Prüfe aktuelle Systemkonfiguration...
system-no-config-banner = Keine Systemkonfigurationsdatei unter /etc/config.scm oder /etc/system.scm gefunden. Gib unten den Pfad zu deiner .scm-Konfiguration ein.
system-validate = Prüfen
system-config-heading = Systemkonfiguration
system-config-blurb = Pfad zu deiner editierbaren .scm-Systemkonfiguration.
system-config-placeholder = /home/you/dotfiles/config.scm
system-validation-empty = Pfad ist leer.
system-validation-missing = Pfad existiert nicht: { $path }
system-validation-not-file = Pfad ist keine reguläre Datei: { $path }
system-validation-ok = OK: { $path }
system-load-paths-heading = Zusätzliche Load-Pfade
system-load-paths-blurb = Weitere Verzeichnisse, die beim Auflösen von Scheme-Imports durchsucht werden.
system-load-paths-auto = Auto: { $path }
system-load-paths-auto-unset = Auto: (Systemkonfiguration oben setzen)
system-load-paths-placeholder = /pfad/zu/extra/modulen
system-add = + Hinzufügen
system-section-system = SYSTEM
system-channels-heading = Kanäle
system-channels-blurb = Verwalte Kanäle auf Benutzerebene im eigenen Tab.
system-channels-none = Keine Kanäle konfiguriert.
system-channels-configured = { $count ->
    [one] { $count } Kanal konfiguriert.
   *[other] { $count } Kanäle konfiguriert.
}
system-channels-unknown = Konfigurierte Kanäle: —
system-open-channels = Kanäle-Tab öffnen
system-channels-source-heading = Quellpfad für Benutzerkanäle
system-channels-source-blurb = Überschreibt ~/.config/guix/channels.scm. Erforderlich, wenn der Standardpfad von `guix home` verwaltet wird (verweist nach /gnu/store).
system-channels-source-placeholder = /home/you/dotfiles/channels.scm (leer lassen für Standard)
system-use-default = Standard verwenden
system-section-user-channels = BENUTZERKANÄLE
system-metadata-heading = Icons & Screenshots
system-metadata-blurb = Hole Icons und Screenshots aus Drittanbieter-Katalogen für ausgewählte Suchergebnisse. Opt-in; erfordert Netzwerkzugriff.
system-metadata-enable = Metadaten von Drittanbietern aktivieren
system-metadata-flathub = Flathub (flathub.org)
system-metadata-debian = screenshots.debian.net
system-cache-heading = Cache
system-cache-blurb = Icons und Screenshots werden bis zu einem Jahr auf der Festplatte gespeichert. Leere ihn, wenn ein Icon beim Anbieter falsch aussieht.
system-cache-dir = Cache-Verzeichnis: { $path }
system-cache-dir-none = Cache-Verzeichnis: (kein XDG-Cache-Verzeichnis gefunden — nur im Arbeitsspeicher)
system-clear-cache = Cache leeren
system-clearing-cache = Leere Cache...
system-cache-cleared = Cache geleert.
system-cache-clear-failed = Cache konnte nicht geleert werden: { $error }
system-discovery-heading = Discovery
system-discovery-toggle = Kanäle und Pakete von toys.whereis.social durchstöbern
system-discovery-blurb = Opt-in. Erfordert Netzwerkzugriff. Wenn aus, erscheint Discovery nirgendwo in der App.
system-desktop-refresh = Anwendungsmenü nach der Installation von Apps aktualisieren
system-desktop-refresh-desc = Baut das Desktop-Menü neu auf, damit neu installierte Anwendungen sofort erscheinen (KDE, XFCE, MATE, LXQt). Ausschalten, wenn Sie lieber manuell aktualisieren.
system-section-metadata = METADATEN

# -- channels --
channels-title = Kanäle
channels-intro = Kanäle sind Paketquellen für Guix. Einen Kanal hinzuzufügen erlaubt dir, die darin angebotene Software zu installieren. Einen zu entfernen bedeutet, dass seine Pakete keine Updates mehr bekommen.
channels-section-user = BENUTZERKANÄLE
channels-submode-installed = Installiert
channels-submode-discover = Entdecken
channels-default-path = ~/.config/guix/channels.scm (Standard)
channels-store-managed = store-verwaltet (schreibgeschützt)
channels-writable = beschreibbar
channels-file = Datei: { $path }
channels-confirm-restore = Wiederherstellung bestätigen
channels-restore-last = Letztes Backup wiederherstellen
channels-cant-edit-title = Diese Datei kann hier nicht bearbeitet werden
channels-cant-edit-blurb = Deine channels.scm wird von `guix home` (oder einem anderen Tool) verwaltet und kann nicht direkt bearbeitet werden. Um Kanaländerungen mit guix-gui vorzunehmen, lege unter { $settings_tab } → { $channels_tab } eine beschreibbare Datei fest.
channels-saving = Speichere...
channels-pull-then-install = Pull, dann { $pkg } installieren
channels-pull-only = Nur Pull
channels-pull-now = Jetzt pullen
channels-keep-changes = Änderungen behalten
channels-pull-failed-shadow = Pull fehlgeschlagen — Channel-Shadow-Bug (#74396).
channels-pull-failed = Pull fehlgeschlagen.
channels-rollback-blurb = Deine channels.scm enthält die neuen Änderungen, aber Guix konnte sie nicht holen. Vorherige channels.scm wiederherstellen?
channels-rollback-none = Keine vorherige channels.scm zum Wiederherstellen.
channels-restore-previous = Vorherige wiederherstellen
channels-empty-title = Keine channels.scm gefunden
channels-empty-blurb = Füge unten einen Kanal hinzu, um eine zu erstellen. Die Datei liegt standardmäßig unter ~/.config/guix/channels.scm.
channels-error = Fehler
channels-loading = Lade channels.scm...
channels-count = { $count ->
    [one] { $count } Kanal
   *[other] { $count } Kanäle
}
channels-none-in-file = Keine Kanäle in dieser Datei.
channels-inherited-title = Außerdem über deine Kanäle eingebunden
channels-inherited-blurb = Diese stammen aus den Kanälen oben und werden von ihnen verwaltet.
channels-branch = Branch: { $branch }
channels-commit = Commit: { $commit }
channels-introduction = Introduction: { $fpr }
channels-no-fingerprint = (kein Fingerprint)
channels-introduction-none = Introduction: (keine)
channels-remove-title = Kanal `{ $name }` entfernen?
channels-remove-intro = { $count ->
    [one] { $count } installiertes Paket stammt aus diesem Kanal:
   *[other] { $count } installierte Pakete stammen aus diesem Kanal:
}
channels-remove-explainer = Diese funktionieren weiter, erhalten aber keine Updates mehr, nachdem der Kanal entfernt wurde.
channels-remove-cmd-label = Um sie zusammen mit dem Kanal zu deinstallieren:
channels-remove-anyway = Kanal trotzdem entfernen
channels-locked = gesperrt
channels-see-warning = siehe Warnung unten
channels-confirm-remove = Entfernen bestätigen
channels-add-heading = Kanal hinzufügen
channels-add-blurb = Alle Felder werden wortwörtlich gespeichert; Introduction-Commit + Fingerprint sind erforderlich.
channels-add-name = Name
channels-add-name-placeholder = z. B. nonguix
channels-add-url = URL
channels-add-url-placeholder = https://gitlab.com/nonguix/nonguix
channels-add-branch = Branch
channels-add-branch-placeholder = master (optional)
channels-add-commit = Commit
channels-add-commit-placeholder = Commit-Hash (optional)
channels-add-intro-commit = Introduction-Commit
channels-add-intro-commit-placeholder = Introduction-Commit-Hash
channels-add-intro-fpr = Introduction-Fingerprint
channels-add-intro-fpr-placeholder = OpenPGP-Fingerprint (z. B. 2A39 3FFF 68F4 EF7A 3D29 ...)
channels-add-btn = Kanal hinzufügen
channels-discover-placeholder = Pakete oder Kanäle suchen...
channels-searching = Suche...
channels-package-results = { $count ->
    [one] { $count } Paket-Ergebnis
   *[other] { $count } Paket-Ergebnisse
}
channels-from = aus { $channel }
channels-packages = Pakete
channels-channels-heading = Kanäle
channels-no-synopsis = (keine Beschreibung)
channels-install = Installieren
channels-add-and-install = Kanal hinzufügen & installieren
channels-loading-discover = Lade Kanäle...
channels-no-introduced = Es wurden keine eingeführten Kanäle zurückgegeben.
channels-available = { $count ->
    [one] { $count } Kanal verfügbar
   *[other] { $count } Kanäle verfügbar
}
channels-already-added = bereits hinzugefügt
channels-add = Hinzufügen
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
channels-set-writable-tooltip = Lege eine beschreibbare Datei in den Einstellungen fest
channels-confirm-add-title = Kanal-Hinzufügen bestätigen
channels-confirm-add-blurb = Dadurch wird der Kanal an deine channels.scm angehängt und die Datei vor dem Speichern validiert.
channels-provenance = Herkunft
channels-supplied-by = Bereitgestellt von { $source }
channels-trust-warning = Einmal hinzugefügt, führt jedes `guix pull` Guile-Code aus dieser Quelle unter deiner Kennung aus. Überprüfe den Introduction-Commit und Fingerprint unten anhand der vom Kanal selbst veröffentlichten Werte, bevor du ihn hinzufügst.
channels-field-name = Name
channels-field-url = URL
channels-field-branch = Branch
channels-field-commit = Commit
channels-field-intro-commit = Intro-Commit
channels-field-intro-fpr = Intro-Fingerprint

# -- channels status messages --
channels-restored = Aus Backup wiederhergestellt.
channels-added-install-prompt = Kanal hinzugefügt. Pull, dann { $pkg } installieren?
channels-updated = Kanäle aktualisiert. Jetzt pullen, um den neuen Katalog zu holen.
channels-no-file-loaded = Keine channels.scm geladen; aktualisiere zuerst den Tab.
channels-store-managed-error = channels.scm unter { $path } ist store-verwaltet. Lege in den Einstellungen einen beschreibbaren Quellpfad-Override fest.
channels-no-backup = Keine Backup-Datei vorhanden.
channels-form-name-required = Name ist erforderlich.
channels-form-url-required = URL ist erforderlich.
channels-form-intro-required = Introduction-Commit und Fingerprint sind erforderlich.
channels-vanished-after-write = channels.scm ist nach dem Schreiben verschwunden — aktualisiere den Tab.
