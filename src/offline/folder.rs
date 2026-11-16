//! Moving the downloads to another folder, when the user picks one in the settings. Songs and
//! covers are moved (renamed when both folders are on the same disk, else copied and deleted),
//! and the list of downloads is merged with any the new folder already holds, so pointing
//! Encore back at an older downloads folder keeps both.

use std::path::{Path, PathBuf};

use rust_i18n::t;

use super::{INDEX, PENDING, read_json, write_json};
use crate::youtube::Collection;

/// Moves everything in `from` into `to`. Files `to` already has are kept, and the copy in
/// `from` is dropped.
pub async fn move_downloads(from: PathBuf, to: PathBuf) -> Result<(), String> {
    tokio::task::spawn_blocking(move || move_all(&from, &to))
        .await
        .map_err(|error| error.to_string())?
}

fn move_all(from: &Path, to: &Path) -> Result<(), String> {
    if to.starts_with(from) || from.starts_with(to) {
        return Err(t!("audio.download_folder_inside").into_owned());
    }
    std::fs::create_dir_all(to).map_err(|error| describe(to, &error))?;
    if !from.is_dir() {
        return Ok(());
    }

    let index = merged::<Collection>(from, to, INDEX, |collection| collection.id.clone());
    let pending = merged::<String>(from, to, PENDING, String::clone);
    for entry in std::fs::read_dir(from).map_err(|error| describe(from, &error))? {
        let entry = entry.map_err(|error| describe(from, &error))?;
        let name = entry.file_name();
        if name == INDEX || name == PENDING {
            continue;
        }
        move_entry(&entry.path(), &to.join(&name))?;
    }
    write_json(&to.join(INDEX), &index)?;
    if !pending.is_empty() {
        write_json(&to.join(PENDING), &pending)?;
    }
    for name in [INDEX, PENDING] {
        // A list that is already gone is fine.
        let _ = std::fs::remove_file(from.join(name));
    }
    // Only an empty folder goes; anything left in it stays where it was.
    let _ = std::fs::remove_dir(from);
    Ok(())
}

/// The list `name` of both folders: `to`'s entries, then those only `from` has.
fn merged<T: serde::Serialize + serde::de::DeserializeOwned>(
    from: &Path,
    to: &Path,
    name: &str,
    key: impl Fn(&T) -> String,
) -> Vec<T> {
    let mut list: Vec<T> = read_json(&to.join(name)).unwrap_or_default();
    let known: std::collections::HashSet<String> = list.iter().map(&key).collect();
    let moved: Vec<T> = read_json(&from.join(name)).unwrap_or_default();
    list.extend(moved.into_iter().filter(|item| !known.contains(&key(item))));
    list
}

/// Moves a file or folder, keeping whatever `target` already has.
fn move_entry(source: &Path, target: &Path) -> Result<(), String> {
    if source.is_dir() {
        std::fs::create_dir_all(target).map_err(|error| describe(target, &error))?;
        for entry in std::fs::read_dir(source).map_err(|error| describe(source, &error))? {
            let entry = entry.map_err(|error| describe(source, &error))?;
            move_entry(&entry.path(), &target.join(entry.file_name()))?;
        }
        let _ = std::fs::remove_dir(source);
        return Ok(());
    }
    if target.exists() {
        return std::fs::remove_file(source).map_err(|error| describe(source, &error));
    }
    if std::fs::rename(source, target).is_ok() {
        return Ok(());
    }
    // Another disk: copy, then delete.
    std::fs::copy(source, target).map_err(|error| describe(target, &error))?;
    std::fs::remove_file(source).map_err(|error| describe(source, &error))
}

fn describe(path: &Path, error: &std::io::Error) -> String {
    t!(
        "audio.download_folder_failed",
        error = format!("{}: {error}", path.display())
    )
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::youtube::TileKind;

    fn collection(id: &str) -> Collection {
        Collection {
            id: id.to_string(),
            kind: TileKind::Playlist,
            title: id.to_string(),
            subtitle: String::new(),
            image_url: None,
            tracks: Vec::new(),
            artist: None,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("encore-move-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn moves_songs_covers_and_the_list_merging_what_is_there() {
        let from = scratch("from");
        let to = scratch("to");
        std::fs::create_dir_all(from.join("images")).expect("from");
        std::fs::create_dir_all(&to).expect("to");
        std::fs::write(from.join("a.webm"), b"a").expect("song");
        std::fs::write(from.join("shared.webm"), b"old").expect("song");
        std::fs::write(from.join("images").join("c1"), b"c").expect("cover");
        std::fs::write(to.join("shared.webm"), b"kept").expect("song");
        write_json(&from.join(INDEX), &vec![collection("x"), collection("y")]).expect("index");
        write_json(&from.join(PENDING), &vec!["y".to_string()]).expect("pending");
        write_json(&to.join(INDEX), &vec![collection("y"), collection("z")]).expect("index");

        move_all(&from, &to).expect("moved");

        assert_eq!(std::fs::read(to.join("a.webm")).expect("a"), b"a");
        assert_eq!(std::fs::read(to.join("shared.webm")).expect("s"), b"kept");
        assert_eq!(
            std::fs::read(to.join("images").join("c1")).expect("c"),
            b"c"
        );
        let index: Vec<Collection> = read_json(&to.join(INDEX)).expect("index");
        let ids: Vec<&str> = index.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["y", "z", "x"]);
        let pending: Vec<String> = read_json(&to.join(PENDING)).expect("pending");
        assert_eq!(pending, ["y"]);
        assert!(!from.exists(), "the old folder is gone once empty");
        let _ = std::fs::remove_dir_all(&to);
    }

    #[test]
    fn a_folder_inside_the_downloads_is_refused() {
        let from = scratch("outer");
        assert!(move_all(&from, &from.join("inner")).is_err());
        assert!(move_all(&from.join("inner"), &from).is_err());
    }
}
