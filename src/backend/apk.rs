use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct ApkBackend;

impl ApkBackend {
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
            .map_err(|e| format!("Falha ao iniciar o apk (com sudo): {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Apk falhou com código: {:?}", status.code()))
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
        let mut cmd = Command::new("apk");
        cmd.arg("info").arg("-e").arg(package);
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
            (pkg_ver.to_string(), "repo".to_string())
        }
    }
}

#[async_trait::async_trait]
impl PackageManagerBackend for ApkBackend {
    fn name(&self) -> &str {
        "APK"
    }

    fn is_available(&self) -> bool {
        has_binary("apk")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let output = match self.run_capture("apk", &["search", "-v", query]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for line in output.lines() {
            if let Some((pkg_ver, desc)) = line.split_once(" - ") {
                let (name, version) = self.split_name_version(pkg_ver.trim());
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
        self.run_privileged("apk", &["add", package]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        self.run_privileged("apk", &["del", package]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_privileged("apk", &["update"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        // apk info -d <package> pega a descrição do pacote
        let desc_output = self.run_capture("apk", &["info", "-d", package]).await.unwrap_or(String::new());
        let description = desc_output.lines().skip(1).collect::<Vec<&str>>().join("\n").trim().to_string();

        // apk info -w <package> pega o website/homepage
        let web_output = self.run_capture("apk", &["info", "-w", package]).await.ok();
        let homepage = web_output.map(|w| w.lines().skip(1).collect::<Vec<&str>>().join("").trim().to_string());

        // apk info -s <package> pega o tamanho
        let size_output = self.run_capture("apk", &["info", "-s", package]).await.ok();
        let size = size_output.map(|s| s.lines().skip(1).collect::<Vec<&str>>().join("").trim().to_string());

        // apk info <package> pega a versão na primeira linha
        let info_output = self.run_capture("apk", &["info", package]).await?;
        let first_line = info_output.lines().next().unwrap_or(package);
        let (_, version) = self.split_name_version(first_line);

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
        let output = self.run_capture("apk", &["info", "-v"]).await?;
        let mut results = Vec::new();

        for line in output.lines() {
            let (name, version) = self.split_name_version(line.trim());
            results.push(InstalledPackage {
                name,
                version,
                backend: self.name().to_string(),
            });
        }

        Ok(results)
    }
}
