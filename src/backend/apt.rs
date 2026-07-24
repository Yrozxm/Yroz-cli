use crate::backend::{
    InstalledPackage, PackageInfo, PackageManagerBackend, SearchResult, has_binary,
};
use std::process::Stdio;
use tokio::process::Command;

pub struct AptBackend;

impl AptBackend {
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

        let status = cmd
            .status()
            .await
            .map_err(|e| format!("Falha ao iniciar o comando: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("O comando falhou com código: {:?}", status.code()))
        }
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

    async fn is_package_installed(&self, package: &str) -> bool {
        let mut cmd = Command::new("dpkg");
        cmd.arg("-s").arg(package);
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
impl PackageManagerBackend for AptBackend {
    fn name(&self) -> &str {
        "APT"
    }

    fn is_available(&self) -> bool {
        has_binary("apt-get") && has_binary("dpkg")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = self.run_capture("apt-cache", &["search", query]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            if let Some((name, desc)) = line.split_once(" - ") {
                results.push(SearchResult {
                    name: name.trim().to_string(),
                    description: desc.trim().to_string(),
                    version: "repo".to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_privileged("apt-get", &["install", "-y", package])
            .await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("apt-get", &["remove", "-y", package])
            .await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("apt-get", &["update"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let output = self.run_capture("apt-cache", &["show", package]).await?;

        let first_block = output.split("\n\n").next().unwrap_or(&output);

        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut homepage = None;
        let mut size = None;

        let mut parsing_desc = false;

        for line in first_block.lines() {
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
                    "Homepage" => homepage = Some(val.to_string()),
                    "Size" => {
                        let bytes: u64 = val.parse().unwrap_or(0);
                        size = Some(if bytes > 1024 * 1024 {
                            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
                        } else if bytes > 1024 {
                            format!("{:.2} KB", bytes as f64 / 1024.0)
                        } else {
                            format!("{} B", bytes)
                        });
                    }
                    "Description-en" | "Description" => {
                        description = val.to_string();
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
        let output = self
            .run_capture("dpkg-query", &["-W", "-f=${Package}\t${Version}\n"])
            .await?;
        let mut results = Vec::new();

        for line in output.lines() {
            if let Some((name, ver)) = line.split_once('\t') {
                results.push(InstalledPackage {
                    name: name.to_string(),
                    version: ver.to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }
}
