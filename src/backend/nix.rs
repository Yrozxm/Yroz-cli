use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage, has_binary};
use std::process::Stdio;
use tokio::process::Command;

pub struct NixBackend;

impl NixBackend {
    pub fn new() -> Self {
        Self
    }

    async fn run_user_interactive(&self, program: &str, args: &[&str]) -> Result<(), String> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status().await
            .map_err(|e| format!("Falha ao iniciar o comando nix: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Nix falhou com código: {:?}", status.code()))
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

    async fn is_package_installed(&self, package_name: &str) -> bool {
        // nix-env -q lista pacotes instalados
        if let Ok(output) = self.run_capture("nix-env", &["-q"]).await {
            let clean_name = package_name.strip_prefix("nixpkgs.").unwrap_or(package_name);
            for line in output.lines() {
                let (name, _) = self.split_name_version(line.trim());
                if name.eq_ignore_ascii_case(clean_name) {
                    return true;
                }
            }
        }
        false
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
impl PackageManagerBackend for NixBackend {
    fn name(&self) -> &str {
        "Nix"
    }

    fn is_available(&self) -> bool {
        has_binary("nix-env")
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        // nix-env -qaP '<query>' busca pacotes e mostra o caminho de atributo (ex: nixpkgs.nano)
        // O argumento do query deve conter asteriscos para busca parcial ou ser envolvido
        let filter = format!(".*{}.*", query);
        let output = match self.run_capture("nix-env", &["-qaP", filter.as_str()]).await {
            Ok(out) => out,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for line in output.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                let attr_path = tokens[0].to_string();
                let pkg_ver = tokens[1];
                let (friendly_name, version) = self.split_name_version(pkg_ver);

                results.push(SearchResult {
                    name: attr_path,
                    description: format!("Nix package: {}", friendly_name),
                    version,
                    backend: self.name().to_string(),
                });
            }
        }

        Ok(results)
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        // Instala usando o caminho do atributo para maior velocidade e precisão (ex: nix-env -iA nixpkgs.nano)
        let attr = if package.starts_with("nixpkgs.") {
            package.to_string()
        } else {
            format!("nixpkgs.{}", package)
        };
        self.run_user_interactive("nix-env", &["-iA", &attr]).await
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        // nix-env -e precisa do nome do pacote limpo (sem nixpkgs.)
        let clean_name = package.strip_prefix("nixpkgs.").unwrap_or(package);
        self.run_user_interactive("nix-env", &["-e", clean_name]).await
    }

    async fn update(&self) -> Result<(), String> {
        self.run_user_interactive("nix-channel", &["--update"]).await
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let clean_name = package.strip_prefix("nixpkgs.").unwrap_or(package);
        
        let output = self.run_capture("nix-env", &["-qaP", "--description", clean_name]).await?;
        
        let mut version = "unknown".to_string();
        let mut description = format!("Nix package {}", clean_name);

        for line in output.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                let pkg_ver = tokens[1];
                let (_, ver) = self.split_name_version(pkg_ver);
                version = ver;
                
                // Se houver mais campos de descrição, extrai
                if tokens.len() >= 3 {
                    description = tokens[2..].join(" ");
                }
                break;
            }
        }

        let installed = self.is_package_installed(package).await;

        Ok(PackageInfo {
            name: package.to_string(),
            version,
            description,
            backend: self.name().to_string(),
            homepage: Some("https://search.nixos.org/packages".to_string()),
            size: None,
            installed,
        })
    }

    async fn list(&self) -> Result<Vec<InstalledPackage>, String> {
        let output = self.run_capture("nix-env", &["-q"]).await?;
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
