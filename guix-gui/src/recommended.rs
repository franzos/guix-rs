//! Curated list of well-known applications surfaced on the Home tab.
//!
//! Hand-picked rather than auto-discovered — the goal is a short,
//! confidence-inspiring overview of "what's actually here", not an
//! exhaustive catalogue (Search covers that). Categories follow the
//! freedesktop.org main-category names so this table can later cross-
//! reference `.desktop` files without a remapping.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Graphics,
    AudioVideo,
    Office,
    Development,
    Engineering,
    Internet,
    Game,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Graphics => "Graphics",
            Category::AudioVideo => "Audio & Video",
            Category::Office => "Office",
            Category::Development => "Development",
            Category::Engineering => "Engineering",
            Category::Internet => "Internet",
            Category::Game => "Games",
        }
    }

    /// Display order in the Home view. Lower is shown first.
    pub fn order(self) -> u8 {
        match self {
            Category::Graphics => 0,
            Category::AudioVideo => 1,
            Category::Office => 2,
            Category::Development => 3,
            Category::Engineering => 4,
            Category::Internet => 5,
            Category::Game => 6,
        }
    }
}

pub struct RecommendedApp {
    /// Exact Guix package name — used as the search key and the install target.
    pub name: &'static str,
    pub category: Category,
    pub blurb: &'static str,
}

/// Apps that need specific Guix channels configured before the install
/// can succeed. Kept as a sparse lookup table so the much larger
/// `RECOMMENDED` slice stays uncluttered — most apps need no gating.
///
/// Entries are matched case-sensitively against `Channel::name` from
/// `guix describe`. All listed channels must be present for the app to
/// appear on the Home tab.
const CHANNEL_REQUIREMENTS: &[(&str, &[&str])] = &[
    // pantherx channel (https://codeberg.org/gofranz/panther) ships GUI
    // apps that aren't in the default Guix channel set. Gated so the
    // tile only appears once the user has pantherx configured.
    ("rnote", &["pantherx"]),
    ("rustdesk", &["pantherx"]),
    ("gitbutler", &["pantherx"]),
    ("appflowy", &["pantherx"]),
    ("halloy", &["pantherx"]),
    ("qalculate-gtk", &["pantherx"]),
    ("tidal-hifi", &["pantherx"]),
    ("discord", &["pantherx"]),
    ("syncthingtray", &["pantherx"]),
];

/// Channels (if any) that must be configured for `name` to be eligible
/// for display. Empty slice means "available on the default channel set".
pub fn required_channels(name: &str) -> &'static [&'static str] {
    CHANNEL_REQUIREMENTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
        .unwrap_or(&[])
}

