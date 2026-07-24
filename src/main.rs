pub mod backend;
pub mod manager;
pub mod config;

use clap::{Parser, Subcommand};
use colored::*;
use manager::PackageManager;

#[derive(Parser)]
#[command(name = "yroz")]
#[command(author = "Yroz Developers")]
#[command(version = "0.1.0")]
#[command(about = "Gestor universal de software para Linux", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pesquisa pacotes em todos os gerenciadores disponíveis
    Search {
        /// Termo de pesquisa
        query: String,
    },
    /// Instala um pacote (prioriza nativo, com fallback para Flatpak/Snap)
    Install {
        /// Nome do pacote ou ID de aplicação
        package: String,
    },
    /// Remove um pacote instalado
    Remove {
        /// Nome do pacote ou ID de aplicação a remover
        package: String,
    },
    /// Atualiza as fontes de pacotes de todos os gerenciadores
    Update,
    /// Mostra detalhes sobre um pacote
    Info {
        /// Nome do pacote ou ID de aplicação
        package: String,
    },
    /// Lista os pacotes instalados gerenciados pelo Yroz
    List,
    /// Atualiza o próprio executável do Yroz para a versão mais recente
    SelfUpdate,
    /// Mostra o estado de suporte e disponibilidade de todos os gerenciadores
    Status,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Executa auto-atualização sem inicializar os gerenciadores se for o comando self-update
    if let Commands::SelfUpdate = cli.command {
        if let Err(err) = run_self_update().await {
            eprintln!("\n{} Erro: {}", "".red(), err.red());
            std::process::exit(1);
        }
        return;
    }

    let manager = PackageManager::new();
    let active_backends = manager.get_available_backend_names();
    if active_backends.is_empty() {
        eprintln!(
            "{} Erro: Nenhum gerenciador de pacotes compatível foi encontrado no sistema.",
            "".red()
        );
        std::process::exit(1);
    }

    let result = match cli.command {
        Commands::Search { query } => {
            let results = manager.search(&query).await;
            if results.is_empty() {
                println!("Nenhum resultado encontrado para '{}'.", query);
            } else {
                println!("{} Resultados encontrados:", "".blue());
                for res in results {
                    println!(
                        "  • {} - {} ({}) [{}]",
                        res.name.bold(),
                        res.description,
                        res.version.dimmed(),
                        res.backend.green()
                    );
                }
            }
            Ok(())
        }
        Commands::Install { package } => manager.install(&package).await,
        Commands::Remove { package } => manager.remove(&package).await,
        Commands::Update => manager.update_all().await,
        Commands::Info { package } => manager.info(&package).await,
        Commands::List => manager.list_installed().await,
        Commands::Status => manager.status().await,
        Commands::SelfUpdate => unreachable!(), // Tratado acima
    };

    if let Err(err) = result {
        eprintln!("\n{} Erro: {}", "".red(), err.red());
        std::process::exit(1);
    }
}

async fn run_self_update() -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Falha ao obter o caminho do executável atual: {}", e))?;

    println!("Verificando atualizações no GitHub...");

    let repo = "yrozxm/Yroz-cli";
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let output = std::process::Command::new("curl")
        .args(&["-s", "-H", "User-Agent: yroz-cli", &api_url])
        .output()
        .map_err(|e| format!("Falha ao executar curl: {}", e))?;

    if !output.status.success() {
        return Err("Falha ao consultar a API do GitHub.".to_string());
    }

    let response = String::from_utf8_lossy(&output.stdout);
    
    let mut download_url = None;
    for line in response.lines() {
        if line.contains("browser_download_url") {
            if let Some(idx) = line.find("https://") {
                let url = line[idx..].trim_matches(|c| c == '"' || c == ',' || c == ' ');
                download_url = Some(url.to_string());
                break;
            }
        }
    }

    let url = match download_url {
        Some(u) => u,
        None => {
            return Err("Nenhuma release pública ou binário disponível encontrado no GitHub para auto-atualização.".to_string());
        }
    };

    println!("Baixando nova versão de {}...", url);
    
    let temp_file = current_exe.with_extension("tmp");
    let dl_status = std::process::Command::new("curl")
        .args(&["-L", "-o", temp_file.to_str().unwrap(), &url])
        .status()
        .map_err(|e| format!("Falha ao baixar o arquivo: {}", e))?;

    if !dl_status.success() {
        return Err("Falha no download da nova versão.".to_string());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&temp_file) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&temp_file, perms);
        }
    }

    let old_file = current_exe.with_extension("old");
    let _ = std::fs::remove_file(&old_file);
    
    std::fs::rename(&current_exe, &old_file)
        .map_err(|e| format!("Falha ao renomear executável atual: {}", e))?;
        
    if let Err(e) = std::fs::rename(&temp_file, &current_exe) {
        let _ = std::fs::rename(&old_file, &current_exe);
        return Err(format!("Falha ao substituir pelo novo executável: {}", e));
    }

    let _ = std::fs::remove_file(&old_file);
    println!("Atualização concluída com sucesso!");
    Ok(())
}
