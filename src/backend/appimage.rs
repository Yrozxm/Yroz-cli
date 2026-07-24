use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use colored::*;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct AppImageFeed {
    items: Vec<AppImageItem>,
}

#[derive(Deserialize, Debug)]
struct AppImageItem {
    name: String,
    links: Option<Vec<AppImageLink>>,
}

#[derive(Deserialize, Debug)]
struct AppImageLink {
    #[serde(rename = "type")]
    link_type: String,
    url: String,
}

#[derive(Deserialize, Debug)]
struct GitHubRelease {
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize, Debug)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

pub struct AppImageBackend;

impl AppImageBackend {
    pub fn new() -> Self {
        Self
    }

    fn get_home_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/yrozxm".to_string());
        PathBuf::from(home)
    }

    fn get_applications_dir(&self) -> PathBuf {
        let dir = self.get_home_dir().join("Applications");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir
    }

    fn get_desktop_dir(&self) -> PathBuf {
        let home_path = self.get_home_dir();
        let config_file = home_path.join(".config/user-dirs.dirs");
        if config_file.exists() {
            if let Ok(content) = fs::read_to_string(config_file) {
                for line in content.lines() {
                    if line.starts_with("XDG_DESKTOP_DIR=") {
                        let path_val = line["XDG_DESKTOP_DIR=".len()..].trim_matches('"');
                        let resolved = path_val.replace("$HOME", home_path.to_str().unwrap_or(""));
                        let path = PathBuf::from(resolved);
                        if path.exists() {
                            return path;
                        }
                    }
                }
            }
        }
        
        let fallbacks = ["Desktop", "Área de Trabalho", "Escritorio", "Bureau", "Schreibtisch"];
        for folder in &fallbacks {
            let path = home_path.join(folder);
            if path.exists() {
                return path;
            }
        }
        
        home_path.join("Desktop")
    }

    fn sanitize_app_name(&self, filename: &str) -> String {
        let mut name = filename.strip_suffix(".AppImage")
            .or_else(|| filename.strip_suffix(".appimage"))
            .unwrap_or(filename);
        let name_lower = name.to_lowercase();
        
        let archs = ["-x86_64", "_x86_64", "-i386", "-i686", "-aarch64", "-armhf", "-arm64"];
        for arch in &archs {
            if name_lower.contains(arch) {
                if let Some(idx) = name_lower.find(arch) {
                    name = &name[..idx];
                }
            }
        }
        
        if let Some(hyphen_idx) = name.rfind('-') {
            let suffix = &name[hyphen_idx + 1..];
            if suffix.starts_with(|c: char| c.is_ascii_digit()) || 
               (suffix.starts_with('v') && suffix[1..].starts_with(|c: char| c.is_ascii_digit())) {
                name = &name[..hyphen_idx];
            }
        }
        
        let mut c = name.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }

    fn create_desktop_shortcut(&self, app_name: &str, executable_path: &Path, target_dir: &Path) -> Result<(), String> {
        let file_name = format!("yroz-{}.desktop", app_name.to_lowercase());
        let desktop_file_path = target_dir.join(&file_name);
        
        let content = format!(
            "[Desktop Entry]\n\
            Type=Application\n\
            Name={}\n\
            Exec={}\n\
            Icon=utilities-terminal\n\
            Terminal=false\n\
            Categories=Utility;\n",
            app_name,
            executable_path.to_str().unwrap_or("")
        );
        
        fs::write(&desktop_file_path, content)
            .map_err(|e| format!("Falha ao criar atalho .desktop em {:?}: {}", desktop_file_path, e))?;
            
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&desktop_file_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&desktop_file_path, perms);
            }
        }
        
        Ok(())
    }

    async fn resolve_appimage_url(&self, package_name: &str) -> Result<String, String> {
        let clean_name = package_name.strip_suffix(".appimage")
            .or_else(|| package_name.strip_suffix(".AppImage"))
            .unwrap_or(package_name)
            .to_lowercase();

        println!("Pesquisando '{}' no catálogo do AppImage...", clean_name);
        
        let temp_feed_path = self.get_home_dir().join(".cache/yroz/feed.json");
        if let Some(parent) = temp_feed_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let feed_url = "https://appimage.github.io/feed.json";
        let status = Command::new("curl")
            .arg("-sL")
            .arg("-o")
            .arg(temp_feed_path.to_str().unwrap())
            .arg(feed_url)
            .status()
            .await
            .map_err(|e| format!("Falha ao baixar feed.json: {}", e))?;

        if !status.success() {
            return Err("Falha ao baixar o feed de AppImages do catálogo.".to_string());
        }

        let feed_content = fs::read_to_string(&temp_feed_path)
            .map_err(|e| format!("Erro ao ler feed.json: {}", e))?;

        let feed: AppImageFeed = serde_json::from_str(&feed_content)
            .map_err(|e| format!("Erro ao processar catálogo do AppImage: {}", e))?;

        let item = feed.items.iter().find(|i| i.name.to_lowercase() == clean_name);
        
        let item = match item {
            Some(i) => i,
            None => {
                let mut suggestions = Vec::new();
                for i in &feed.items {
                    let item_name_lower = i.name.to_lowercase();
                    if item_name_lower.contains(&clean_name) || 
                       (clean_name.len() >= 3 && item_name_lower.len() >= 3 && levenshtein(&clean_name, &item_name_lower) <= 2) {
                        suggestions.push(i.name.clone());
                    }
                }
                
                if !suggestions.is_empty() {
                    return Err(format!(
                        "AppImage '{}' não foi encontrado no catálogo. Você quis dizer: {}?",
                        package_name,
                        suggestions.join(", ").bold()
                    ));
                } else {
                    return Err(format!("AppImage '{}' não foi encontrado no catálogo.", package_name));
                }
            }
        };

        let mut github_repo = None;
        let mut direct_download = None;

        if let Some(links) = &item.links {
            for link in links {
                if link.link_type.eq_ignore_ascii_case("GitHub") {
                    github_repo = Some(link.url.clone());
                } else if link.link_type.eq_ignore_ascii_case("Download") {
                    if link.url.to_lowercase().ends_with(".appimage") {
                        direct_download = Some(link.url.clone());
                    } else if link.url.contains("github.com") && link.url.contains("/releases") {
                        let parts: Vec<&str> = link.url.split('/').collect();
                        if parts.len() >= 5 {
                            github_repo = Some(format!("{}/{}", parts[3], parts[4]));
                        }
                    }
                }
            }
        }

        if let Some(repo) = github_repo {
            println!("Localizado repositório GitHub: {}", repo);
            println!("Consultando a release estável mais recente no GitHub...");

            let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
            let temp_release_path = self.get_home_dir().join(".cache/yroz/release.json");
            
            let release_status = Command::new("curl")
                .arg("-sL")
                .arg("-H")
                .arg("User-Agent: yroz-cli")
                .arg("-o")
                .arg(temp_release_path.to_str().unwrap())
                .arg(&api_url)
                .status()
                .await
                .map_err(|e| format!("Falha ao consultar API do GitHub: {}", e))?;

            if !release_status.success() {
                return Err("Falha ao obter release do GitHub.".to_string());
            }

            let release_content = fs::read_to_string(&temp_release_path)
                .map_err(|e| format!("Erro ao ler release.json: {}", e))?;

            let release: GitHubRelease = serde_json::from_str(&release_content)
                .map_err(|_| "Nenhum arquivo de instalação pública foi encontrado para este AppImage no GitHub.".to_string())?;

            let host_arch = std::env::consts::ARCH;
            let mut matched_asset = None;
            
            for asset in &release.assets {
                let asset_name_lower = asset.name.to_lowercase();
                if asset_name_lower.ends_with(".appimage") {
                    if host_arch == "x86_64" && (asset_name_lower.contains("x86_64") || asset_name_lower.contains("amd64")) {
                        matched_asset = Some(asset.browser_download_url.clone());
                        break;
                    } else if host_arch == "aarch64" && (asset_name_lower.contains("aarch64") || asset_name_lower.contains("arm64")) {
                        matched_asset = Some(asset.browser_download_url.clone());
                        break;
                    }
                }
            }

            if matched_asset.is_none() {
                for asset in &release.assets {
                    if asset.name.to_lowercase().ends_with(".appimage") {
                        matched_asset = Some(asset.browser_download_url.clone());
                        break;
                    }
                }
            }

            if let Some(url) = matched_asset {
                return Ok(url);
            }
        }

        if let Some(url) = direct_download {
            return Ok(url);
        }

        Err(format!("Não foi possível encontrar um link de download de AppImage direto ou no GitHub para '{}'.", package_name))
    }
}

