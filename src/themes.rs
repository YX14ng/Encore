//! Themes: the built-in palettes, palettes read from `~/.config/encore/themes/*.json`, the
//! accent color choice (the theme's, a fixed one, or taken from the playing song's cover) and
//! the appearance settings that remember them.

use std::path::PathBuf;

use iced::Color;
use serde::{Deserialize, Serialize};

use crate::theme::{ENCORE, Palette};

/// A theme the user can pick.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub palette: Palette,
}

/// The themes that come with Encore.
pub fn built_in() -> Vec<Theme> {
    let theme = |name: &str, palette| Theme {
        name: name.to_string(),
        palette,
    };
    vec![
        theme("Encore", ENCORE),
        theme(
            "Midnight",
            Palette {
                sidebar: rgb(0x0e, 0x12, 0x1f),
                center: rgb(0x12, 0x17, 0x27),
                lyrics: rgb(0x10, 0x14, 0x22),
                player: rgb(0x0f, 0x13, 0x21),
                hero: rgb(0x1e, 0x25, 0x3a),
                text: rgb(0xec, 0xef, 0xf8),
                text_secondary: rgb(0x91, 0x9a, 0xb4),
                text_dim: rgb(0x55, 0x5e, 0x78),
                accent: rgb(0x6c, 0x8c, 0xff),
                ..ENCORE
            },
        ),
        theme(
            "OLED",
            Palette {
                sidebar: rgb(0x00, 0x00, 0x00),
                center: rgb(0x00, 0x00, 0x00),
                lyrics: rgb(0x00, 0x00, 0x00),
                player: rgb(0x08, 0x08, 0x08),
                hero: rgb(0x14, 0x14, 0x14),
                ..ENCORE
            },
        ),
        theme(
            "Nord",
            Palette {
                sidebar: rgb(0x2b, 0x30, 0x3b),
                center: rgb(0x2e, 0x34, 0x40),
                lyrics: rgb(0x2b, 0x30, 0x3b),
                player: rgb(0x29, 0x2e, 0x39),
                hero: rgb(0x3b, 0x42, 0x52),
                text: rgb(0xec, 0xef, 0xf4),
                text_secondary: rgb(0xa3, 0xab, 0xbc),
                text_dim: rgb(0x6b, 0x74, 0x88),
                accent: rgb(0x88, 0xc0, 0xd0),
                ..ENCORE
            },
        ),
        theme(
            "Daylight",
            Palette {
                sidebar: rgb(0xee, 0xee, 0xf1),
                center: rgb(0xfa, 0xfa, 0xfb),
                lyrics: rgb(0xf3, 0xf3, 0xf6),
                player: rgb(0xf0, 0xf0, 0xf3),
                hero: rgb(0xdd, 0xdd, 0xe2),
                text: rgb(0x16, 0x16, 0x1a),
                text_secondary: rgb(0x5d, 0x5d, 0x68),
                text_dim: rgb(0x9a, 0x9a, 0xa5),
                accent: rgb(0xe8, 0x2e, 0x47),
                ink: Color::BLACK,
                light: true,
            },
        ),
    ]
}

const fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::from_rgb8(red, green, blue)
}

/// Fixed accent colors offered next to "theme" and "from cover".
pub const ACCENTS: [Color; 8] = [
    rgb(0xff, 0x44, 0x59),
    rgb(0xff, 0x5c, 0xa8),
    rgb(0xff, 0x8a, 0x3d),
    rgb(0xf5, 0xc2, 0x42),
    rgb(0x3d, 0xd6, 0x8c),
    rgb(0x2e, 0xc4, 0xc4),
    rgb(0x4d, 0x8d, 0xff),
    rgb(0xa2, 0x6b, 0xff),
];

/// Where the accent color comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Accent {
    /// The theme's own accent.
    #[default]
    Theme,
    /// The most colorful part of the playing song's cover, like Cider and Pear do.
    Cover,
    /// One of `ACCENTS`.
    Fixed(usize),
}

/// The appearance choices, kept in `~/.config/encore/appearance.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// The name of the chosen theme.
    pub theme: String,
    pub accent: Accent,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "Encore".to_string(),
            accent: Accent::Theme,
        }
    }
}

impl Appearance {
    /// Reads the saved choices; a missing or broken file gives the defaults.
    pub fn load() -> Self {
        std::fs::read_to_string(config_dir().join("appearance.json"))
            .ok()
            .and_then(|json| serde_json::from_str::<Self>(&json).ok())
            .unwrap_or_default()
    }

    /// Saves the choices. A failed write only means they are not remembered next time.
    pub fn save(&self) {
        let dir = config_dir();
        if std::fs::create_dir_all(&dir).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(self)
        {
            let _ = std::fs::write(dir.join("appearance.json"), json);
        }
    }

