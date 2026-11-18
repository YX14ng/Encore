//! Asking GitHub for Encore's latest release: one request to its public API, which says the
//! version, the page of the release and each package with its SHA-256.

use serde::Deserialize;

const LATEST: &str = "https://api.github.com/repos/YX14ng/Encore/releases/latest";

/// The version Encore is.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// A version, `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// `0.5.1` or `v0.5.1`.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().trim_start_matches('v').split('.');
        let mut next = || parts.next()?.parse().ok();
        let version = Version(next()?, next()?, next()?);
        Some(version)
    }

    pub fn current() -> Self {
        Self::parse(CURRENT).unwrap_or(Version(0, 0, 0))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// A file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    /// The file's SHA-256 in hex, as GitHub reports it.
    pub sha256: Option<String>,
}

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The release's page, with its notes.
    pub page: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

/// Reads GitHub's answer about a release.
pub fn parse(json: &str) -> Result<Release, String> {
    let raw: RawRelease = serde_json::from_str(json).map_err(|error| error.to_string())?;
    if raw.draft || raw.prerelease {
        return Err("not a release".to_string());
    }
    let version =
        Version::parse(&raw.tag_name).ok_or_else(|| format!("unknown version {}", raw.tag_name))?;
    Ok(Release {
        version,
        page: raw.html_url,
        assets: raw
            .assets
            .into_iter()
            .map(|asset| Asset {
                name: asset.name,
                url: asset.browser_download_url,
                sha256: asset
                    .digest
                    .and_then(|digest| digest.strip_prefix("sha256:").map(str::to_string)),
            })
            .collect(),
    })
}

/// The latest release on GitHub.
pub async fn latest() -> Result<Release, String> {
    let response = reqwest::Client::new()
        .get(LATEST)
        // GitHub asks every client to say who it is; nothing about the user goes with it.
        .header(reqwest::header::USER_AGENT, format!("Encore/{CURRENT}"))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    let json = response.text().await.map_err(|error| error.to_string())?;
    parse(&json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert_eq!(Version::parse("v0.5.0"), Some(Version(0, 5, 0)));
        assert!(Version::parse("0.10.0") > Version::parse("0.9.9"));
        assert!(Version::parse("1.0.0") > Version::parse("0.99.0"));
        assert_eq!(Version::parse("v0.5"), None);
        assert_eq!(Version(0, 5, 1).to_string(), "0.5.1");
    }

    #[test]
    fn reads_a_release_with_its_checksums() {
        let json = r#"{"tag_name":"v0.5.1","html_url":"https://github.com/YX14ng/Encore/releases/tag/v0.5.1",
            "draft":false,"prerelease":false,"assets":[
            {"name":"encore-ytm-0.5.1-1-x86_64.pkg.tar.zst","browser_download_url":"https://x/a","digest":"sha256:abc"},
            {"name":"old.zip","browser_download_url":"https://x/b","digest":null}]}"#;
        let release = parse(json).expect("release");
        assert_eq!(release.version, Version(0, 5, 1));
        let asset = release
            .asset("encore-ytm-0.5.1-1-x86_64.pkg.tar.zst")
            .expect("asset");
        assert_eq!(asset.sha256.as_deref(), Some("abc"));
        assert_eq!(
            release.asset("old.zip").and_then(|a| a.sha256.clone()),
            None
        );
        let draft = r#"{"tag_name":"v9.0.0","html_url":"x","draft":true}"#;
        assert!(parse(draft).is_err());
    }
}