#[async_trait::async_trait]
impl PackageManagerBackend for AppImageBackend {
    fn name(&self) -> &str {
        "AppImage"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn search(&self, _query: &str) -> Result<Vec<SearchResult>, String> {
        Ok(Vec::new())
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        let url = if package.starts_with("http://") || package.starts_with("https://") {
            package.to_string()
        } else {
            self.resolve_appimage_url(package).await?
        };

        let filename = url.split('/').last().ok_or_else(|| "URL de AppImage inválida".to_string())?;
        
        if !filename.to_lowercase().ends_with(".appimage") {
            return Err("A URL resolvida não aponta para um arquivo .AppImage válido".to_string());
        }

        let app_dir = self.get_applications_dir();
        let temp_filename = format!("{}.tmp", filename);
        let temp_path = app_dir.join(&temp_filename);
        let target_path = app_dir.join(filename);

        println!("Iniciando o download do AppImage de {}...", url);

        let status = Command::new("curl")
            .arg("-L")
            .arg("-#")
            .arg("-o")
            .arg(temp_path.to_str().unwrap())
            .arg(&url)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .map_err(|e| {
                let _ = fs::remove_file(&temp_path);
                format!("Falha ao executar curl: {}", e)
            })?;

        if !status.success() {
            let _ = fs::remove_file(&temp_path);
            return Err("Falha ao baixar o arquivo AppImage.".to_string());
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = match fs::metadata(&temp_path) {
                Ok(m) => m,
                Err(e) => {
                    let _ = fs::remove_file(&temp_path);
                    return Err(format!("Erro ao obter metadados do arquivo temporário: {}", e));
                }
            };
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            if let Err(e) = fs::set_permissions(&temp_path, perms) {
                let _ = fs::remove_file(&temp_path);
                return Err(format!("Falha ao aplicar permissões de execução: {}", e));
            }
        }

        let sanitized_name = self.sanitize_app_name(filename);
        let system_applications_dir = self.get_home_dir().join(".local/share/applications");
        let desktop_dir = self.get_desktop_dir();

        let mut created_shortcuts = Vec::new();

        if !system_applications_dir.exists() {
            if let Err(e) = fs::create_dir_all(&system_applications_dir) {
                let _ = fs::remove_file(&temp_path);
                return Err(format!("Falha ao criar diretório de atalhos do sistema: {}", e));
            }
        }
        
        let system_shortcut_path = system_applications_dir.join(format!("yroz-{}.desktop", sanitized_name.to_lowercase()));
        if let Err(e) = self.create_desktop_shortcut(&sanitized_name, &target_path, &system_applications_dir) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("Falha ao criar atalho no menu de aplicativos: {}", e));
        }
        created_shortcuts.push(system_shortcut_path);

