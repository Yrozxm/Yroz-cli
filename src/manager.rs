use crate::backend::apt::AptBackend;
use crate::backend::flatpak::FlatpakBackend;
use crate::backend::snap::SnapBackend;
use crate::backend::pacman::PacmanBackend;
use crate::backend::dnf::DnfBackend;
use crate::backend::portage::PortageBackend;
use crate::backend::xbps::XbpsBackend;
use crate::backend::apk::ApkBackend;
use crate::backend::yay::YayBackend;
use crate::backend::nix::NixBackend;
use crate::backend::zypper::ZypperBackend;
use crate::backend::appimage::AppImageBackend;
use crate::backend::eopkg::EopkgBackend;
use crate::backend::napt::NaptBackend;
use crate::backend::{PackageManagerBackend, SearchResult};
use crate::config::YrozConfig;

use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};

pub struct PackageManager {
    backends: Vec<Box<dyn PackageManagerBackend>>,
}

impl PackageManager {
    pub fn new() -> Self {
        let config = YrozConfig::load();
        let is_disabled = |name: &str| {
            config.disabled_backends.iter().any(|b| b.eq_ignore_ascii_case(name))
        };

        let mut backends: Vec<Box<dyn PackageManagerBackend>> = Vec::new();

        let napt = NaptBackend::new();
        if napt.is_available() && !is_disabled("NAPT") {
            backends.push(Box::new(napt));
        }

        let apt = AptBackend::new();
        if apt.is_available() && !is_disabled("APT") {
            backends.push(Box::new(apt));
        }

        let pacman = PacmanBackend::new();
        if pacman.is_available() && !is_disabled("Pacman") {
            backends.push(Box::new(pacman));
        }

        let dnf = DnfBackend::new();
        if dnf.is_available() && !is_disabled("DNF") {
            backends.push(Box::new(dnf));
        }

        let portage = PortageBackend::new();
        if portage.is_available() && !is_disabled("Portage") {
            backends.push(Box::new(portage));
        }

        let xbps = XbpsBackend::new();
        if xbps.is_available() && !is_disabled("XBPS") {
            backends.push(Box::new(xbps));
        }

        let zypper = ZypperBackend::new();
        if zypper.is_available() && !is_disabled("Zypper") {
            backends.push(Box::new(zypper));
        }

        let apk = ApkBackend::new();
        if apk.is_available() && !is_disabled("APK") {
            backends.push(Box::new(apk));
        }

        let eopkg = EopkgBackend::new();
        if eopkg.is_available() && !is_disabled("eopkg") {
            backends.push(Box::new(eopkg));
        }

        let yay = YayBackend::new();
        if yay.is_available() && !is_disabled("AUR/yay") && !is_disabled("yay") {
            backends.push(Box::new(yay));
        }

        let nix = NixBackend::new();
        if nix.is_available() && !is_disabled("Nix") {
            backends.push(Box::new(nix));
        }

        let flatpak = FlatpakBackend::new();
        if flatpak.is_available() && !is_disabled("Flatpak") {
            backends.push(Box::new(flatpak));
        }

        let snap = SnapBackend::new();
        if snap.is_available() && !is_disabled("Snap") {
            backends.push(Box::new(snap));
        }

        let appimage = AppImageBackend::new();
        if appimage.is_available() && !is_disabled("AppImage") {
            backends.push(Box::new(appimage));
        }

        Self { backends }
    }

    pub fn get_available_backend_names(&self) -> Vec<String> {
        self.backends.iter().map(|b| b.name().to_string()).collect()
    }

    pub async fn search(&self, query: &str) -> Vec<SearchResult> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(format!(
            "A pesquisar por '{}' em todos os gerenciadores...",
            query
        ));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        let mut handles = Vec::new();

        for backend in &self.backends {
            let b_name = backend.name().to_string();
            let query = query.to_string();

            let backend_ref = backend.as_ref();
            handles.push(async move { (b_name, backend_ref.search(&query).await) });
        }

        let results_raw = futures_util::future::join_all(handles).await;
        pb.finish_and_clear();

        let mut all_results = Vec::new();
        for (b_name, res) in results_raw {
            match res {
                Ok(mut list) => all_results.append(&mut list),
                Err(e) => {
                    eprintln!(
                        "{} Erro ao buscar no gerenciador {}: {}",
                        "".yellow(),
                        b_name.bold(),
                        e.red()
                    );
                }
            }
        }

