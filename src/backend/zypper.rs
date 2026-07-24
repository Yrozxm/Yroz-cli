use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct ZypperBackend;

impl ZypperBackend {
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
            .map_err(|e| format!("Falha ao iniciar o zypper (com sudo): {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Zypper falhou com código: {:?}", status.code()))
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
}

#[async_trait::async_trait]
impl PackageManagerBackend for ZypperBackend {
    fn name(&self) -> &str {
        "Zypper"
    }

    fn is_available(&self) -> bool {
        has_binary("zypper")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = match self.run_capture("zypper", &["--non-interactive", "search", query]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for line in output.lines() {
            if line.contains("Name") && line.contains("Summary") {
                continue;
            }
            if line.starts_with("---") || line.is_empty() || line.starts_with("Loading repository") {
                continue;
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let pkg_type = parts[3].trim();
                if pkg_type.eq_ignore_ascii_case("package") {
                    results.push(SearchResult {
                        name: parts[1].trim().to_string(),
                        description: parts[2].trim().to_string(),
                        version: "repo".to_string(),
                        backend: self.name().to_string(),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_privileged("zypper", &["--non-interactive", "install", "-y", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("zypper", &["--non-interactive", "remove", "-y", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("zypper", &["--non-interactive", "refresh"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let output = self.run_capture("zypper", &["info", package]).await?;
        
        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut homepage = None;
        let mut size = None;

        let mut parsing_desc = false;

        for line in output.lines() {
            if parsing_desc {
                if line.starts_with(' ') {
                    description.push('\n');
                    description.push_str(line.trim());
                    continue;
                } else {
                    parsing_desc = false;
                }
            }

            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let val = val.trim();

                match key {
                    "Version" => version = val.to_string(),
                    "URL" => homepage = Some(val.to_string()),
                    "Installed Size" => size = Some(val.to_string()),
                    "Description" => {
                        parsing_desc = true;
                    }
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
        // Zypper search --installed-only lista pacotes instalados
        let output = self.run_capture("zypper", &["--non-interactive", "search", "--installed-only"]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            if line.contains("Name") && line.contains("Summary") {
                continue;
            }
            if line.starts_with("---") || line.is_empty() || line.starts_with("Loading repository") {
                continue;
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let pkg_type = parts[3].trim();
                if pkg_type.eq_ignore_ascii_case("package") {
                    results.push(InstalledPackage {
                        name: parts[1].trim().to_string(),
                        version: "installed".to_string(),
                        backend: self.name().to_string(),
                    });
                }
            }
        }

        Ok(results)
    }
}
