//! What differs between Linux and Windows: where Encore keeps its files, files only the user may
//! read, starting the programs Encore uses (yt-dlp, ffmpeg) and opening links.
//!
//! On Linux Encore follows the XDG folders and finds its programs on the PATH. On Windows it
//! keeps its files under `%APPDATA%` and `%LOCALAPPDATA%`, ships yt-dlp, deno and ffmpeg next to
//! `encore.exe`, and starts them without a console window.

use std::path::{Path, PathBuf};

/// The user's home folder.
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let var = "USERPROFILE";
    #[cfg(not(windows))]
    let var = "HOME";
    std::env::var_os(var).map(PathBuf::from)
}

/// Data that must survive cache cleaning: `$XDG_DATA_HOME/encore` or `~/.local/share/encore`;
/// `%APPDATA%\Encore` on Windows.
pub fn data_dir() -> PathBuf {
    folder("XDG_DATA_HOME", ".local/share", "APPDATA", None)
}

/// Settings and themes: `$XDG_CONFIG_HOME/encore` or `~/.config/encore`;
/// `%APPDATA%\Encore\config` on Windows.
pub fn config_dir() -> PathBuf {
    folder("XDG_CONFIG_HOME", ".config", "APPDATA", Some("config"))
}

/// What can be downloaded again: `$XDG_CACHE_HOME/encore` or `~/.cache/encore`;
/// `%LOCALAPPDATA%\Encore\cache` on Windows.
pub fn cache_dir() -> PathBuf {
    folder("XDG_CACHE_HOME", ".cache", "LOCALAPPDATA", Some("cache"))
}

#[cfg(not(windows))]
fn folder(xdg: &str, in_home: &str, _windows: &str, _inner: Option<&str>) -> PathBuf {
    std::env::var_os(xdg)
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(in_home)))
        .unwrap_or_else(std::env::temp_dir)
        .join("encore")
}

#[cfg(windows)]
fn folder(_xdg: &str, _in_home: &str, windows: &str, inner: Option<&str>) -> PathBuf {
    let encore = std::env::var_os(windows)
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Encore");
    match inner {
        Some(inner) => encore.join(inner),
        None => encore,
    }
}

/// Creates `dir` readable only by the user. On Windows the folders under the user's profile
/// already are.
pub fn private_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Writes `contents` to a new file at `path` that only the user can read.
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(contents)
}

/// One of the programs Encore runs, ready to be given arguments.
pub fn command(program: &str) -> tokio::process::Command {
    let command = tokio::process::Command::new(program_path(program));
    #[cfg(windows)]
    let command = {
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        if let Some(path) = path_with_bundled() {
            command.env("PATH", path);
        }
        command
    };
    command
}

/// [`command`] for code that does not wait asynchronously.
pub fn std_command(program: &str) -> std::process::Command {
    let command = std::process::Command::new(program_path(program));
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        if let Some(path) = path_with_bundled() {
            command.env("PATH", path);
        }
        command
    };
    command
}

/// Keeps a console program from opening a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The folder of Encore's own executable, where the Windows build ships its programs.
#[cfg(windows)]
fn bundled_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Where to run `program` from: the copy shipped next to `encore.exe` on Windows, or the PATH.
fn program_path(program: &str) -> PathBuf {
    #[cfg(windows)]
    if let Some(bundled) = bundled_dir()
        .map(|dir| dir.join(format!("{program}.exe")))
        .filter(|path| path.is_file())
    {
        return bundled;
    }
    PathBuf::from(program)
}

/// The PATH with Encore's folder first, so yt-dlp finds the deno and ffmpeg shipped with it.
#[cfg(windows)]
fn path_with_bundled() -> Option<std::ffi::OsString> {
    let dir = bundled_dir()?;
    let rest = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(std::iter::once(dir).chain(std::env::split_paths(&rest))).ok()
}

/// Opens `url` in the default browser.
pub fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(not(windows))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(url)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encore_has_a_folder_of_each_kind() {
        let folders = [data_dir(), config_dir(), cache_dir()];
        assert!(folders.iter().all(|dir| dir.is_absolute()));
        assert!(folders[0] != folders[1] && folders[1] != folders[2]);
    }

    #[cfg(unix)]
    #[test]
    fn private_files_are_only_for_the_user() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("encore-private-{}", std::process::id()));
        private_dir(&dir).expect("dir");
        let file = dir.join("secret");
        write_private(&file, b"x").expect("file");
        let mode = |path: &Path| std::fs::metadata(path).expect("meta").permissions().mode();
        assert_eq!(mode(&dir) & 0o777, 0o700);
        assert_eq!(mode(&file) & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn programs_run_from_the_path_unless_shipped() {
        assert_eq!(
            program_path("encore-no-such-program"),
            PathBuf::from("encore-no-such-program")
        );
    }
}