    /// The palette these choices give among `themes`, with the cover's color if the accent
    /// follows the cover and one is known. An unknown theme name falls back to Encore's.
    pub fn palette(&self, themes: &[Theme], cover: Option<Color>) -> Palette {
        let mut palette = themes
            .iter()
            .find(|theme| theme.name == self.theme)
            .map_or(ENCORE, |theme| theme.palette);
        match self.accent {
            Accent::Theme => {}
            Accent::Cover => {
                if let Some(color) = cover {
                    palette.accent = readable(color, palette.light);
                }
            }
            Accent::Fixed(index) => {
                if let Some(color) = ACCENTS.get(index) {
                    palette.accent = *color;
                }
            }
        }
        palette
    }
}

/// `$XDG_CONFIG_HOME/encore`, or `~/.config/encore`.
pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("encore")
}

/// Where custom themes live.
pub fn themes_dir() -> PathBuf {
    config_dir().join("themes")
}

/// A theme file: a name and colors as `#rrggbb`. Missing colors come from Encore's dark palette,
/// or from Daylight with `"light": true`.
#[derive(Debug, Deserialize)]
struct ThemeFile {
    name: String,
    #[serde(default)]
    light: bool,
    sidebar: Option<String>,
    center: Option<String>,
    lyrics: Option<String>,
    player: Option<String>,
    hero: Option<String>,
    text: Option<String>,
    text_secondary: Option<String>,
    text_dim: Option<String>,
    accent: Option<String>,
}

/// Every theme: the built-in ones, then the custom ones sorted by file name. Files that cannot
/// be read are reported as `"file.json: why"` instead.
pub fn load_all() -> (Vec<Theme>, Vec<String>) {
    let mut themes = built_in();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(themes_dir()) else {
        return (themes, errors);
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    for file in files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        match std::fs::read_to_string(&file)
            .map_err(|error| error.to_string())
            .and_then(|json| parse_theme(&json))
        {
            Ok(theme) => {
                themes.retain(|known| known.name != theme.name);
                themes.push(theme);
            }
            Err(error) => errors.push(format!("{name}: {error}")),
        }
    }
    (themes, errors)
}

/// Reads one theme file.
pub fn parse_theme(json: &str) -> Result<Theme, String> {
    let file: ThemeFile = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let base = if file.light {
        built_in()
            .into_iter()
            .find(|theme| theme.palette.light)
            .map_or(ENCORE, |theme| theme.palette)
    } else {
        ENCORE
    };
    let color = |field: &str, value: &Option<String>, fallback: Color| match value {
        Some(hex) => parse_hex(hex).ok_or(format!("{field} is not a #rrggbb color: {hex:?}")),
        None => Ok(fallback),
    };
    Ok(Theme {
        name: file.name.trim().to_string(),
        palette: Palette {
            sidebar: color("sidebar", &file.sidebar, base.sidebar)?,
            center: color("center", &file.center, base.center)?,
            lyrics: color("lyrics", &file.lyrics, base.lyrics)?,
            player: color("player", &file.player, base.player)?,
            hero: color("hero", &file.hero, base.hero)?,
            text: color("text", &file.text, base.text)?,
            text_secondary: color("text_secondary", &file.text_secondary, base.text_secondary)?,
            text_dim: color("text_dim", &file.text_dim, base.text_dim)?,
            accent: color("accent", &file.accent, base.accent)?,
            ink: base.ink,
            light: base.light,
        },
    })
}

/// `#rrggbb` (the `#` is optional) as a color.
pub fn parse_hex(hex: &str) -> Option<Color> {
    let digits = hex.trim().trim_start_matches('#');
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some(rgb(channel(0)?, channel(2)?, channel(4)?))
}

/// The most colorful color of a cover image: hues are counted weighted by how saturated and
/// bright each pixel is, and the strongest hue's average color wins. `None` for images that
/// are gray or cannot be read.
pub fn cover_color(bytes: &[u8]) -> Option<Color> {
    const BUCKETS: usize = 24;
    let image = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(32, 32)
        .to_rgb8();
    let mut weights = [0.0_f32; BUCKETS];
    let mut sums = [[0.0_f32; 3]; BUCKETS];
    for pixel in image.pixels() {
        let [red, green, blue] = pixel.0.map(|channel| f32::from(channel) / 255.0);
        let (hue, saturation, value) = to_hsv(red, green, blue);
        if saturation < 0.25 || value < 0.2 {
            continue;
        }
        let bucket = ((hue / 360.0 * BUCKETS as f32) as usize).min(BUCKETS - 1);
        let weight = saturation * value;
        weights[bucket] += weight;
        for (sum, channel) in sums[bucket].iter_mut().zip([red, green, blue]) {
            *sum += channel * weight;
        }
    }
    let (best, weight) = weights
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    // A few colored pixels in a gray cover are not worth an accent.
    if weight < image.pixels().len() as f32 * 0.02 {
        return None;
    }
    let [red, green, blue] = sums[best].map(|sum| sum / weight);
    Some(Color::from_rgb(red, green, blue))
}

