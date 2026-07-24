use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct PortageBackend;

impl PortageBackend {
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
            .map_err(|e| format!("Falha ao iniciar o emerge (com sudo): {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Emerge falhou com código: {:?}", status.code()))
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
        // No Gentoo, verifica se há uma entrada em /var/db/pkg para o pacote
        if let Ok(entries) = std::fs::read_dir("/var/db/pkg") {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Ok(pkg_entries) = std::fs::read_dir(entry.path()) {
                        for pkg_entry in pkg_entries.flatten() {
                            let name_ver = pkg_entry.file_name().to_string_lossy().into_owned();
                            if name_ver.starts_with(&format!("{}-", package)) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

#[async_trait::async_trait]
impl PackageManagerBackend for PortageBackend {
    fn name(&self) -> &str {
        "Portage"
    }

    fn is_available(&self) -> bool {
        has_binary("emerge")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = match self.run_capture("emerge", &["-s", query]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        let mut current_name = None;
        let mut current_version = String::new();
        let mut current_desc = String::new();

        for line in output.lines() {
            if line.starts_with("*  ") {
                if let Some(name) = current_name.take() {
                    results.push(SearchResult {
                        name,
                        description: current_desc.clone(),
                        version: if current_version.is_empty() { "repo".to_string() } else { current_version.clone() },
                        backend: self.name().to_string(),
                    });
                    current_version.clear();
                    current_desc.clear();
                }

                let full_name = line["*  ".len()..].trim();
                let clean_name = if let Some((_, name_part)) = full_name.split_once('/') {
                    name_part.to_string()
                } else {
                    full_name.to_string()
                };
                current_name = Some(clean_name);
            } else if let Some(idx) = line.find("Latest version available:") {
                current_version = line[idx + "Latest version available:".len()..].trim().to_string();
            } else if let Some(idx) = line.find("Description:") {
                current_desc = line[idx + "Description:".len()..].trim().to_string();
            }
        }

        if let Some(name) = current_name {
            results.push(SearchResult {
                name,
                description: current_desc,
                version: if current_version.is_empty() { "repo".to_string() } else { current_version },
                backend: self.name().to_string(),
            });
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_privileged("emerge", &["--ask=n", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("emerge", &["--depclean", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("emerge", &["--sync"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        // Emerge -s retorna informações detalhadas suficientes
        let output = self.run_capture("emerge", &["-s", package]).await?;
        
        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut homepage = None;
        let mut size = None;

        for line in output.lines() {
            if let Some(idx) = line.find("Latest version available:") {
                version = line[idx + "Latest version available:".len()..].trim().to_string();
            } else if let Some(idx) = line.find("Description:") {
                description = line[idx + "Description:".len()..].trim().to_string();
            } else if let Some(idx) = line.find("Homepage:") {
                homepage = Some(line[idx + "Homepage:".len()..].trim().to_string());
            } else if let Some(idx) = line.find("Size of files:") {
                size = Some(line[idx + "Size of files:".len()..].trim().to_string());
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
        let mut results = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/var/db/pkg") {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Ok(pkg_entries) = std::fs::read_dir(entry.path()) {
                        for pkg_entry in pkg_entries.flatten() {
                            if pkg_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                let name_ver = pkg_entry.file_name().to_string_lossy().into_owned();
                                let parts: Vec<&str> = name_ver.split('-').collect();
                                if parts.len() >= 2 {
                                    let mut version_idx = parts.len() - 1;
                                    for (i, part) in parts.iter().enumerate() {
                                        if part.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                                            version_idx = i;
                                            break;
                                        }
                                    }
                                    let name = parts[0..version_idx].join("-");
                                    let version = parts[version_idx..].join("-");
                                    
                                    results.push(InstalledPackage {
                                        name,
                                        version,
                                        backend: self.name().to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(results)
    }
}
