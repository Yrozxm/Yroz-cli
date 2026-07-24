pub mod apt;
pub mod flatpak;
pub mod snap;
pub mod pacman;
pub mod dnf;
pub mod portage;
pub mod xbps;
pub mod apk;
pub mod yay;
pub mod nix;
pub mod zypper;
pub mod appimage;
pub mod eopkg;
pub mod napt;

use std::fmt;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub version: String,
    pub backend: String,
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub backend: String,
    pub homepage: Option<String>,
    pub size: Option<String>,
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub backend: String,
}

impl fmt::Display for SearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) - v{} [{}]",
            self.name, self.description, self.version, self.backend
        )
    }
}

#[async_trait::async_trait]
pub trait PackageManagerBackend: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String>;
    async fn install(&self, package: &str) -> Result<(), String>;
    async fn remove(&self, package: &str) -> Result<(), String>;
    async fn update(&self) -> Result<(), String>;
    async fn info(&self, package: &str) -> Result<PackageInfo, String>;
    async fn list(&self) -> Result<Vec<InstalledPackage>, String>;
}

pub fn has_binary(binary_name: &str) -> bool {
    std::process::Command::new("which")
        .arg(binary_name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
