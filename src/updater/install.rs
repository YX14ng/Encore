//! Installing a new version: finding how Encore was installed, downloading the matching package
//! while checking its SHA-256, and handing it to the system's package manager (with `pkexec`,
//! which asks for the password) or, on Windows, to the installer.

use std::path::{Path, PathBuf};

use rust_i18n::t;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::github::{Asset, CURRENT, Release, Version};

/// How Encore was installed, which decides how it updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The Arch package (`pacman`).
    Arch,
    /// The `.deb` (Linux Mint, Ubuntu).
    Deb,
    /// The `.rpm` (Fedora).
    Rpm,
    /// Windows, with the installer.
    WindowsSetup,
    /// Anything else (the `.tar.gz`, the Windows `.zip`, a build from source): the release page
    /// is opened instead.
    Manual,
}

/// What installing did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The new version is installed; it runs once Encore starts again.
    Installed,
    /// The Windows installer is running and needs Encore closed.
    InstallerStarted,
}

/// The package of `release` for `kind`, if it is published yet.
pub fn asset_for(release: &Release, kind: Kind) -> Option<&Asset> {
    let version = release.version;
    let name = match kind {
        Kind::Arch => format!("encore-ytm-{version}-1-x86_64.pkg.tar.zst"),
        Kind::Deb => format!("encore-ytm_{version}-1_amd64.deb"),
        Kind::Rpm => format!("encore-ytm-{version}-1.x86_64.rpm"),
        Kind::WindowsSetup => format!("encore-{version}-x86_64-windows-setup.exe"),
        Kind::Manual => return None,
    };
    release.asset(&name)
}

/// How this Encore was installed.
pub async fn detect() -> Kind {
    let Ok(exe) = std::env::current_exe().and_then(std::fs::canonicalize) else {
        return Kind::Manual;
    };
    detect_at(&exe).await
}

#[cfg(windows)]
async fn detect_at(exe: &Path) -> Kind {
    // The installer leaves its uninstaller next to encore.exe; the .zip does not.
    let installed = exe
        .parent()
        .is_some_and(|dir| dir.join("unins000.exe").is_file());
    if installed {
        Kind::WindowsSetup
    } else {
        Kind::Manual
    }
}

#[cfg(not(windows))]
async fn detect_at(exe: &Path) -> Kind {
    // The package managers say which package owns the program.
    let owned_by = |program: &str, args: &[&str]| {
        let mut command = tokio::process::Command::new(program);
        command.args(args).arg(exe);
        async move {
            command
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await
                .is_ok_and(|status| status.success())
        }
    };
    if owned_by("pacman", &["-Qqo"]).await {
        Kind::Arch
    } else if owned_by("dpkg", &["-S"]).await {
        Kind::Deb
    } else if owned_by("rpm", &["-qf"]).await {
        Kind::Rpm
    } else {
        Kind::Manual
    }
}

/// Downloads `asset`, checks it, and installs it the way `kind` does.
pub async fn install(asset: Asset, kind: Kind) -> Result<Outcome, String> {
    let file = download(&asset).await?;
    let result = run_installer(&file, kind).await;
    if kind != Kind::WindowsSetup {
        // A file that is already gone is fine; the installer still needs its own.
        let _ = tokio::fs::remove_file(&file).await;
    }
    result
}