        let desktop_shortcut_path = desktop_dir.join(format!("yroz-{}.desktop", sanitized_name.to_lowercase()));
        if desktop_dir.exists() {
            if let Err(e) = self.create_desktop_shortcut(&sanitized_name, &target_path, &desktop_dir) {
                for shortcut in created_shortcuts {
                    let _ = fs::remove_file(shortcut);
                }
                let _ = fs::remove_file(&temp_path);
                return Err(format!("Falha ao criar atalho na Área de Trabalho: {}", e));
            }
            created_shortcuts.push(desktop_shortcut_path);
        }

        if let Err(e) = fs::rename(&temp_path, &target_path) {
            for shortcut in created_shortcuts {
                let _ = fs::remove_file(shortcut);
            }
            let _ = fs::remove_file(&temp_path);
            return Err(format!("Falha ao efetivar a instalação (rename falhou): {}", e));
        }

        println!("AppImage instalado em: {:?}", target_path);
        println!("Higienizando atalhos como: {}", sanitized_name);
        if desktop_dir.exists() {
            println!("Atalho criado na Área de Trabalho com sucesso!");
        }

        Ok(())
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        let app_dir = self.get_applications_dir();
        let mut app_file_path = None;
        let mut app_filename = String::new();

        let clean_package = package.strip_suffix(".appimage")
            .or_else(|| package.strip_suffix(".AppImage"))
            .unwrap_or(package);

