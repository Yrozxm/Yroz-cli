use crate::backend::{
    InstalledPackage, PackageInfo, PackageManagerBackend, SearchResult, has_binary,
};
use std::process::Stdio;
use tokio::process::Command;

pub struct SnapBackend;

impl SnapBackend {
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
            .map_err(|e| format!("Falha ao iniciar o comando snap (com sudo): {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Snap falhou com código: {:?}", status.code()))
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
}

#[async_trait::async_trait]
impl PackageManagerBackend for SnapBackend {
    fn name(&self) -> &str {
        "Snap"
    }

    fn is_available(&self) -> bool {
        has_binary("snap")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = self.run_capture("snap", &["search", query]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 4 {
                let name = tokens[0];

                if name.eq_ignore_ascii_case("name") || name.eq_ignore_ascii_case("nome") {
                    continue;
                }

                let version = tokens[1].to_string();
                let _publisher = tokens[2];
                let _notes = tokens[3];

                let summary_part = if let Some(idx) = line.find(tokens[2]) {
                    if let Some(sub_idx) = line[idx + tokens[2].len()..].find(tokens[3]) {
                        line[idx + tokens[2].len() + sub_idx + tokens[3].len()..].trim()
                    } else {
                        ""
                    }
                } else {
                    ""
                };

                results.push(SearchResult {
                    name: name.to_string(),
                    description: summary_part.to_string(),
                    version,
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_privileged("snap", &["install", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("snap", &["remove", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("snap", &["refresh"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let output = self.run_capture("snap", &["info", package]).await?;

        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut homepage = None;
        let mut size = None;
        let mut installed = false;

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
                    "summary" => description = val.to_string(),
                    "store-url" => homepage = Some(val.to_string()),
                    "description" => {
                        parsing_desc = true;
                    }
                    "installed" => {
                        installed = true;

                        let parts: Vec<&str> = val.split_whitespace().collect();
                        if !parts.is_empty() {
                            version = parts[0].to_string();
                        }
                        if parts.len() >= 3 {
                            size = Some(parts[2].to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        if !installed {
            for line in output.lines() {
                if line.contains("latest/stable:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        version = parts[1].to_string();
                    }
                }
            }
        }

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
        let output = self.run_capture("snap", &["list"]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                let name = tokens[0];
                if name.eq_ignore_ascii_case("name") || name.eq_ignore_ascii_case("nome") {
                    continue;
                }
                results.push(InstalledPackage {
                    name: name.to_string(),
                    version: tokens[1].to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }
}
