app-title = Guix Software

tab-home = Home
tab-search = Search
tab-installed = Installed
tab-updates = Updates
tab-channels = Channels
tab-system = Settings
tab-about = About

settings-language = Language
settings-language-system = System default

# -- shared --
# Button label (imperative verb).
common-refresh = Refresh
# Button label (imperative verb).
common-cancel = Cancel
common-close = Close
common-dismiss = Dismiss
# Button label (imperative verb).
common-remove = Remove
common-open-settings = Open Settings

# -- app shell / overlay --
app-discover-failed = Failed to discover guix
app-discovering = Discovering guix...
app-brand = Guix
app-lightbox-close = Close (Esc)
app-lightbox-no-image = no image / failed
app-running = Running...
app-copy = Copy
app-show-log = Show log
app-hide-log = Hide log
# Status label shown in the progress overlay.
app-done = Done.
app-failed-exit = Failed (exit { $code }).
app-ended-no-summary = Ended without exit summary.
app-op-title = { $label } (op #{ $id })
app-cancel-pkexec-tooltip = Cannot cancel privileged operations — the kernel doesn't allow signaling root-owned processes. Wait for it to complete.
app-bootstrap-help =
    Reconfigure failed: the running system Guix doesn't recognise a module your
    config imports. This usually means a channel updated after your last
    reconfigure and the new module isn't baked into the system Guix yet.

    Bootstrap once manually:

        sudo guix system reconfigure -L { $load } { $cfg }

    After that, this button will work for subsequent updates.
app-set-source-config-first = Set the source config path on the System tab first.
app-op-start-failed = Failed to start op: { $error }
app-restore-panicked = Restore task panicked: { $detail }
app-restore-failed = Restore failed: { $detail }
app-discovery-client-failed = Discovery client failed: { $detail }

# -- op kinds --
op-install = Installing
op-remove = Removing
op-upgrade = Upgrading user packages
op-pull = Fetching user catalog
op-system-pull = Fetching system catalog
op-reconfigure = Reconfiguring system

# -- stages --
stage-starting = Starting
stage-channel-update = Updating channels
stage-computing-deriv = Computing derivation
stage-downloading = Downloading
stage-building = Building
stage-profile = Updating profile
# Status label shown in the progress overlay.
stage-done = Done
# Status label shown in the progress overlay.
stage-failed = Failed
stage-build-failed = Build failed: { $name }
stage-build-failed-log = Build failed: { $name } (log: { $log })

# -- categories --
category-graphics = Graphics
category-audio-video = Audio & Video
category-office = Office
category-development = Development
category-engineering = Engineering
category-internet = Internet
category-game = Games

# -- home --
home-title = Home
home-subtitle = A starting point — well-known applications available in Guix. Open one to install, or use Search for the full catalogue.
home-installed-badge = installed

# -- search --
search-title = Search
search-placeholder = Search packages...
search-loading-catalog = Loading package catalog...
search-searching = Searching...
search-results = { $count ->
    [one] { $count } result
   *[other] { $count } results
}
search-truncated = Showing first { $shown } of ≥{ $total } { $total ->
    [one] match
   *[other] matches
}; refine your query.
search-error-label = Search error:
search-copy-details = Copy details
search-select-prompt = Select a package to see details.
search-homepage = homepage:
search-license = license: { $license }
search-outputs = outputs: { $outputs }
# Button label (imperative verb).
search-install = Install
# Button label (imperative verb).
search-remove = Remove
search-screenshots-flathub = Screenshots via Flathub ({ $id })
search-screenshots-debian = Screenshots via screenshots.debian.net
search-loading-media = Loading icons / screenshots...
search-failed = Search failed.

# -- installed --
installed-title = Installed
installed-count = { $count ->
    [one] { $count } installed package
   *[other] { $count } installed packages
}
installed-loading = Loading...
installed-error = Error: { $error }

# -- updates --
updates-title = Updates
updates-your-packages-blurb = Manage your user-level packages.
updates-your-packages = Your packages
updates-fetch-latest = Fetch latest catalog
updates-update-my-packages = Update my packages
updates-system-blurb = Apply your system configuration. Requires admin authentication.
updates-system = System
updates-source-config = Source config: { $path }
updates-source-config-unset = Source config: (not set — open Settings to choose)
updates-fetch-system = Fetch system catalog
updates-update-system = Update system
# DO NOT TRANSLATE — literal shell command shown as a tooltip
updates-update-system-tip = pkexec guix system reconfigure
updates-confirm-reconfigure = Confirm system reconfigure
updates-reconfigure-blurb = Running as root via pkexec. Verify the paths below — each will be loaded by Guile with root privileges.
updates-config = Config:
updates-load-paths-none = Load paths (-L): (none)
updates-load-paths = { $count ->
    [one] Load paths (-L), { $count } entry:
   *[other] Load paths (-L), { $count } entries:
}
updates-confirm-reconfigure-btn = Confirm reconfigure
updates-loading-channels = Loading channels...
updates-error-channels = Error loading channels: { $error }
updates-last-pulled = Last pulled: { $age }.
updates-last-pulled-never = Last pulled: never.
updates-channels-none = Channels: (none discovered).
updates-channels = Channels: { $list }.
updates-channel-no-commit = (no commit)
updates-last-pulled-root = Last pulled (root): { $age }.
updates-last-pulled-root-never = Last pulled (root): never.
updates-last-reconfigured = Last reconfigured: { $age }.
updates-last-reconfigured-never = Last reconfigured: never (not a Guix System host?).

# -- updates: privileged help card --
updates-privileged-help-heading = Administrator action needed
updates-privileged-help-no-agent = No polkit authentication agent was detected. A password prompt may not appear. If none shows up, start your desktop's polkit agent, or run the equivalent command below in a terminal.
updates-privileged-help-failed = This privileged operation couldn't complete: { $error }. Run the equivalent command below in a terminal instead.
updates-privileged-help-failure-generic = authentication or privileged step failed
updates-privileged-help-cmd-label = Run this manually in a terminal:
# DO NOT TRANSLATE: literal shell command
updates-privileged-help-cmd-pull = sudo guix pull
# DO NOT TRANSLATE: literal shell command
updates-privileged-help-cmd-reconfigure = sudo guix system reconfigure -L { $load } { $cfg }

# -- about --
about-title = About
about-version = Version { $version }
about-tagline = Desktop frontend for the Guix package manager.
about-authors = Authors
about-source = Source & contributions
about-source-blurb = Bug reports and pull requests are welcome.
about-license = License
about-license-line = Guix GUI is released under the GNU General Public License v3.0.
about-license-detail = You may redistribute and modify it under the terms of that licence. See the LICENSE file in the repository for the full text.
about-third-party = Third-party data
about-third-party-blurb = Application icons and screenshots are fetched from external services when you enable third-party metadata in Settings. Trademarks, icons, and screenshots remain the property of their respective projects.
about-channel-discovery = Channel discovery
about-channel-discovery-blurb = The Channels tab's Discover sub-mode browses Guix channels and packages indexed by toys.whereis.social. Opt-in; requires network. The catalog and its contributors remain the property of their respective projects.
about-built-with = Built with
about-built-with-detail = Licences of individual crates are listed in their respective repositories.

# -- progress overlay --
progress-last = Last: { $line }
progress-running = Running ({ $count }):
progress-counts = { $built }/{ $started } built, { $dl_done }/{ $dl_started } downloaded ({ $mb } MB)
progress-build-line = { "  - " }{ $name } [building]
progress-build-item = { "  - " }{ $status ->
    [done] { $name } [done]
    [failed] { $name } [FAILED]
   *[other] { $name } [building]
}
progress-finished = Finished ({ $done } done, { $failed } failed):
progress-and-more = ... and { $count } more
progress-active-downloads = Active downloads ({ $count }):
progress-completed-downloads = Completed downloads ({ $count }):
progress-starting = Starting...
progress-failed = Failed.
progress-stage-ellipsis = { $stage }...

# -- system / settings --
system-title = Settings
system-current-config = Current system config: { $path }
system-not-guix = Not on Guix System: { $error }
system-checking-config = Checking current system config...
system-no-config-banner = No system configuration file detected at /etc/config.scm or /etc/system.scm. Enter the path to your .scm configuration below.
# Button label (imperative verb).
system-validate = Validate
system-config-heading = System config
system-config-blurb = Path to your editable .scm system configuration.
system-config-placeholder = /home/you/dotfiles/config.scm
system-validation-empty = Path is empty.
system-validation-missing = Path does not exist: { $path }
system-validation-not-file = Path is not a regular file: { $path }
system-validation-ok = OK: { $path }
system-load-paths-heading = Extra load paths
system-load-paths-blurb = Additional directories to search when resolving Scheme imports.
system-load-paths-auto = Auto: { $path }
system-load-paths-auto-unset = Auto: (set system config above)
system-load-paths-placeholder = /path/to/extra/modules
# Button label (imperative verb).
system-add = + Add
system-section-system = SYSTEM
system-channels-heading = Channels
system-channels-blurb = Manage user-level channels in the dedicated tab.
system-channels-none = No channels configured.
system-channels-configured = { $count ->
    [one] { $count } channel configured.
   *[other] { $count } channels configured.
}
system-channels-unknown = Channels configured: —
system-open-channels = Open Channels tab
system-channels-source-heading = User channels source path
system-channels-source-blurb = Override for ~/.config/guix/channels.scm. Required when the default path is managed by `guix home` (resolves into /gnu/store).
system-channels-source-placeholder = /home/you/dotfiles/channels.scm (leave empty for default)
system-use-default = Use default
system-section-user-channels = USER CHANNELS
system-metadata-heading = Icons & screenshots
system-metadata-blurb = Fetch icons and screenshots from third-party catalogs for selected search results. Opt-in; requires network access.
system-metadata-enable = Enable third-party metadata
system-metadata-flathub = Flathub (flathub.org)
system-metadata-debian = screenshots.debian.net
system-cache-heading = Cache
system-cache-blurb = Icons and screenshots are saved on disk for up to a year. Clear it if an icon looks wrong upstream.
system-cache-dir = Cache directory: { $path }
system-cache-dir-none = Cache directory: (no XDG cache dir found — using in-memory only)
system-clear-cache = Clear cache
system-clearing-cache = Clearing cache...
system-cache-cleared = Cache cleared.
system-cache-clear-failed = Failed to clear cache: { $error }
system-discovery-heading = Discovery
system-discovery-toggle = Browse channels and packages from toys.whereis.social
system-discovery-blurb = Opt-in. Requires network access. When off, discovery does not appear anywhere in the app.
system-desktop-refresh = Refresh application menu after installing apps
system-desktop-refresh-desc = Rebuilds the desktop menu so newly installed applications appear right away (KDE, XFCE, MATE, LXQt). Turn off if you prefer to refresh manually.
system-section-metadata = METADATA

# -- channels --
channels-title = Channels
channels-intro = Channels are package sources for Guix. Adding a channel lets you install software it provides. Removing one means its packages stop getting updates.
channels-section-user = USER CHANNELS
channels-submode-installed = Installed
channels-submode-discover = Discover
channels-default-path = ~/.config/guix/channels.scm (default)
channels-store-managed = store-managed (read-only)
channels-writable = writable
channels-file = File: { $path }
channels-confirm-restore = Confirm restore
channels-restore-last = Restore last backup
channels-cant-edit-title = This file can't be edited here
channels-cant-edit-blurb = Your channels.scm is managed by `guix home` (or another tool) and can't be edited directly. To use guix-gui for channel changes, set a writable file in { $settings_tab } → { $channels_tab }.
channels-saving = Saving...
channels-pull-then-install = Pull, then install { $pkg }
channels-pull-only = Pull only
channels-pull-now = Pull now
channels-keep-changes = Keep changes
channels-pull-failed-shadow = Pull failed — channel shadow bug (#74396).
channels-pull-failed = Pull failed.
channels-rollback-blurb = Your channels.scm has the new changes but Guix couldn't fetch them. Restore the previous channels.scm?
channels-rollback-none = No previous channels.scm to restore.
channels-restore-previous = Restore previous
channels-empty-title = No channels.scm found
channels-empty-blurb = Add a channel below to create one. The file lives at ~/.config/guix/channels.scm by default.
channels-error = Error
channels-loading = Loading channels.scm...
channels-count = { $count ->
    [one] { $count } channel
   *[other] { $count } channels
}
channels-none-in-file = No channels in this file.
channels-inherited-title = Also pulled in by your channels
channels-inherited-blurb = These come from the channels above and are managed by them.
channels-branch = branch: { $branch }
channels-commit = commit: { $commit }
channels-introduction = introduction: { $fpr }
channels-no-fingerprint = (no fingerprint)
channels-introduction-none = introduction: (none)
channels-remove-title = Remove channel `{ $name }`?
channels-remove-intro = { $count ->
    [one] { $count } installed package comes from this channel:
   *[other] { $count } installed packages come from this channel:
}
channels-remove-explainer = These will keep working but won't receive updates after the channel is removed.
channels-remove-cmd-label = To uninstall them along with the channel:
channels-remove-anyway = Remove channel anyway
channels-locked = locked
channels-see-warning = see warning below
channels-confirm-remove = Confirm remove
channels-add-heading = Add a channel
channels-add-blurb = All fields are stored verbatim; introduction commit + fingerprint are required.
channels-add-name = Name
channels-add-name-placeholder = e.g. nonguix
channels-add-url = URL
channels-add-url-placeholder = https://gitlab.com/nonguix/nonguix
channels-add-branch = Branch
channels-add-branch-placeholder = master (optional)
channels-add-commit = Commit
channels-add-commit-placeholder = commit hash (optional)
channels-add-intro-commit = Introduction commit
channels-add-intro-commit-placeholder = introduction commit hash
channels-add-intro-fpr = Introduction fingerprint
channels-add-intro-fpr-placeholder = OpenPGP fingerprint (e.g. 2A39 3FFF 68F4 EF7A 3D29 ...)
channels-add-btn = Add channel
channels-discover-placeholder = Search packages or channels...
channels-searching = Searching...
channels-package-results = { $count ->
    [one] { $count } package result
   *[other] { $count } package results
}
channels-from = from { $channel }
channels-packages = Packages
channels-channels-heading = Channels
channels-no-synopsis = (no synopsis)
# Button label (imperative verb).
channels-install = Install
channels-add-and-install = Add channel & install
channels-loading-discover = Loading channels...
channels-no-introduced = No introduced channels were returned.
channels-available = { $count ->
    [one] { $count } channel available
   *[other] { $count } channels available
}
channels-already-added = already added
# Button label (imperative verb).
channels-add = Add
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
channels-set-writable-tooltip = Set a writable file in Settings
channels-confirm-add-title = Confirm channel add
channels-confirm-add-blurb = This will append the channel to your channels.scm and validate the file before saving.
channels-provenance = Provenance
channels-supplied-by = Supplied by { $source }
channels-trust-warning = Once added, every `guix pull` runs Guile code from this source as you. Verify the introduction commit and fingerprint below against the channel's own published values before adding.
channels-field-name = name
channels-field-url = url
channels-field-branch = branch
channels-field-commit = commit
channels-field-intro-commit = intro commit
channels-field-intro-fpr = intro fingerprint

# -- channels status messages --
channels-restored = Restored from backup.
channels-added-install-prompt = Channel added. Pull, then install { $pkg }?
channels-updated = Channels updated. Pull now to fetch the new catalog.
channels-no-file-loaded = No channels.scm loaded; refresh the tab first.
channels-store-managed-error = channels.scm at { $path } is store-managed. Set a writable source-path override in Settings.
channels-no-backup = No backup file present.
channels-form-name-required = Name is required.
channels-form-url-required = URL is required.
channels-form-intro-required = Introduction commit and fingerprint are required.
channels-vanished-after-write = channels.scm vanished after write — refresh the tab.
