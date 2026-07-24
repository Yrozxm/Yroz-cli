use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct DnfBackend;

impl DnfBackend {
    pub fn new() -> Self {
        Self
    }

    async fn run_privileged(&self, program: &str, args: &[&str]) -> Result<(), String> {
        let is_root = std::env::var("USER").map(|u| u == "root").unwrap_or(false)
            || std::process::Command::new("id")
                .arg("-u")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
                .unwrap_or(false);

        let mut cmd = if is_root {
            Command::new(program)
        } else {
            let mut c = Command::new("sudo");
            c.arg(program);
            c
        };

        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status().await
            .map_err(|e| format!("Falha ao iniciar o comando dnf (com sudo): {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Dnf falhou com código: {:?}", status.code()))
        }
    }

    async fn run_capture(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.env("LC_ALL", "C");

        let output = cmd.output().await
            .map_err(|e| format!("Falha ao executar o comando {}: {}", program, e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    async fn is_package_installed(&self, package: &str) -> bool {
        let mut cmd = Command::new("rpm");
        cmd.arg("-q").arg(package);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        if let Ok(status) = cmd.status().await {
            status.success()
        } else {
            false
        }
    }

    fn clean_package_name(&self, name_with_arch: &str) -> String {
        if let Some((clean_name, _)) = name_with_arch.split_once('.') {
            let parts: Vec<&str> = name_with_arch.split('.').collect();
            if let Some(suffix) = parts.last() {
                if ["x86_64", "i686", "noarch", "armhfp", "aarch64"].contains(suffix) {
                    return clean_name.to_string();
                }
            }
        }
        name_with_arch.to_string()
    }
}

#[async_trait::async_trait]
impl PackageManagerBackend for DnfBackend {
    fn name(&self) -> &str {
        "DNF"
    }

    fn is_available(&self) -> bool {
        has_binary("dnf") && has_binary("rpm")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = match self.run_capture("dnf", &["search", query]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for line in output.lines() {
            if line.contains("Matched:") || line.is_empty() || line.starts_with("Last metadata") {
                continue;
            }

            if let Some((full_name, desc)) = line.split_once(':') {
                let name = self.clean_package_name(full_name.trim());
                results.push(SearchResult {
                    name,
                    description: desc.trim().to_string(),
                    version: "repo".to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_privileged("dnf", &["install", "-y", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("dnf", &["remove", "-y", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("dnf", &["makecache"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let output = self.run_capture("dnf", &["info", package]).await?;
        
        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut homepage = None;
        let mut size = None;

        for line in output.lines() {
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let val = val.trim();

                match key {
                    "Version" => version = val.to_string(),
                    "Description" => description = val.to_string(),
                    "URL" => homepage = Some(val.to_string()),
                    "Size" => size = Some(val.to_string()),
                    _ => {}
                }
            }
        }

        let installed = self.is_package_installed(package).await;

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
        let output = self.run_capture("dnf", &["list", "installed"]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            if line.starts_with("Installed Packages") || line.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                let name = self.clean_package_name(tokens[0]);
                results.push(InstalledPackage {
                    name,
                    version: tokens[1].to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }
}
