//! Metadata of panther's `X-sandboxed` packages, from the `sandbox`
//! package property or `share/sandbox/<base>.scm` in the output.

use lexpr::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxInfo {
    /// Name of the wrapped package.
    pub base: String,
    /// Output of `base` whose programs are wrapped.
    pub output: String,
    pub policy: SandboxPolicy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub read_only_paths: Vec<String>,
    pub writable_paths: Vec<String>,
    pub network: bool,
    pub wayland: bool,
    pub x11: bool,
    pub gpu: bool,
    pub audio: bool,
    pub camera: bool,
    pub user_namespaces: bool,
    pub file_arguments: bool,
    /// xdg-dbus-proxy filter options, verbatim.
    pub dbus_rules: Vec<String>,
    /// Unrecognised keys and known keys with a wrongly shaped value, as
    /// `(key, printed value)`.
    pub unknown: Vec<(String, String)>,
}

/// A sandboxed package and what it wraps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxVariant {
    pub name: String,
    pub info: SandboxInfo,
}

impl SandboxInfo {
    pub fn from_value(value: &Value) -> Option<Self> {
        let mut base = None;
        let mut output = None;
        let mut policy = None;
        for entry in proper_list(value)? {
            let Some((key, val)) = entry.as_pair() else {
                continue;
            };
            match key.as_symbol() {
                Some("base") => base = val.as_str().map(str::to_owned),
                Some("output") => output = val.as_str().map(str::to_owned),
                Some("policy") => policy = SandboxPolicy::from_value(val),
                _ => {}
            }
        }
        Some(Self {
            base: base?,
            output: output?,
            policy: policy?,
        })
    }

    pub fn from_file_text(text: &str) -> Option<Self> {
        Self::from_value(&lexpr::from_str(text).ok()?)
    }
}

impl SandboxPolicy {
    fn from_value(value: &Value) -> Option<Self> {
        let mut p = Self::default();
        for entry in proper_list(value)? {
            let Some((key, val)) = entry.as_pair().filter(|(k, _)| k.is_symbol()) else {
                p.unknown.push((entry.to_string(), String::new()));
                continue;
            };
            let name = key.as_symbol().unwrap_or_default();
            let ok = match name {
                "read-only-paths" => set_strings(&mut p.read_only_paths, val),
                "writable-paths" => set_strings(&mut p.writable_paths, val),
                "dbus-rules" => set_strings(&mut p.dbus_rules, val),
                "network?" => set_bool(&mut p.network, val),
                "wayland?" => set_bool(&mut p.wayland, val),
                "x11?" => set_bool(&mut p.x11, val),
                "gpu?" => set_bool(&mut p.gpu, val),
                "audio?" => set_bool(&mut p.audio, val),
                "camera?" => set_bool(&mut p.camera, val),
                "user-namespaces?" => set_bool(&mut p.user_namespaces, val),
                "file-arguments?" => set_bool(&mut p.file_arguments, val),
                _ => false,
            };
            if !ok {
                p.unknown.push((name.to_owned(), val.to_string()));
            }
        }
        Some(p)
    }
}

fn proper_list(value: &Value) -> Option<impl Iterator<Item = &Value>> {
    if value.is_list() {
        value.list_iter()
    } else {
        None
    }
}

fn set_bool(field: &mut bool, value: &Value) -> bool {
    value.as_bool().map(|b| *field = b).is_some()
}

fn set_strings(field: &mut Vec<String>, value: &Value) -> bool {
    let Some(items) = proper_list(value) else {
        return false;
    };
    let strings: Option<Vec<String>> = items.map(|v| v.as_str().map(str::to_owned)).collect();
    strings.map(|s| *field = s).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    // `write` of the `sandbox` property of panther's transmission-sandboxed.
    const TRANSMISSION: &str = r#"((base . "transmission") (output . "gui") (policy (read-only-paths ".cache/fontconfig") (writable-paths ".config/transmission" "Downloads") (network? . #t) (wayland? . #t) (x11? . #t) (gpu? . #t) (audio? . #f) (camera? . #f) (user-namespaces? . #f) (file-arguments? . #t) (dbus-rules "--call=org.freedesktop.portal.Desktop=org.freedesktop.DBus.Properties.*@/org/freedesktop/portal/desktop" "--call=org.freedesktop.portal.Desktop=org.freedesktop.portal.Settings.*@/org/freedesktop/portal/desktop" "--broadcast=org.freedesktop.portal.Desktop=org.freedesktop.portal.Settings.*@/org/freedesktop/portal/desktop" "--own=com.transmissionbt.Transmission" "--talk=org.freedesktop.Notifications")))"#;

    #[test]
    fn parses_transmission_metadata() {
        let info = SandboxInfo::from_file_text(TRANSMISSION).expect("parses");
        assert_eq!(info.base, "transmission");
        assert_eq!(info.output, "gui");
        let p = &info.policy;
        assert_eq!(p.read_only_paths, vec![".cache/fontconfig"]);
        assert_eq!(p.writable_paths, vec![".config/transmission", "Downloads"]);
        assert!(p.network && p.wayland && p.x11 && p.gpu && p.file_arguments);
        assert!(!p.audio && !p.camera && !p.user_namespaces);
        assert_eq!(p.dbus_rules.len(), 5);
        assert_eq!(p.dbus_rules[3], "--own=com.transmissionbt.Transmission");
        assert!(p.unknown.is_empty());
    }

    #[test]
    fn accepts_explicit_dotted_forms() {
        let text = r#"((base . "mpv") (output . "out") (policy . ((read-only-paths . ()) (writable-paths . (".cache/mpv")) (dbus-rules . ()) (audio? . #t))))"#;
        let p = SandboxInfo::from_file_text(text).expect("parses").policy;
        assert!(p.read_only_paths.is_empty());
        assert_eq!(p.writable_paths, vec![".cache/mpv"]);
        assert!(p.dbus_rules.is_empty());
        assert!(p.audio);
        assert!(p.unknown.is_empty());
    }

    #[test]
    fn empty_list_written_without_dot() {
        let text = r#"((base . "x") (output . "out") (policy (read-only-paths) (dbus-rules)))"#;
        let p = SandboxInfo::from_file_text(text).expect("parses").policy;
        assert!(p.read_only_paths.is_empty());
        assert!(p.dbus_rules.is_empty());
        assert!(p.unknown.is_empty());
    }

    #[test]
    fn unknown_key_is_kept() {
        let text = r#"((base . "x") (output . "out") (policy (network? . #t) (printing? . #t)))"#;
        let p = SandboxInfo::from_file_text(text).expect("parses").policy;
        assert!(p.network);
        assert_eq!(p.unknown, vec![("printing?".to_owned(), "#t".to_owned())]);
    }

    #[test]
    fn bool_with_list_value_lands_in_unknown() {
        let text = r#"((base . "x") (output . "out") (policy (network? #t)))"#;
        let p = SandboxInfo::from_file_text(text).expect("parses").policy;
        assert!(!p.network);
        assert_eq!(p.unknown.len(), 1);
        assert_eq!(p.unknown[0].0, "network?");
    }

    #[test]
    fn missing_base_is_none() {
        let text = r#"((output . "out") (policy (network? . #t)))"#;
        assert_eq!(SandboxInfo::from_file_text(text), None);
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(SandboxInfo::from_file_text("not (an alist"), None);
        assert_eq!(SandboxInfo::from_file_text("42"), None);
        assert_eq!(SandboxInfo::from_file_text(""), None);
    }
}
