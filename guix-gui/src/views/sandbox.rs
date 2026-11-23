//! Install state and permission summary of panther's `X-sandboxed` variants.

use iced::widget::{column, text, Column};
use iced::Element;
use libguix::{InstalledPackage, SandboxInfo, SandboxPolicy};

use crate::styles::{BOLD, MONO, MUTED};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState<'a> {
    NotInstalled,
    Plain(&'a InstalledPackage),
    Sandboxed(&'a InstalledPackage),
}

/// The sandboxed entry wins when both modes are installed.
pub fn install_state<'a>(installed: &'a [InstalledPackage], base: &str) -> InstallState<'a> {
    if let Some(p) = installed
        .iter()
        .find(|p| p.sandbox.as_ref().is_some_and(|s| s.base == base))
    {
        return InstallState::Sandboxed(p);
    }
    match installed.iter().find(|p| p.name == base) {
        Some(p) => InstallState::Plain(p),
        None => InstallState::NotInstalled,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SessionType {
    Wayland,
    #[default]
    Other,
}

impl SessionType {
    /// From the value of `XDG_SESSION_TYPE`.
    pub fn from_env_value(value: Option<&str>) -> Self {
        if value == Some("wayland") {
            Self::Wayland
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Wayland,
    X11,
}

/// The display the sandbox launcher hands to the program in this session.
pub fn display_line(policy: &SandboxPolicy, session: SessionType) -> Option<Display> {
    let wayland = policy.wayland.then_some(Display::Wayland);
    let x11 = policy.x11.then_some(Display::X11);
    match session {
        SessionType::Wayland => wayland.or(x11),
        SessionType::Other => x11.or(wayland),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DbusLabel {
    Notifications,
    Keyring,
    Tray,
    Settings,
    Screencast,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DbusRule {
    Label(DbusLabel),
    /// Portal plumbing, implied by any of these labels.
    Plumbing(&'static [DbusLabel]),
}

const REQUEST_PORTALS: &[DbusLabel] = &[DbusLabel::Screencast, DbusLabel::Open];
const SESSION_PORTALS: &[DbusLabel] = &[DbusLabel::Screencast];

/// Rules from `px/sandbox.scm` and the panther variants.
const DBUS_RULES: &[(&str, DbusRule)] = &[
    ("--talk=org.freedesktop.Notifications", DbusRule::Label(DbusLabel::Notifications)),
    ("--talk=org.freedesktop.secrets", DbusRule::Label(DbusLabel::Keyring)),
    ("--talk=org.kde.StatusNotifierWatcher", DbusRule::Label(DbusLabel::Tray)),
    ("--own=org.freedesktop.StatusNotifierItem-2-1", DbusRule::Label(DbusLabel::Tray)),
    ("--own=org.kde.StatusNotifierItem-2-1", DbusRule::Label(DbusLabel::Tray)),
    (
        "--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.Settings.*@/org/freedesktop/portal/desktop",
        DbusRule::Label(DbusLabel::Settings),
    ),
    (
        "--broadcast=org.freedesktop.portal.Desktop=org.freedesktop.portal.Settings.*@/org/freedesktop/portal/desktop",
        DbusRule::Label(DbusLabel::Settings),
    ),
    (
        "--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.ScreenCast.*@/org/freedesktop/portal/desktop",
        DbusRule::Label(DbusLabel::Screencast),
    ),
    (
        "--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.OpenURI.OpenFile@/org/freedesktop/portal/desktop",
        DbusRule::Label(DbusLabel::Open),
    ),
    (
        "--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.OpenURI.OpenDirectory@/org/freedesktop/portal/desktop",
        DbusRule::Label(DbusLabel::Open),
    ),
    (
        "--call=org.freedesktop.portal.Desktop=org.freedesktop.DBus.Properties.*@/org/freedesktop/portal/desktop",
        DbusRule::Plumbing(&[DbusLabel::Settings, DbusLabel::Screencast, DbusLabel::Open]),
    ),
    (
        "--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.Request.*@/org/freedesktop/portal/desktop/request/*",
        DbusRule::Plumbing(REQUEST_PORTALS),
    ),
    (
        "--broadcast=org.freedesktop.portal.Desktop=org.freedesktop.portal.Request.*@/org/freedesktop/portal/desktop/request/*",
        DbusRule::Plumbing(REQUEST_PORTALS),
    ),
    (
        "--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.Session.*@/org/freedesktop/portal/desktop/session/*",
        DbusRule::Plumbing(SESSION_PORTALS),
    ),
    (
        "--broadcast=org.freedesktop.portal.Desktop=org.freedesktop.portal.Session.*@/org/freedesktop/portal/desktop/session/*",
        DbusRule::Plumbing(SESSION_PORTALS),
    ),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DbusSummary<'a> {
    /// Sorted, each label once.
    pub labels: Vec<DbusLabel>,
    /// Rules shown verbatim, in policy order.
    pub raw: Vec<&'a str>,
}

pub fn classify_dbus(rules: &[String]) -> DbusSummary<'_> {
    let lookup = |rule: &str| DBUS_RULES.iter().find(|(r, _)| *r == rule).map(|e| e.1);
    let mut labels: Vec<DbusLabel> = rules
        .iter()
        .filter_map(|rule| match lookup(rule) {
            Some(DbusRule::Label(label)) => Some(label),
            _ => None,
        })
        .collect();
    labels.sort_unstable();
    labels.dedup();
    let raw = rules
        .iter()
        .filter(|rule| match lookup(rule) {
            Some(DbusRule::Label(_)) => false,
            Some(DbusRule::Plumbing(portals)) => !portals.iter().any(|p| labels.contains(p)),
            None => true,
        })
        .map(String::as_str)
        .collect();
    DbusSummary { labels, raw }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermKind {
    Network,
    Audio,
    Camera,
    Gpu,
    Display(Display),
    UserNamespaces,
    FileArguments,
    ReadFolders,
    WriteFolders,
    Dbus(DbusLabel),
    DbusOther,
    UnknownFields,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermLine {
    pub kind: PermKind,
    /// Paths, raw D-Bus rules, or `key value` pairs, verbatim.
    pub details: Vec<String>,
}

impl PermLine {
    fn bare(kind: PermKind) -> Self {
        Self {
            kind,
            details: Vec::new(),
        }
    }
}

/// Granted items only, in display order.
pub fn permission_lines(policy: &SandboxPolicy, session: SessionType) -> Vec<PermLine> {
    let mut lines = Vec::new();
    let flags = [
        (policy.network, PermKind::Network),
        (policy.audio, PermKind::Audio),
        (policy.camera, PermKind::Camera),
        (policy.gpu, PermKind::Gpu),
    ];
    lines.extend(flags.iter().filter(|f| f.0).map(|f| PermLine::bare(f.1)));
    if let Some(d) = display_line(policy, session) {
        lines.push(PermLine::bare(PermKind::Display(d)));
    }
    if policy.user_namespaces {
        lines.push(PermLine::bare(PermKind::UserNamespaces));
    }
    if policy.file_arguments {
        lines.push(PermLine::bare(PermKind::FileArguments));
    }
    for (paths, kind) in [
        (&policy.read_only_paths, PermKind::ReadFolders),
        (&policy.writable_paths, PermKind::WriteFolders),
    ] {
        if !paths.is_empty() {
            lines.push(PermLine {
                kind,
                details: paths.clone(),
            });
        }
    }
    let dbus = classify_dbus(&policy.dbus_rules);
    lines.extend(
        dbus.labels
            .iter()
            .map(|l| PermLine::bare(PermKind::Dbus(*l))),
    );
    if !dbus.raw.is_empty() {
        lines.push(PermLine {
            kind: PermKind::DbusOther,
            details: dbus.raw.iter().map(|r| (*r).to_owned()).collect(),
        });
    }
    if !policy.unknown.is_empty() {
        lines.push(PermLine {
            kind: PermKind::UnknownFields,
            details: policy
                .unknown
                .iter()
                .map(|(k, v)| format!("{k} {v}").trim_end().to_owned())
                .collect(),
        });
    }
    if lines.is_empty() {
        lines.push(PermLine::bare(PermKind::Nothing));
    }
    lines
}

pub fn permissions_block<'a, M: 'a>(info: &SandboxInfo, session: SessionType) -> Element<'a, M> {
    let mut col = Column::new().spacing(6).push(
        text(crate::t!("sandbox-permissions-title"))
            .font(BOLD)
            .size(13),
    );
    for line in permission_lines(&info.policy, session) {
        let mut entry = column![text(perm_label(line.kind)).size(12)].spacing(2);
        let is_path = matches!(line.kind, PermKind::ReadFolders | PermKind::WriteFolders);
        for d in line.details {
            let shown = if is_path && !d.starts_with('/') && !d.starts_with("$XDG_RUNTIME_DIR/") {
                format!("~/{d}")
            } else {
                d
            };
            entry = entry.push(text(shown).font(MONO).size(11).color(MUTED));
        }
        col = col.push(entry);
    }
    col.into()
}

fn perm_label(kind: PermKind) -> String {
    match kind {
        PermKind::Network => crate::t!("sandbox-perm-network"),
        PermKind::Audio => crate::t!("sandbox-perm-audio"),
        PermKind::Camera => crate::t!("sandbox-perm-camera"),
        PermKind::Gpu => crate::t!("sandbox-perm-gpu"),
        PermKind::Display(Display::Wayland) => crate::t!("sandbox-perm-display-wayland"),
        PermKind::Display(Display::X11) => crate::t!("sandbox-perm-display-x11"),
        PermKind::UserNamespaces => crate::t!("sandbox-perm-user-namespaces"),
        PermKind::FileArguments => crate::t!("sandbox-perm-file-args"),
        PermKind::ReadFolders => crate::t!("sandbox-perm-read-folders"),
        PermKind::WriteFolders => crate::t!("sandbox-perm-write-folders"),
        PermKind::Dbus(DbusLabel::Notifications) => crate::t!("sandbox-dbus-notifications"),
        PermKind::Dbus(DbusLabel::Keyring) => crate::t!("sandbox-dbus-keyring"),
        PermKind::Dbus(DbusLabel::Tray) => crate::t!("sandbox-dbus-tray"),
        PermKind::Dbus(DbusLabel::Settings) => crate::t!("sandbox-dbus-settings"),
        PermKind::Dbus(DbusLabel::Screencast) => crate::t!("sandbox-dbus-screencast"),
        PermKind::Dbus(DbusLabel::Open) => crate::t!("sandbox-dbus-open"),
        PermKind::DbusOther => crate::t!("sandbox-dbus-other"),
        PermKind::UnknownFields => crate::t!("sandbox-unknown-fields"),
        PermKind::Nothing => crate::t!("sandbox-perm-none"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const PORTAL: &str = "org.freedesktop.portal.Desktop=org.freedesktop.";
    const PATH: &str = "@/org/freedesktop/portal/desktop";

    // `%portal-settings-dbus-rules` in px/sandbox.scm.
    fn settings_rules() -> Vec<String> {
        vec![
            format!("--call={PORTAL}DBus.Properties.*{PATH}"),
            format!("--call={PORTAL}portal.Settings.*{PATH}"),
            format!("--broadcast={PORTAL}portal.Settings.*{PATH}"),
        ]
    }

    // `%portal-screencast-dbus-rules` in px/sandbox.scm.
    fn screencast_rules() -> Vec<String> {
        vec![
            format!("--call={PORTAL}DBus.Properties.*{PATH}"),
            format!("--call={PORTAL}portal.ScreenCast.*{PATH}"),
            format!("--call={PORTAL}portal.Request.*{PATH}/request/*"),
            format!("--call={PORTAL}portal.Session.*{PATH}/session/*"),
            format!("--broadcast={PORTAL}portal.Request.*{PATH}/request/*"),
            format!("--broadcast={PORTAL}portal.Session.*{PATH}/session/*"),
        ]
    }

    // `%portal-open-dbus-rules` in px/sandbox.scm.
    fn open_rules() -> Vec<String> {
        vec![
            format!("--call={PORTAL}DBus.Properties.*{PATH}"),
            format!("--call={PORTAL}portal.OpenURI.OpenFile{PATH}"),
            format!("--call={PORTAL}portal.OpenURI.OpenDirectory{PATH}"),
            format!("--call={PORTAL}portal.Request.*{PATH}/request/*"),
            format!("--broadcast={PORTAL}portal.Request.*{PATH}/request/*"),
        ]
    }

    // App-specific rules of transmission-sandboxed.
    fn transmission_rules() -> Vec<String> {
        strings(&[
            "--own=com.transmissionbt.Transmission",
            "--talk=org.freedesktop.Notifications",
        ])
    }

    // App-specific rules of anytype-sandboxed.
    fn anytype_rules() -> Vec<String> {
        strings(&[
            "--own=org.freedesktop.StatusNotifierItem-2-1",
            "--talk=org.freedesktop.Notifications",
            "--talk=org.kde.StatusNotifierWatcher",
            "--talk=org.freedesktop.secrets",
        ])
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn concat(lists: &[Vec<String>]) -> Vec<String> {
        lists.concat()
    }

    fn assert_accounted(rules: &[String]) {
        let summary = classify_dbus(rules);
        for rule in rules {
            let accounted = match DBUS_RULES.iter().find(|(r, _)| r == rule).map(|e| e.1) {
                Some(DbusRule::Label(l)) => summary.labels.contains(&l),
                Some(DbusRule::Plumbing(portals)) => {
                    portals.iter().any(|p| summary.labels.contains(p))
                        != summary.raw.contains(&rule.as_str())
                }
                None => summary.raw.contains(&rule.as_str()),
            };
            assert!(accounted, "unaccounted rule {rule}");
        }
    }

    #[test]
    fn plumbing_of_another_portal_is_raw() {
        let session: Vec<String> = screencast_rules()
            .into_iter()
            .filter(|r| r.contains("portal.Session") || r.contains("portal.Request"))
            .collect();
        let rules = concat(&[settings_rules(), session.clone()]);
        let s = classify_dbus(&rules);
        assert_eq!(s.labels, vec![DbusLabel::Settings]);
        let expected: Vec<&str> = session.iter().map(String::as_str).collect();
        assert_eq!(s.raw, expected);
        assert_accounted(&rules);
    }

    #[test]
    fn classify_settings_portal() {
        let rules = settings_rules();
        let s = classify_dbus(&rules);
        assert_eq!(s.labels, vec![DbusLabel::Settings]);
        assert!(s.raw.is_empty());
        assert_accounted(&rules);
    }

    #[test]
    fn classify_screencast_portal() {
        let rules = screencast_rules();
        let s = classify_dbus(&rules);
        assert_eq!(s.labels, vec![DbusLabel::Screencast]);
        assert!(s.raw.is_empty());
        assert_accounted(&rules);
    }

    #[test]
    fn classify_open_portal() {
        let rules = open_rules();
        let s = classify_dbus(&rules);
        assert_eq!(s.labels, vec![DbusLabel::Open]);
        assert!(s.raw.is_empty());
        assert_accounted(&rules);
    }

    #[test]
    fn classify_transmission() {
        let rules = concat(&[settings_rules(), transmission_rules()]);
        let s = classify_dbus(&rules);
        assert_eq!(
            s.labels,
            vec![DbusLabel::Notifications, DbusLabel::Settings]
        );
        assert_eq!(s.raw, vec!["--own=com.transmissionbt.Transmission"]);
        assert_accounted(&rules);
    }

    #[test]
    fn classify_anytype() {
        let rules = concat(&[settings_rules(), anytype_rules()]);
        let s = classify_dbus(&rules);
        assert_eq!(
            s.labels,
            vec![
                DbusLabel::Notifications,
                DbusLabel::Keyring,
                DbusLabel::Tray,
                DbusLabel::Settings
            ]
        );
        assert!(s.raw.is_empty());
        assert_accounted(&rules);
    }

    #[test]
    fn classify_all_portals_together() {
        let rules = concat(&[
            settings_rules(),
            screencast_rules(),
            open_rules(),
            transmission_rules(),
            anytype_rules(),
        ]);
        let s = classify_dbus(&rules);
        assert_eq!(s.raw, vec!["--own=com.transmissionbt.Transmission"]);
        assert_eq!(s.labels.len(), 6);
        assert_accounted(&rules);
    }

    #[test]
    fn plumbing_without_portal_label_is_raw() {
        let plumbing: Vec<String> = screencast_rules()
            .into_iter()
            .filter(|r| !r.contains("ScreenCast"))
            .collect();
        let rules = concat(&[plumbing.clone(), anytype_rules()]);
        let s = classify_dbus(&rules);
        assert_eq!(
            s.labels,
            vec![
                DbusLabel::Notifications,
                DbusLabel::Keyring,
                DbusLabel::Tray
            ]
        );
        let expected: Vec<&str> = plumbing.iter().map(String::as_str).collect();
        assert_eq!(s.raw, expected);
        assert_accounted(&rules);
    }

    #[test]
    fn raw_keeps_policy_order_without_portal() {
        let rules = strings(&[
            "--own=a.b",
            &format!("--call={PORTAL}DBus.Properties.*{PATH}"),
            "--own=c.d",
        ]);
        assert_eq!(classify_dbus(&rules).raw, rules);
    }

    fn pkg(name: &str, sandbox_base: Option<&str>) -> InstalledPackage {
        InstalledPackage {
            name: name.to_owned(),
            version: "1.0".to_owned(),
            output: "out".to_owned(),
            store_path: PathBuf::from("/gnu/store/x"),
            sandbox: sandbox_base.map(|b| SandboxInfo {
                base: b.to_owned(),
                output: "out".to_owned(),
                policy: SandboxPolicy::default(),
            }),
        }
    }

    #[test]
    fn install_state_not_installed() {
        let installed = vec![pkg("other", None)];
        assert_eq!(install_state(&installed, "foo"), InstallState::NotInstalled);
    }

    #[test]
    fn install_state_plain() {
        let installed = vec![pkg("foo", None)];
        assert_eq!(
            install_state(&installed, "foo"),
            InstallState::Plain(&installed[0])
        );
    }

    #[test]
    fn install_state_sandboxed() {
        let installed = vec![pkg("foo-sandboxed", Some("foo"))];
        assert_eq!(
            install_state(&installed, "foo"),
            InstallState::Sandboxed(&installed[0])
        );
    }

    #[test]
    fn install_state_both_resolves_to_sandboxed() {
        let installed = vec![pkg("foo", None), pkg("foo-sandboxed", Some("foo"))];
        assert_eq!(
            install_state(&installed, "foo"),
            InstallState::Sandboxed(&installed[1])
        );
    }

    #[test]
    fn install_state_name_without_sandbox_data_is_ordinary() {
        let installed = vec![pkg("foo-sandboxed", None)];
        assert_eq!(install_state(&installed, "foo"), InstallState::NotInstalled);
    }

    #[test]
    fn session_type_from_env() {
        assert_eq!(
            SessionType::from_env_value(Some("wayland")),
            SessionType::Wayland
        );
        assert_eq!(SessionType::from_env_value(Some("x11")), SessionType::Other);
        assert_eq!(SessionType::from_env_value(Some("")), SessionType::Other);
        assert_eq!(SessionType::from_env_value(None), SessionType::Other);
    }

    #[test]
    fn display_line_all_combinations() {
        use Display::{Wayland, X11};
        let cases = [
            (SessionType::Wayland, true, true, Some(Wayland)),
            (SessionType::Wayland, true, false, Some(Wayland)),
            (SessionType::Wayland, false, true, Some(X11)),
            (SessionType::Wayland, false, false, None),
            (SessionType::Other, true, true, Some(X11)),
            (SessionType::Other, true, false, Some(Wayland)),
            (SessionType::Other, false, true, Some(X11)),
            (SessionType::Other, false, false, None),
        ];
        for (session, wayland, x11, expected) in cases {
            let policy = SandboxPolicy {
                wayland,
                x11,
                ..SandboxPolicy::default()
            };
            assert_eq!(
                display_line(&policy, session),
                expected,
                "{session:?} wayland={wayland} x11={x11}"
            );
        }
    }

    #[test]
    fn permission_lines_empty_policy_is_nothing() {
        let lines = permission_lines(&SandboxPolicy::default(), SessionType::Wayland);
        assert_eq!(lines, vec![PermLine::bare(PermKind::Nothing)]);
    }

    #[test]
    fn permission_lines_unknown_only_is_not_nothing() {
        let policy = SandboxPolicy {
            unknown: vec![("printing?".to_owned(), "#t".to_owned())],
            ..SandboxPolicy::default()
        };
        let lines = permission_lines(&policy, SessionType::Other);
        assert_eq!(
            lines,
            vec![PermLine {
                kind: PermKind::UnknownFields,
                details: vec!["printing? #t".to_owned()],
            }]
        );
    }

    #[test]
    fn permission_lines_order() {
        let policy = SandboxPolicy {
            read_only_paths: strings(&[".cache/fontconfig"]),
            writable_paths: strings(&[".config/transmission", "Downloads"]),
            network: true,
            wayland: true,
            x11: true,
            gpu: true,
            audio: true,
            camera: true,
            user_namespaces: true,
            file_arguments: true,
            dbus_rules: concat(&[settings_rules(), transmission_rules()]),
            unknown: vec![("printing?".to_owned(), "#t".to_owned())],
        };
        let kinds: Vec<PermKind> = permission_lines(&policy, SessionType::Wayland)
            .into_iter()
            .map(|l| l.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                PermKind::Network,
                PermKind::Audio,
                PermKind::Camera,
                PermKind::Gpu,
                PermKind::Display(Display::Wayland),
                PermKind::UserNamespaces,
                PermKind::FileArguments,
                PermKind::ReadFolders,
                PermKind::WriteFolders,
                PermKind::Dbus(DbusLabel::Notifications),
                PermKind::Dbus(DbusLabel::Settings),
                PermKind::DbusOther,
                PermKind::UnknownFields,
            ]
        );
    }
}
