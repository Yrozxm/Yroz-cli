use crate::backend::{
    InstalledPackage, PackageInfo, PackageManagerBackend, SearchResult, has_binary,
};
use std::process::Stdio;
use tokio::process::Command;

pub struct FlatpakBackend;

impl FlatpakBackend {
    pub fn new() -> Self {
        Self
    }

    async fn run_capture(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.env("LC_ALL", "C");

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Falha ao executar o comando {}: {}", program, e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}

#[async_trait::async_trait]
impl PackageManagerBackend for FlatpakBackend {
    fn name(&self) -> &str {
        "Flatpak"
    }

    fn is_available(&self) -> bool {
        has_binary("flatpak")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = self.run_capture("flatpak", &["search", query]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                let name = parts[2].trim().to_string();
                let friendly_name = parts[0].trim();
                let desc = parts[1].trim();
                let version = parts[3].trim().to_string();

                results.push(SearchResult {
                    name,
                    description: format!("{} - {}", friendly_name, desc),
                    version: if version.is_empty() {
                        "stable".to_string()
                    } else {
                        version
                    },
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        let mut cmd = Command::new("flatpak");

        cmd.args(&["install", "-y", "flathub", package])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd
            .status()
            .await
            .map_err(|e| format!("Falha ao iniciar o flatpak: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "flatpak install falhou com código: {:?}",
                status.code()
            ))
        }
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        let mut cmd = Command::new("flatpak");
        cmd.args(&["uninstall", "-y", package])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd
            .status()
            .await
            .map_err(|e| format!("Falha ao iniciar o flatpak: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "flatpak uninstall falhou com código: {:?}",
                status.code()
            ))
        }
    }

    async fn update(&self) -> Result<(), String> {
        let mut cmd = Command::new("flatpak");
        cmd.args(&["update", "-y"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd
            .status()
            .await
            .map_err(|e| format!("Falha ao iniciar o flatpak: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "flatpak update falhou com código: {:?}",
                status.code()
            ))
        }
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let (output, installed) = match self.run_capture("flatpak", &["info", package]).await {
            Ok(out) => (out, true),
            Err(_) => {
                let out = self
                    .run_capture("flatpak", &["remote-info", "flathub", package])
                    .await?;
                (out, false)
            }
        };

        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut size = None;

        let lines: Vec<&str> = output.lines().collect();
        if !lines.is_empty() {
            description = lines[0].to_string();
            if lines.len() > 1 && !lines[1].is_empty() {
                description = format!("{} - {}", description, lines[1].trim());
            }
        }

        for line in output.lines() {
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let val = val.trim();

                match key {
                    "Version" | "Versão" => version = val.to_string(),
                    "Installed" | "Instalado" => size = Some(val.to_string()),
                    _ => {}
                }
            }
        }

        let homepage = Some(format!("https://flathub.org/apps/{}", package));

        Ok(PackageInfo {
            name: package.to_string(),
            version,
            description,
            backend: self.name().to_string(),
            homepage,
            size,
            installed,
        })
    }

    async fn list(&self) -> Result<Vec<InstalledPackage>, String> {
        let output = self
            .run_capture("flatpak", &["list", "--columns=application,version"])
            .await?;
        let mut results = Vec::new();

        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if !parts.is_empty() {
                let name = parts[0].trim().to_string();
                let version = if parts.len() > 1 {
                    parts[1].trim().to_string()
                } else {
                    "unknown".to_string()
                };
                results.push(InstalledPackage {
                    name,
                    version,
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }
}