        all_results
    }

    pub async fn update_all(&self) -> Result<(), String> {
        for backend in &self.backends {
            println!(
                "{} Atualizando a base de dados do {}...",
                "".cyan(),
                backend.name().bold()
            );
            if let Err(e) = backend.update().await {
                eprintln!(
                    "{} Falha ao atualizar {}: {}",
                    "".red(),
                    backend.name().bold(),
                    e.red()
                );
            } else {
                println!(
                    "{} {} atualizado com sucesso!",
                    "".green(),
                    backend.name().bold()
                );
            }
        }
        Ok(())
    }

    pub async fn install(&self, package: &str) -> Result<(), String> {
        let config = YrozConfig::load();

        if package.to_lowercase().ends_with(".appimage") {
            if let Some(appimage) = self.find_backend("AppImage") {
                return appimage.install(package).await;
            } else {
                return Err("Backend do AppImage não está disponível ou foi desativado.".to_string());
            }
        }

        if package.contains('.') {
            if let Some(flatpak) = self.find_backend("Flatpak") {
                println!(
                    "{} Detectado possível ID de Flatpak. Verificando...",
                    "".blue()
                );
                if flatpak.info(package).await.is_ok() {
                    println!(
                        "{} Encontrado no {}! Iniciando instalação...",
                        "".green(),
                        "Flatpak".bold()
                    );
                    return flatpak.install(package).await;
                }
            }
        }

        let priority_order = if !config.priority.is_empty() {
            config.priority.clone()
        } else {
            vec![
                "NAPT".to_string(),
                "APT".to_string(),
                "Pacman".to_string(),
                "DNF".to_string(),
                "Portage".to_string(),
                "XBPS".to_string(),
                "APK".to_string(),
                "eopkg".to_string(),
                "AUR/yay".to_string(),
                "Nix".to_string(),
                "Flatpak".to_string(),
                "Snap".to_string(),
            ]
        };

        for name in &priority_order {
            if let Some(backend) = self.find_backend(name) {
                if name.eq_ignore_ascii_case("Flatpak") && package.contains('.') {
                    continue;
                }

                println!(
                    "{} Verificando disponibilidade em {}...",
                    "".blue(),
                    backend.name()
                );
                if backend.info(package).await.is_ok() {
                    println!(
                        "{} Encontrado no {}! Iniciando instalação...",
                        "".green(),
                        backend.name().bold()
                    );
                    return backend.install(package).await;
                }
            }
        }

        println!(
            "{} Pacote '{}' não encontrado diretamente. Pesquisando alternativas...",
            "".yellow(),
            package
        );
        let search_results = self.search(package).await;

        if search_results.is_empty() {
            return Err(format!(
                "Não foi possível encontrar o pacote '{}' em nenhum gerenciador.",
                package
            ));
        }

        println!("\n{} Pacotes correspondentes encontrados:", "".cyan());
        for (i, res) in search_results.iter().enumerate() {
            println!(
                "  [{}] {} - {} ({}) [{}]",
                (i + 1).to_string().green().bold(),
                res.name.bold(),
                res.description,
                res.version,
                res.backend.blue()
            );
        }

        print!("\nEscolha o número do pacote a instalar (ou pressione Enter para cancelar): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.is_empty() {
            println!("Operação cancelada pelo usuário.");
            return Ok(());
        }

        if let Ok(idx) = input.parse::<usize>() {
            if idx > 0 && idx <= search_results.len() {
                let chosen = &search_results[idx - 1];
                println!(
                    "{} Instalando {} via {}...",
                    "".green(),
                    chosen.name.bold(),
                    chosen.backend.bold()
                );
                if let Some(b) = self.find_backend(&chosen.backend) {
                    return b.install(&chosen.name).await;
                }
            }
        }

        Err("Opção inválida de escolha.".to_string())
    }

    pub async fn remove(&self, package: &str) -> Result<(), String> {
        let mut installed_backends = Vec::new();

        for backend in &self.backends {
            if let Ok(info) = backend.info(package).await {
                if info.installed {
                    installed_backends.push(backend.name().to_string());
                }
            }
        }

        if installed_backends.is_empty() {
            return Err(format!(
                "O pacote '{}' não parece estar instalado em nenhum gerenciador gerenciado pelo Yroz.",
                package
            ));
        }

        let backend_name = if installed_backends.len() == 1 {
            installed_backends[0].clone()
        } else {
            println!(
                "{} O pacote '{}' está instalado em múltiplos gerenciadores:",
                "".yellow(),
                package
            );
            for (i, name) in installed_backends.iter().enumerate() {
                println!("  [{}] {}", (i + 1).to_string().green(), name);
            }
            print!("Selecione qual deseja remover (ou Enter para cancelar): ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim();

            if input.is_empty() {
                println!("Remoção cancelada.");
                return Ok(());
            }

            if let Ok(idx) = input.parse::<usize>() {
                if idx > 0 && idx <= installed_backends.len() {
                    installed_backends[idx - 1].clone()
                } else {
                    return Err("Escolha inválida.".to_string());
                }
            } else {
                return Err("Escolha inválida.".to_string());
            }
        };

        if let Some(backend) = self.find_backend(&backend_name) {
            println!(
                "{} Removendo {} de {}...",
                "".red(),
                package.bold(),
                backend_name.bold()
            );
            backend.remove(package).await
        } else {
            Err("Erro ao resolver gerenciador de destino.".to_string())
        }
    }

    pub async fn info(&self, package: &str) -> Result<(), String> {
        let mut found = false;

        for backend in &self.backends {
            if let Ok(info) = backend.info(package).await {
                found = true;
                println!(
                    "\n{} Informações do Pacote no {}:",
                    "".blue(),
                    backend.name().bold()
                );
                println!("  {} {}", "Nome:".bold(), info.name);
                println!("  {} {}", "Versão:".bold(), info.version);
                println!(
                    "  {} {}",
                    "Instalado:".bold(),
                    if info.installed {
                        "Sim".green().bold()
                    } else {
                        "Não".red()
                    }
                );
                if let Some(ref sz) = info.size {
                    println!("  {} {}", "Tamanho:".bold(), sz);
                }
                if let Some(ref hp) = info.homepage {
                    println!("  {} {}", "Homepage:".bold(), hp);
                }
                println!("  {} {}", "Descrição:".bold(), info.description);
            }
        }

        if !found {
            return Err(format!(
                "Não foi possível obter informações para '{}' em nenhum gerenciador.",
                package
            ));
        }

        Ok(())
    }

    pub async fn list_installed(&self) -> Result<(), String> {
        for backend in &self.backends {
            println!(
                "\n{} Pacotes instalados via {}:",
                "".blue(),
                backend.name().bold()
            );
            match backend.list().await {
                Ok(list) => {
                    if list.is_empty() {
                        println!("  (nenhum pacote encontrado)");
                    } else {
                        let show_count = std::cmp::min(100, list.len());
                        for pkg in &list[0..show_count] {
                            println!("  {} ({})", pkg.name, pkg.version);
                        }
                        if list.len() > 100 {
                            println!("  ... e mais {} pacotes.", list.len() - 100);
                        }
                    }
                }
                Err(e) => eprintln!("  Erro ao listar pacotes: {}", e.red()),
            }
        }
        Ok(())
    }

    fn find_backend(&self, name: &str) -> Option<&Box<dyn PackageManagerBackend>> {
        self.backends
            .iter()
            .find(|b| b.name().eq_ignore_ascii_case(name))
    }

    pub async fn status(&self) -> Result<(), String> {
        let config = YrozConfig::load();
        let is_disabled = |name: &str| {
            config.disabled_backends.iter().any(|b| b.eq_ignore_ascii_case(name))
        };

        println!("Estado dos Gerenciadores de Pacotes:\n");
        
        let all_backends: Vec<(String, bool)> = vec![
            ("NAPT".to_string(), NaptBackend::new().is_available()),
            ("APT".to_string(), AptBackend::new().is_available()),
            ("Pacman".to_string(), PacmanBackend::new().is_available()),
            ("DNF".to_string(), DnfBackend::new().is_available()),
            ("Portage".to_string(), PortageBackend::new().is_available()),
            ("XBPS".to_string(), XbpsBackend::new().is_available()),
            ("Zypper".to_string(), ZypperBackend::new().is_available()),
            ("APK".to_string(), ApkBackend::new().is_available()),
            ("eopkg".to_string(), EopkgBackend::new().is_available()),
            ("AUR/yay".to_string(), YayBackend::new().is_available()),
            ("Nix".to_string(), NixBackend::new().is_available()),
            ("Flatpak".to_string(), FlatpakBackend::new().is_available()),
            ("Snap".to_string(), SnapBackend::new().is_available()),
            ("AppImage".to_string(), AppImageBackend::new().is_available()),
        ];

        for (name, available) in all_backends {
            let status_str = if is_disabled(&name) {
                "Desativado (config)".yellow().bold().to_string()
            } else if available {
                "Ativo".green().bold().to_string()
            } else {
                "Não disponível".dimmed().to_string()
            };
            
            println!("  {:<12} : [{}]", name.bold(), status_str);
        }
        
        Ok(())
    }
}
