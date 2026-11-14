//! Covers the user picks for playlists. YouTube Music has no way to change a playlist's
//! picture, so they stay on this computer: one square PNG per playlist in
//! `~/.local/share/encore/covers/`, named after the playlist's id.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::widget::image::Handle;

/// The side of the saved square, in pixels: plenty for the 180 px header on a 2× screen.
const SIDE: u32 = 600;
/// Bigger files are refused before decoding them.
const MAX_BYTES: u64 = 40 * 1024 * 1024;
/// The file types offered in the file chooser.
pub const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// The folder that holds the covers.
pub fn dir() -> PathBuf {
    crate::account::data_dir().join("covers")
}

/// Every saved cover, by playlist id, ready to draw.
pub fn load() -> HashMap<String, Handle> {
    load_from(&dir())
}

fn load_from(dir: &Path) -> HashMap<String, Handle> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return HashMap::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let id = path
                .file_name()?
                .to_str()?
                .strip_suffix(".png")?
                .to_string();
            let bytes = std::fs::read(&path).ok()?;
            is_playlist_id(&id).then(|| (id, Handle::from_bytes(bytes)))
        })
        .collect()
}

/// Makes the image at `source` the cover of playlist `id`: cut to a centered square, scaled
/// to `SIDE` and saved as PNG. Returns the new cover. Slow; run it off the interface thread.
pub fn save(id: &str, source: &Path) -> Result<Handle, String> {
    save_in(&dir(), id, source)
}

fn save_in(dir: &Path, id: &str, source: &Path) -> Result<Handle, String> {
    if !is_playlist_id(id) {
        return Err(format!("{id:?} is not a playlist id"));
    }
    let size = std::fs::metadata(source)
        .map_err(|error| format!("Could not read {}: {error}", source.display()))?
        .len();
    if size > MAX_BYTES {
        return Err("The image is bigger than 40 MB.".to_string());
    }
    let picture = image::open(source).map_err(|error| {
        format!(
            "{} is not an image Encore can read: {error}",
            source.display()
        )
    })?;
    let square = square(&picture).resize_exact(SIDE, SIDE, image::imageops::FilterType::Lanczos3);

    crate::account::private_dir(dir)?;
    let path = dir.join(format!("{id}.png"));
    let partial = dir.join(format!("{id}.png.part"));
    square
        .save_with_format(&partial, image::ImageFormat::Png)
        .map_err(|error| format!("Could not save the cover: {error}"))?;
    std::fs::rename(&partial, &path)
        .map_err(|error| format!("Could not save the cover: {error}"))?;
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    Ok(Handle::from_bytes(bytes))
}

/// Goes back to YouTube's picture for playlist `id`.
pub fn remove(id: &str) -> Result<(), String> {
    remove_from(&dir(), id)
}

fn remove_from(dir: &Path, id: &str) -> Result<(), String> {
    if !is_playlist_id(id) {
        return Ok(());
    }
    match std::fs::remove_file(dir.join(format!("{id}.png"))) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove the cover: {error}")),
    }
}

/// Asks the desktop's file chooser (through the XDG portal) for an image. `None` if the user
/// cancels or there is no file chooser.
pub async fn pick() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Playlist cover")
        .add_filter("Images", &EXTENSIONS)
        .pick_file()
        .await
        .map(|file| file.path().to_path_buf())
}

/// The largest centered square of `picture`.
fn square(picture: &image::DynamicImage) -> image::DynamicImage {
    let (width, height) = (picture.width(), picture.height());
    let side = width.min(height);
    picture.crop_imm((width - side) / 2, (height - side) / 2, side, side)
}

/// Playlist ids are letters, digits, "-" and "_", so they are safe as file names.
fn is_playlist_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_playlist_ids_become_file_names() {
        assert!(is_playlist_id("PLEphN-1_x"));
        assert!(!is_playlist_id("../escape"));
        assert!(!is_playlist_id("a/b"));
        assert!(!is_playlist_id(""));
    }

    #[test]
    fn cuts_the_middle_square() {
        let wide = image::DynamicImage::new_rgb8(300, 100);
        let square = square(&wide);
        assert_eq!((square.width(), square.height()), (100, 100));
        let tall = image::DynamicImage::new_rgb8(80, 200);
        assert_eq!(square_size(&tall), 80);
    }

    fn square_size(picture: &image::DynamicImage) -> u32 {
        let square = square(picture);
        assert_eq!(square.width(), square.height());
        square.width()
    }

    #[test]
    fn saves_and_removes_a_cover() {
        let home = std::env::temp_dir().join(format!("encore-covers-{}", std::process::id()));
        let covers = home.join("covers");
        let source = home.join("picture.png");
        std::fs::create_dir_all(&home).expect("dir");
        image::DynamicImage::new_rgb8(40, 30)
            .save(&source)
            .expect("picture");
        save_in(&covers, "PLtest", &source).expect("save");
        let saved = image::open(covers.join("PLtest.png")).expect("saved cover");
        assert_eq!((saved.width(), saved.height()), (SIDE, SIDE));
        assert!(load_from(&covers).contains_key("PLtest"));
        assert!(save_in(&covers, "PLtest", &home.join("missing.png")).is_err());
        assert!(save_in(&covers, "../x", &source).is_err());
        remove_from(&covers, "PLtest").expect("remove");
        assert!(load_from(&covers).is_empty());
        let _ = std::fs::remove_dir_all(&home);
    }
}
