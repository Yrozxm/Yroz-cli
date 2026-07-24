use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct EopkgBackend;

impl EopkgBackend {
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
            .map_err(|e| format!("Falha ao iniciar o eopkg (com sudo): {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Eopkg falhou com código: {:?}", status.code()))
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
        let mut cmd = Command::new("eopkg");
        cmd.arg("info").arg(package);
        if let Ok(output) = cmd.output().await {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains("Package state: installed") || stdout.contains("Estado do pacote: instalado")
        } else {
            false
        }
    }
}

#[async_trait::async_trait]
impl PackageManagerBackend for EopkgBackend {
    fn name(&self) -> &str {
        "eopkg"
    }

    fn is_available(&self) -> bool {
        has_binary("eopkg")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = match self.run_capture("eopkg", &["search", query]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for line in output.lines() {
            if let Some((name_part, desc_part)) = line.split_once(" - ") {
                results.push(SearchResult {
                    name: name_part.trim().to_string(),
                    description: desc_part.trim().to_string(),
                    version: "repo".to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        self.run_privileged("eopkg", &["install", "-y", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("eopkg", &["remove", "-y", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("eopkg", &["update-repo"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let output = self.run_capture("eopkg", &["info", package]).await?;
        
        let mut version = "unknown".to_string();
        let mut description = String::new();
        let mut homepage = None;
        let mut size = None;

        for line in output.lines() {
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let val = val.trim();

                match key {
                    "Name" | "Nome" => {}
                    "Version" | "Versão" => version = val.to_string(),
                    "Homepage" | "Página inicial" => homepage = Some(val.to_string()),
                    "Installed size" | "Tamanho instalado" => size = Some(val.to_string()),
                    "Description" | "Descrição" => description = val.to_string(),
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
        let output = self.run_capture("eopkg", &["list-installed"]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            if let Some((name_part, _)) = line.split_once(" - ") {
                results.push(InstalledPackage {
                    name: name_part.trim().to_string(),
                    version: "installed".to_string(),
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }
}
