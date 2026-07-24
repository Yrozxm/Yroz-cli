use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct YayBackend;

impl YayBackend {
    pub fn new() -> Self {
        Self
    }

    async fn run_user_interactive(&self, program: &str, args: &[&str]) -> Result<(), String> {
        // yay NÃO deve ser executado como root/sudo no nível do processo principal,
        // ele mesmo chamará o sudo quando necessário para pacman.
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status().await
            .map_err(|e| format!("Falha ao iniciar o yay: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Yay falhou com código: {:?}", status.code()))
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
        let mut cmd = Command::new("pacman");
        cmd.arg("-Q").arg(package);
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
impl PackageManagerBackend for YayBackend {
    fn name(&self) -> &str {
        "AUR/yay"
    }

    fn is_available(&self) -> bool {
        has_binary("yay") && has_binary("pacman")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        // yay -Ssa <query> busca apenas no AUR
        let output = match self.run_capture("yay", &["-Ssa", query]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        let mut lines_iter = output.lines();

        while let Some(first_line) = lines_iter.next() {
            let first_line = first_line.trim();
            if first_line.is_empty() {
                continue;
            }

            let second_line = lines_iter.next().unwrap_or("").trim();
            let parts: Vec<&str> = first_line.split_whitespace().collect();

            if parts.len() >= 2 {
                let repo_and_name = parts[0];
                let version = parts[1].to_string();

                // Garante que é do AUR (começa com aur/)
                if repo_and_name.starts_with("aur/") {
                    let name = repo_and_name["aur/".len()..].to_string();
                    results.push(SearchResult {
                        name,
                        description: second_line.to_string(),
                        version,
                        backend: self.name().to_string(),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_user_interactive("yay", &["-S", "--noconfirm", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_user_interactive("yay", &["-Rns", "--noconfirm", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_user_interactive("yay", &["-Sua", "--noconfirm"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let output = self.run_capture("yay", &["-Si", package]).await?;
        
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
                    "Download Size" | "First Submitted" => {
                        // yay pode não trazer tamanho, mas traz dados do AUR
                        size = Some(val.to_string());
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
        // pacman -Qm lista pacotes estrangeiros instalados (ex: compilados do AUR)
        let output = self.run_capture("pacman", &["-Qm"]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                results.push(InstalledPackage {
                    name: tokens[0].to_string(),
                    version: tokens[1].to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }
}
