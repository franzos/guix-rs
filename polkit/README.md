# Polkit policies — `org.libguix.system-{reconfigure,pull}`

This directory ships two polkit actions that let `libguix` invoke
privileged Guix operations via `pkexec` with an interactive admin prompt
instead of asking the user to drop into a root shell.

## What's here

- `org.libguix.system-reconfigure.policy` — action
  `org.libguix.system-reconfigure`. Permits
  `/run/current-system/profile/bin/guix system reconfigure …` (argv
  constrained to `argv1=system argv2=reconfigure`).
  `allow_active=auth_admin_keep` so successive invocations within
  polkit's grace window don't re-prompt.
- `org.libguix.system-pull.policy` — action
  `org.libguix.system-pull`. Permits
  `/run/current-system/profile/bin/guix pull …` (argv constrained to
  `argv1=pull`). Same `auth_admin_keep` semantics. This updates the
  **root** catalog at
  `/var/guix/profiles/per-user/root/current-guix`, which is what
  `guix system reconfigure` resolves packages against. `reconfigure`
  itself does not auto-pull — see the lib's `SystemOps::pull` docs.

Both actions target the *system* guix at
`/run/current-system/profile/bin/guix`, not the per-user
`~/.config/guix/current/bin/guix` — pkexec refuses to exec binaries
outside trusted paths.

The argv constraints scope each action narrowly: without them polkit
would match either action for *any* invocation of the trusted guix
binary. With them, `system reconfigure` and `pull` get separate grant
decisions.

## Install — Guix System (recommended)

`/etc/polkit-1/` on Guix System is declared in the system config, not
mutable at runtime. Ship both policies as a tiny Guix package and extend
`polkit-service-type` with it. A reference package definition lives in
the [panther channel](https://codeberg.org/gofranz/panther) at
`px/packages/libguix.scm` as `libguix-polkit`; lift it into any channel.

In your `operating-system`'s service list:

```scheme
(use-modules (gnu services dbus)               ;; polkit-service-type
             (px packages libguix))            ;; libguix-polkit

(services
  (cons (simple-service 'libguix-polkit
                        polkit-service-type
                        (list libguix-polkit))
        %desktop-services))
```

Then `sudo guix system reconfigure ...`. Verify after:

```sh
pkaction --action-id org.libguix.system-reconfigure --verbose
pkaction --action-id org.libguix.system-pull        --verbose
```

## Install — foreign distro (Guix on Debian/Fedora/etc.)

Polkit on a non-Guix host reads action files from
`/etc/polkit-1/actions/`. As root:

```sh
sudo cp polkit/org.libguix.system-reconfigure.policy /etc/polkit-1/actions/
sudo cp polkit/org.libguix.system-pull.policy        /etc/polkit-1/actions/
sudo chmod 644 /etc/polkit-1/actions/org.libguix.system-*.policy
sudo chown root:root /etc/polkit-1/actions/org.libguix.system-*.policy
```

Polkit picks new actions up without a restart. Verify with the same
`pkaction` calls as above.

## Authentication agent

`pkexec` only works if a polkit **authentication agent** is running in
the user's session — the agent is what actually shows the password
prompt. Without one, calls to `pkexec` from a GUI hang or fail silently.

Common agents (any one is fine, pick what matches your desktop):

- `lxqt-policykit-agent` (LXQt — lightweight, works anywhere)
- `polkit-gnome-authentication-agent-1` (GNOME)
- `polkit-kde-authentication-agent-1` (KDE Plasma)
- `mate-polkit`, `xfce-polkit`, `hyprpolkitagent` (matching desktops)

The agent should be started as part of your session — typically via
your window manager / desktop autostart, or your user shepherd
services. If `libguix` detects a system reconfigure or system pull
was requested but no agent is reachable, it surfaces a clear error
pointing here.

## Uninstall

- Guix System: drop the `simple-service` form, reconfigure.
- Foreign distro: `sudo rm /etc/polkit-1/actions/org.libguix.system-*.policy`.

## Why custom actions

Upstream Guix doesn't ship a polkit policy — `guix-daemon` runs as root
already, but `guix system reconfigure` and `guix pull` are normal user
commands that happen to need root to mutate system state. Without our
own actions, `pkexec guix …` falls back to the default
`org.freedesktop.policykit.exec` action, which works but doesn't let us
tune `auth_admin_keep`, write a meaningful prompt, or constrain the
action to a specific subcommand via argv annotations.

## Caveats

- The action path `/run/current-system/profile/bin/guix` is the version
  of `guix` baked into the running system. After `guix pull`, the
  user's `~/.config/guix/current/bin/guix` is newer. For reconfigure
  that's *fine* — the system guix is what should be applying system
  changes — but it does mean reconfigure may lag behind features
  available in the user's pulled guix. Documented behaviour, not a bug.
- pkexec strips most env vars. `libguix` will need to pass
  `--load-path` etc. explicitly if the user's config depends on extra
  channels.
- The argv constraints in both actions are matched verbatim by polkit.
  If a future libguix invocation inserts a flag *between* the guix
  binary and the subcommand (e.g.
  `guix --some-flag system reconfigure …`) the action will stop
  matching and fall back to the generic `policykit.exec`. We don't do
  that today.
