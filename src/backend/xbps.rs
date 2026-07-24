use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct XbpsBackend;

impl XbpsBackend {
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
            .map_err(|e| format!("Falha ao iniciar o xbps-install (com sudo): {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Xbps falhou com código: {:?}", status.code()))
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
        let mut cmd = Command::new("xbps-query");
        cmd.arg("-S").arg(package);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        if let Ok(status) = cmd.status().await {
            status.success()
        } else {
            false
        }
    }

    fn split_name_version(&self, pkg_ver: &str) -> (String, String) {
        if let Some(idx) = pkg_ver.rfind('-') {
            (pkg_ver[..idx].to_string(), pkg_ver[idx + 1..].to_string())
        } else {
            (pkg_ver.to_string(), "unknown".to_string())
        }
    }
}

#[async_trait::async_trait]
impl PackageManagerBackend for XbpsBackend {
    fn name(&self) -> &str {
        "XBPS"
    }

    fn is_available(&self) -> bool {
        has_binary("xbps-query") && has_binary("xbps-install")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = match self.run_capture("xbps-query", &["-Rs", query]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for line in output.lines() {
            if let Some((pkg_ver_part, desc)) = line.split_once(" - ") {
                let pkg_ver_part = pkg_ver_part.trim();
                let clean_pkg_ver = pkg_ver_part
                    .strip_prefix("[-] ")
                    .unwrap_or(pkg_ver_part)
                    .strip_prefix("[*] ")
                    .unwrap_or(pkg_ver_part)
                    .trim();

                let (name, version) = self.split_name_version(clean_pkg_ver);

                results.push(SearchResult {
                    name,
                    description: desc.trim().to_string(),
                    version,
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_privileged("xbps-install", &["-Sy", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("xbps-remove", &["-Ry", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("xbps-install", &["-S"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let output = self.run_capture("xbps-query", &["-R", package]).await?;
        
        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut homepage = None;
        let mut size = None;

        for line in output.lines() {
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let val = val.trim();

                match key {
                    "pkgver" => {
                        let (_, ver) = self.split_name_version(val);
                        version = ver;
                    }
                    "short_desc" => description = val.to_string(),
                    "homepage" => homepage = Some(val.to_string()),
                    "installed_size" => size = Some(val.to_string()),
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
        let output = self.run_capture("xbps-query", &["-l"]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                let (name, version) = self.split_name_version(tokens[1]);
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