/// Downloads `asset` into the cache, hashing it as it comes, and keeps it only if it matches
/// the SHA-256 GitHub published.
async fn download(asset: &Asset) -> Result<PathBuf, String> {
    let expected = asset
        .sha256
        .clone()
        .ok_or_else(|| t!("update.no_checksum").into_owned())?;
    let dir = crate::platform::cache_dir().join("updates");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| error.to_string())?;
    let path = dir.join(&asset.name);
    let mut response = reqwest::Client::new()
        .get(&asset.url)
        .header(reqwest::header::USER_AGENT, format!("Encore/{CURRENT}"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
    }
    file.flush().await.map_err(|error| error.to_string())?;
    let got = hex(&hasher.finalize());
    if !got.eq_ignore_ascii_case(&expected) {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(t!("update.checksum_mismatch").into_owned());
    }
    Ok(path)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The command that installs `file` for `kind`, asking for the password through `pkexec`.
fn command_for(file: &Path, kind: Kind) -> Option<(&'static str, Vec<String>)> {
    let file = file.to_string_lossy().into_owned();
    let (manager, args): (&str, Vec<&str>) = match kind {
        Kind::Arch => ("pacman", vec!["-U", "--noconfirm"]),
        Kind::Deb => ("apt-get", vec!["install", "-y"]),
        Kind::Rpm => ("dnf", vec!["install", "-y"]),
        Kind::WindowsSetup | Kind::Manual => return None,
    };
    let mut all = vec![manager.to_string()];
    all.extend(args.into_iter().map(str::to_string));
    all.push(file);
    Some(("pkexec", all))
}

async fn run_installer(file: &Path, kind: Kind) -> Result<Outcome, String> {
    if kind == Kind::WindowsSetup {
        // The installer replaces encore.exe, so it runs on its own and Encore closes; it starts
        // Encore again when it is done.
        std::process::Command::new(file)
            .args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(Outcome::InstallerStarted);
    }
    let (program, args) = command_for(file, kind).ok_or_else(|| "nothing to run".to_string())?;
    let output = tokio::process::Command::new(program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|error| error.to_string())?;
    match output.status.code() {
        Some(0) => Ok(Outcome::Installed),
        // pkexec: the password prompt was dismissed or refused.
        Some(126 | 127) => Err(t!("update.cancelled").into_owned()),
        _ => {
            let error = String::from_utf8_lossy(&output.stderr);
            let last = error.lines().rev().find(|line| !line.trim().is_empty());
            Err(last.unwrap_or("failed").trim().to_string())
        }
    }
}

/// Starts the newly installed Encore; the running one then closes.
pub fn restart() -> std::io::Result<()> {
    std::process::Command::new(std::env::current_exe()?)
        .spawn()
        .map(|_| ())
}

/// Whether `release` is newer than this Encore.
pub fn is_newer(release: &Release) -> bool {
    release.version > Version::current()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(names: &[&str]) -> Release {
        Release {
            version: Version(9, 1, 0),
            page: "https://x".to_string(),
            assets: names
                .iter()
                .map(|name| Asset {
                    name: (*name).to_string(),
                    url: format!("https://x/{name}"),
                    sha256: Some("00".to_string()),
                })
                .collect(),
        }
    }

    #[test]
    fn each_kind_takes_its_own_package() {
        let all = release(&[
            "encore-ytm-9.1.0-1-x86_64.pkg.tar.zst",
            "encore-ytm_9.1.0-1_amd64.deb",
            "encore-ytm-9.1.0-1.x86_64.rpm",
            "encore-9.1.0-x86_64-windows-setup.exe",
        ]);
        for kind in [Kind::Arch, Kind::Deb, Kind::Rpm, Kind::WindowsSetup] {
            assert!(asset_for(&all, kind).is_some(), "{kind:?}");
        }
        assert!(asset_for(&all, Kind::Manual).is_none());
        let only_rpm = release(&["encore-ytm-9.1.0-1.x86_64.rpm"]);
        assert!(
            asset_for(&only_rpm, Kind::Arch).is_none(),
            "not published yet"
        );
        assert!(is_newer(&all));
    }

    #[test]
    fn packages_install_through_pkexec() {
        let file = Path::new("/tmp/e.pkg.tar.zst");
        let (program, args) = command_for(file, Kind::Arch).expect("arch");
        assert_eq!(program, "pkexec");
        assert_eq!(args, ["pacman", "-U", "--noconfirm", "/tmp/e.pkg.tar.zst"]);
        assert_eq!(command_for(file, Kind::Deb).expect("deb").1[0], "apt-get");
        assert!(command_for(file, Kind::Manual).is_none());
    }

    #[test]
    fn hashes_print_as_hex() {
        assert_eq!(hex(&[0x0a, 0xff]), "0aff");
    }

    /// Asks GitHub for the latest release and downloads one of its Linux packages, checking it:
    /// `cargo test -- --ignored downloads_and_checks_a_real_package`.
    #[test]
    #[ignore = "downloads a package from GitHub"]
    fn downloads_and_checks_a_real_package() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let release = runtime
            .block_on(super::super::github::latest())
            .expect("release");
        let asset = [Kind::Deb, Kind::Rpm, Kind::Arch]
            .into_iter()
            .find_map(|kind| asset_for(&release, kind).cloned())
            .expect("a Linux package");
        let path = runtime
            .block_on(download(&asset))
            .expect("checked download");
        assert!(path.metadata().expect("file").len() > 1_000_000);
        let _ = std::fs::remove_file(path);

        let mut forged = asset;
        forged.sha256 = Some("00".repeat(32));
        assert!(
            runtime.block_on(download(&forged)).is_err(),
            "a wrong checksum is refused"
        );
    }
}