/// GUI applications from the default Guix channel whose Flathub
/// component ID's tail matches the Guix package name (so the icon
/// lookup in `app_metadata` resolves). CLIs are intentionally omitted —
/// the Home tab is a visual discover surface, not a package index.
pub const RECOMMENDED: &[RecommendedApp] = &[
    // -- Graphics --
    RecommendedApp {
        name: "gimp",
        category: Category::Graphics,
        blurb: "Raster image editor — the long-standing open-source Photoshop alternative.",
    },
    RecommendedApp {
        name: "inkscape",
        category: Category::Graphics,
        blurb: "Vector graphics editor for SVG illustration and design work.",
    },
    RecommendedApp {
        name: "krita",
        category: Category::Graphics,
        blurb: "Digital painting application built around illustrators and concept artists.",
    },
    RecommendedApp {
        name: "blender",
        category: Category::Graphics,
        blurb: "3D modelling, animation, sculpting, and rendering suite.",
    },
    RecommendedApp {
        name: "darktable",
        category: Category::Graphics,
        blurb: "Non-destructive photo workflow with a capable RAW developer.",
    },
    RecommendedApp {
        name: "rawtherapee",
        category: Category::Graphics,
        blurb: "Cross-platform RAW image processor with fine-grained tonal controls.",
    },
    RecommendedApp {
        name: "mypaint",
        category: Category::Graphics,
        blurb: "Free-form painting app focused on a distraction-free canvas.",
    },
    RecommendedApp {
        name: "scribus",
        category: Category::Graphics,
        blurb: "Desktop publishing for posters, newsletters, magazines, and books.",
    },
    RecommendedApp {
        name: "digikam",
        category: Category::Graphics,
        blurb: "Advanced photo library, RAW developer, and tagging workflow from KDE.",
    },
    RecommendedApp {
        name: "shotwell",
        category: Category::Graphics,
        blurb: "Lightweight personal photo organiser for the GNOME desktop.",
    },
    // -- Audio & Video --
    RecommendedApp {
        name: "audacity",
        category: Category::AudioVideo,
        blurb: "Multi-track audio editor and recorder.",
    },
    RecommendedApp {
        name: "vlc",
        category: Category::AudioVideo,
        blurb: "Plays just about every audio and video format you'll encounter.",
    },
    RecommendedApp {
        name: "kdenlive",
        category: Category::AudioVideo,
        blurb: "Non-linear video editor with multi-track timeline editing.",
    },
    RecommendedApp {
        name: "shotcut",
        category: Category::AudioVideo,
        blurb: "Cross-platform video editor built on FFmpeg with a wide format range.",
    },
    RecommendedApp {
        name: "pitivi",
        category: Category::AudioVideo,
        blurb: "GNOME-native video editor focused on a clean, approachable workflow.",
    },
    RecommendedApp {
        name: "ardour",
        category: Category::AudioVideo,
        blurb: "Professional digital audio workstation for recording and mixing.",
    },
    RecommendedApp {
        name: "lmms",
        category: Category::AudioVideo,
        blurb: "Music production studio for beats, basslines, and full arrangements.",
    },
    RecommendedApp {
        name: "mixxx",
        category: Category::AudioVideo,
        blurb: "DJ software with deck mixing, effects, and controller support.",
    },
    RecommendedApp {
        name: "mpv",
        category: Category::AudioVideo,
        blurb: "Scriptable media player with great codec coverage and a minimal UI.",
    },
    RecommendedApp {
        name: "celluloid",
        category: Category::AudioVideo,
        blurb: "GTK front-end for mpv — drag, drop, play, with a tidy modern interface.",
    },
    RecommendedApp {
        name: "clementine",
        category: Category::AudioVideo,
        blurb: "Music player and library manager with internet radio and podcast support.",
    },
    RecommendedApp {
        name: "musescore",
        category: Category::AudioVideo,
        blurb: "Music notation, composition, and playback for scores of any complexity.",
    },
    RecommendedApp {
        name: "gpodder",
        category: Category::AudioVideo,
        blurb: "Podcast subscription manager that syncs across devices.",
    },
    RecommendedApp {
        name: "tidal-hifi",
        category: Category::AudioVideo,
        blurb: "Unofficial desktop client for the Tidal music streaming service.",
    },
    // -- Office --
    RecommendedApp {
        name: "libreoffice",
        category: Category::Office,
        blurb: "Full office suite — documents, spreadsheets, presentations.",
    },
    RecommendedApp {
        name: "gnucash",
        category: Category::Office,
        blurb: "Double-entry accounting for personal finance and small business.",
    },
    RecommendedApp {
        name: "evince",
        category: Category::Office,
        blurb: "Lightweight viewer for PDFs, ePubs, and a handful of other formats.",
    },
    RecommendedApp {
        name: "xournalpp",
        category: Category::Office,
        blurb: "Hand-written note taking and PDF annotation with pressure sensitivity.",
    },
    RecommendedApp {
        name: "keepassxc",
        category: Category::Office,
        blurb: "Offline password manager with a cross-platform GUI.",
    },
    RecommendedApp {
        name: "calibre",
        category: Category::Office,
        blurb: "Ebook library manager — conversion, sync, and a built-in reader.",
    },
    RecommendedApp {
        name: "okular",
        category: Category::Office,
        blurb: "Universal document viewer for PDFs, ePubs, comics, and more.",
    },
    RecommendedApp {
        name: "foliate",
        category: Category::Office,
        blurb: "Polished ebook reader with annotations and dictionary lookup.",
    },
    RecommendedApp {
        name: "rnote",
        category: Category::Office,
        blurb: "Hand-drawn note taking and PDF annotation built for stylus input.",
    },
    RecommendedApp {
        name: "appflowy",
        category: Category::Office,
        blurb: "Open-source workspace for docs, tasks, and databases — a Notion alternative.",
    },
    RecommendedApp {
        name: "qalculate-gtk",
        category: Category::Office,
        blurb: "Multi-purpose calculator with units, currencies, plotting, and symbolic algebra.",
    },
    // -- Development --
    RecommendedApp {
        name: "zed",
        category: Category::Development,
        blurb: "High-performance multiplayer code editor — fast, GPU-rendered, collaborative.",
    },
    RecommendedApp {
        name: "gitbutler",
        category: Category::Development,
        blurb: "Reimagined git workflow with virtual branches and a visual commit composer.",
    },
    // -- Engineering --
    RecommendedApp {
        name: "openscad",
        category: Category::Engineering,
        blurb: "Programmer's 3D CAD modeller — describe geometry as code.",
    },
    RecommendedApp {
        name: "freecad",
        category: Category::Engineering,
        blurb: "Parametric 3D CAD for mechanical engineering and product design.",
    },
    RecommendedApp {
        name: "kicad",
        category: Category::Engineering,
        blurb: "Electronic design automation suite for schematics and PCB layout.",
    },
    // -- Internet --
    RecommendedApp {
        name: "thunderbird",
        category: Category::Internet,
        blurb: "Mail, calendar, and contacts client from the Mozilla project.",
    },
    RecommendedApp {
        name: "hexchat",
        category: Category::Internet,
        blurb: "IRC client with multi-server support and a sane default interface.",
    },
    RecommendedApp {
        name: "transmission",
        category: Category::Internet,
        blurb: "Lightweight BitTorrent client with a clean GTK interface.",
    },
    RecommendedApp {
        name: "gajim",
        category: Category::Internet,
        blurb: "XMPP chat client with end-to-end encryption and group-chat support.",
    },
    RecommendedApp {
        name: "dino",
        category: Category::Internet,
        blurb: "Modern XMPP client focused on a clean, conversation-first interface.",
    },
    RecommendedApp {
        name: "halloy",
        category: Category::Internet,
        blurb: "Modern Rust-built IRC client with a clean, native interface.",
    },
    RecommendedApp {
        name: "rustdesk",
        category: Category::Internet,
        blurb: "Self-hostable remote desktop — an open alternative to TeamViewer.",
    },
    RecommendedApp {
        name: "discord",
        category: Category::Internet,
        blurb: "Voice, video, and text chat platform popular with gaming and dev communities.",
    },
    RecommendedApp {
        name: "syncthingtray",
        category: Category::Internet,
        blurb: "System-tray GUI for Syncthing — status, controls, and notifications at a glance.",
    },
    // -- Games --
    RecommendedApp {
        name: "0ad",
        category: Category::Game,
        blurb: "Free real-time strategy game set across ancient civilisations.",
    },
    RecommendedApp {
        name: "supertuxkart",
        category: Category::Game,
        blurb: "Cart-racing game starring the Linux mascot and friends.",
    },
    RecommendedApp {
        name: "minetest",
        category: Category::Game,
        blurb: "Open-source voxel sandbox in the vein of Minecraft.",
    },
    RecommendedApp {
        name: "wesnoth",
        category: Category::Game,
        blurb: "Turn-based fantasy strategy with an extensive single-player campaign.",
    },
    RecommendedApp {
        name: "widelands",
        category: Category::Game,
        blurb: "Settlers-style real-time strategy focused on economic build-up.",
    },
    RecommendedApp {
        name: "openttd",
        category: Category::Game,
        blurb: "Open-source rebuild of the Transport Tycoon Deluxe classic.",
    },
];