/// Lifts or darkens `color` so it stands out as an accent on a dark (or light) background.
pub fn readable(color: Color, light: bool) -> Color {
    let (hue, saturation, value) = to_hsv(color.r, color.g, color.b);
    let value = if light {
        value.min(0.7)
    } else {
        value.max(0.85)
    };
    from_hsv(hue, saturation.max(0.55), value)
}

fn to_hsv(red: f32, green: f32, blue: f32) -> (f32, f32, f32) {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == red {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if max == green {
        60.0 * ((blue - red) / delta + 2.0)
    } else {
        60.0 * ((red - green) / delta + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    (hue, saturation, max)
}

fn from_hsv(hue: f32, saturation: f32, value: f32) -> Color {
    let chroma = value * saturation;
    let side = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match (hue.rem_euclid(360.0) / 60.0) as u32 {
        0 => (chroma, side, 0.0),
        1 => (side, chroma, 0.0),
        2 => (0.0, chroma, side),
        3 => (0.0, side, chroma),
        4 => (side, 0.0, chroma),
        _ => (chroma, 0.0, side),
    };
    let lift = value - chroma;
    Color::from_rgb(red + lift, green + lift, blue + lift)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(pixels: &[[u8; 3]]) -> Vec<u8> {
        let image = image::RgbImage::from_fn(pixels.len() as u32, 8, |x, _| {
            image::Rgb(pixels[x as usize])
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("png");
        bytes.into_inner()
    }

    #[test]
    fn reads_hex_colors() {
        assert_eq!(parse_hex("#ff4459"), Some(rgb(0xff, 0x44, 0x59)));
        assert_eq!(parse_hex("88C0D0"), Some(rgb(0x88, 0xc0, 0xd0)));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#gg0000"), None);
    }

    #[test]
    fn a_theme_file_fills_the_gaps_and_names_bad_colors() {
        let theme = parse_theme(r##"{"name": "Mine", "accent": "#00ff00"}"##).expect("theme");
        assert_eq!(theme.name, "Mine");
        assert_eq!(theme.palette.accent, rgb(0, 0xff, 0));
        assert_eq!(theme.palette.center, ENCORE.center);

        let light = parse_theme(r#"{"name": "Paper", "light": true}"#).expect("theme");
        assert!(light.palette.light);
        assert_eq!(light.palette.ink, Color::BLACK);

        let broken = parse_theme(r#"{"name": "Bad", "text": "white"}"#);
        assert!(broken.is_err_and(|error| error.contains("text")));
    }

    #[test]
    fn the_accent_follows_the_choice() {
        let themes = built_in();
        let mut appearance = Appearance {
            theme: "Nord".to_string(),
            accent: Accent::Theme,
        };
        assert_eq!(
            appearance.palette(&themes, None).accent,
            rgb(0x88, 0xc0, 0xd0)
        );
        appearance.accent = Accent::Fixed(4);
        assert_eq!(appearance.palette(&themes, None).accent, ACCENTS[4]);
        appearance.accent = Accent::Cover;
        let nord = appearance.palette(&themes, None);
        assert_eq!(
            nord.accent,
            rgb(0x88, 0xc0, 0xd0),
            "no cover keeps the theme's"
        );
        let red = appearance.palette(&themes, Some(rgb(0x80, 0x10, 0x10)));
        assert!(
            red.accent.r > 0.8 && red.accent.g < 0.4,
            "got {:?}",
            red.accent
        );
        appearance.theme = "Gone".to_string();
        assert_eq!(appearance.palette(&themes, None).center, ENCORE.center);
    }

    #[test]
    fn finds_the_strongest_color_of_a_cover() {
        let blue = [0x20, 0x40, 0xe0];
        let cover = png(&[[0x10; 3], [0x10; 3], blue, blue, blue, [0xf0, 0x90, 0x90]]);
        let color = cover_color(&cover).expect("a color");
        assert!(color.b > 0.8 && color.r < 0.2, "got {color:?}");
        assert_eq!(cover_color(&png(&[[0x80; 3], [0x20; 3]])), None);
        assert_eq!(cover_color(b"not an image"), None);
    }

    #[test]
    fn hsv_round_trips() {
        let color = rgb(0x4d, 0x8d, 0xff);
        let (hue, saturation, value) = to_hsv(color.r, color.g, color.b);
        let back = from_hsv(hue, saturation, value);
        assert!((back.r - color.r).abs() < 1e-3 && (back.b - color.b).abs() < 1e-3);
    }
}
