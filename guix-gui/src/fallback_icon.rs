//! Colored letter-tile placeholder shown when an app has no real icon.

use iced::widget::{container, text};
use iced::{Background, Border, Color, Element, Length};

use crate::styles::BOLD;

const PALETTE: [Color; 8] = [
    Color::from_rgb(0.231, 0.510, 0.965),
    Color::from_rgb(0.604, 0.361, 0.965),
    Color::from_rgb(0.925, 0.282, 0.600),
    Color::from_rgb(0.937, 0.388, 0.247),
    Color::from_rgb(0.918, 0.620, 0.180),
    Color::from_rgb(0.196, 0.706, 0.486),
    Color::from_rgb(0.149, 0.667, 0.737),
    Color::from_rgb(0.514, 0.553, 0.612),
];

fn initial(name: &str) -> char {
    name.chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('?')
}

// Stable per name: same app keeps its color across launches.
fn background_for(name: &str) -> Color {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    PALETTE[(h.finish() % PALETTE.len() as u64) as usize]
}

fn foreground_for(bg: Color) -> Color {
    let luma = 0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b;
    if luma > 0.6 {
        Color::from_rgb(0.09, 0.10, 0.12)
    } else {
        Color::WHITE
    }
}

pub fn fallback_icon<'a, M: 'a>(name: &str, size: f32) -> Element<'a, M> {
    let bg = background_for(name);
    let fg = foreground_for(bg);
    let letter = text(initial(name).to_string())
        .size(size * 0.5)
        .font(BOLD)
        .color(fg);
    container(letter)
        .center(Length::Fixed(size))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: (size * 0.22).into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_uppercases_first_char() {
        assert_eq!(initial("firefox"), 'F');
        assert_eq!(initial("gimp"), 'G');
    }

    #[test]
    fn initial_handles_empty() {
        assert_eq!(initial(""), '?');
    }

    #[test]
    fn background_is_always_in_palette() {
        for name in ["firefox", "gimp", "", "0ad", "héllo"] {
            assert!(PALETTE.contains(&background_for(name)));
        }
    }

    #[test]
    fn background_varies_across_names() {
        let names = [
            "firefox", "gimp", "vlc", "emacs", "krita", "blender", "inkscape", "audacity",
        ];
        let distinct: std::collections::HashSet<_> = names
            .iter()
            .map(|n| {
                let c = background_for(n);
                (c.r.to_bits(), c.g.to_bits(), c.b.to_bits())
            })
            .collect();
        assert!(distinct.len() > 1, "background_for must depend on the name");
    }

    #[test]
    fn foreground_is_white_on_dark_and_dark_on_light() {
        assert_eq!(foreground_for(Color::from_rgb(0.1, 0.1, 0.1)), Color::WHITE);
        assert_eq!(
            foreground_for(Color::from_rgb(0.95, 0.95, 0.95)),
            Color::from_rgb(0.09, 0.10, 0.12)
        );
    }
}