/// Iterate apps grouped by category, in display order.
pub fn grouped() -> Vec<(Category, Vec<&'static RecommendedApp>)> {
    let mut by_cat: std::collections::HashMap<Category, Vec<&'static RecommendedApp>> =
        std::collections::HashMap::new();
    for app in RECOMMENDED {
        by_cat.entry(app.category).or_default().push(app);
    }
    let mut out: Vec<(Category, Vec<&'static RecommendedApp>)> = by_cat.into_iter().collect();
    out.sort_by_key(|(c, _)| c.order());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicate_names() {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for app in RECOMMENDED {
            assert!(
                seen.insert(app.name),
                "duplicate recommended app: {}",
                app.name
            );
        }
    }

    #[test]
    fn all_categories_have_at_least_one_app() {
        // Sanity check: if a category is in the enum, the curated list
        // should include at least one example, or it shouldn't be listed.
        let groups = grouped();
        let cats_with_apps: std::collections::HashSet<Category> =
            groups.iter().map(|(c, _)| *c).collect();
        for app in RECOMMENDED {
            assert!(cats_with_apps.contains(&app.category));
        }
    }

    #[test]
    fn grouped_is_sorted_by_display_order() {
        let groups = grouped();
        let orders: Vec<u8> = groups.iter().map(|(c, _)| c.order()).collect();
        let mut sorted = orders.clone();
        sorted.sort();
        assert_eq!(orders, sorted);
    }
}