        if let Ok(entries) = fs::read_dir(&app_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let sanitized = self.sanitize_app_name(&name);
                if name.eq_ignore_ascii_case(clean_package) || sanitized.eq_ignore_ascii_case(clean_package) {
                    app_file_path = Some(entry.path());
                    app_filename = name;
                    break;
                }
            }
        }

        let path_to_remove = match app_file_path {
            Some(p) => p,
            None => return Err(format!("AppImage '{}' não encontrado em ~/Applications", package)),
        };

        println!("Removendo AppImage em {:?}...", path_to_remove);
        fs::remove_file(path_to_remove)
            .map_err(|e| format!("Falha ao excluir arquivo AppImage: {}", e))?;

        let sanitized_name = self.sanitize_app_name(&app_filename);
        let shortcut_filename = format!("yroz-{}.desktop", sanitized_name.to_lowercase());

        let system_shortcut = self.get_home_dir().join(".local/share/applications").join(&shortcut_filename);
        if system_shortcut.exists() {
            let _ = fs::remove_file(system_shortcut);
        }

        let desktop_shortcut = self.get_desktop_dir().join(&shortcut_filename);
        if desktop_shortcut.exists() {
            let _ = fs::remove_file(desktop_shortcut);
        }

        println!("AppImage e atalhos removidos com sucesso!");
        Ok(())
    }

    async fn update(&self) -> Result<(), String> {
        Ok(())
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let app_dir = self.get_applications_dir();
        let mut found = None;

        let clean_package = package.strip_suffix(".appimage")
            .or_else(|| package.strip_suffix(".AppImage"))
            .unwrap_or(package);

        if let Ok(entries) = fs::read_dir(&app_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let sanitized = self.sanitize_app_name(&name);
                if name.eq_ignore_ascii_case(clean_package) || sanitized.eq_ignore_ascii_case(clean_package) {
                    found = Some(entry.path());
                    break;
                }
            }
        }

        let path = found.ok_or_else(|| format!("AppImage '{}' não está instalado.", package))?;
        let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
        
        let size_str = format!("{:.2} MB", (metadata.len() as f64) / (1024.0 * 1024.0));
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        let sanitized = self.sanitize_app_name(&filename);

        Ok(PackageInfo {
            name: sanitized,
            version: "AppImage local".to_string(),
            description: format!("Local AppImage salvo em: {:?}", path),
            backend: self.name().to_string(),
            homepage: None,
            size: Some(size_str),
            installed: true,
        })
    }

    async fn list(&self) -> Result<Vec<InstalledPackage>, String> {
        let app_dir = self.get_applications_dir();
        let mut results = Vec::new();

        if let Ok(entries) = fs::read_dir(&app_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.to_lowercase().ends_with(".appimage") {
                    let sanitized = self.sanitize_app_name(&name);
                    results.push(InstalledPackage {
                        name: sanitized,
                        version: "local".to_string(),
                        backend: self.name().to_string(),
                    });
                }
            }
        }

        Ok(results)
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let mut cache = vec![0; b.len() + 1];
    for (j, val) in cache.iter_mut().enumerate() {
        *val = j;
    }
    for (i, ca) in a.chars().enumerate() {
        let mut temp = i;
        cache[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let next_temp = cache[j + 1];
            if ca == cb {
                cache[j + 1] = temp;
            } else {
                cache[j + 1] = std::cmp::min(
                    temp + 1,
                    std::cmp::min(cache[j] + 1, cache[j + 1] + 1)
                );
            }
            temp = next_temp;
        }
    }
    cache[b.len()]
}
