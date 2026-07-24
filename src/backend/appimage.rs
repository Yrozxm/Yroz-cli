use crate::backend::{PackageManagerBackend, SearchResult, PackageInfo, InstalledPackage};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use colored::*;

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
}

#[async_trait::async_trait]
impl PackageManagerBackend for AppImageBackend {
    fn name(&self) -> &str {
        "AppImage"
    }

    fn is_available(&self) -> bool {
        // AppImage está sempre disponível no Linux por usar curl e fs locais
        true
    }

    async fn search(&self, _query: &str) -> Result<Vec<SearchResult>, String> {
        // Busca direta em repositório não é suportada por ser focado em instalação de URL
        Ok(Vec::new())
    }

    async fn install(&self, package: &str) -> Result<(), String> {
        if !package.starts_with("http://") && !package.starts_with("https://") {
            return Err("Para instalar via AppImage, você deve passar a URL direta do arquivo (ex: yroz install https://exemplo.com/app.AppImage)".to_string());
        }

        let url = package;
        let filename = url.split('/').last().ok_or_else(|| "URL de AppImage inválida".to_string())?;
        
        if !filename.to_lowercase().ends_with(".appimage") {
            return Err("A URL fornecida não aponta para um arquivo .AppImage válido".to_string());
        }

        let app_dir = self.get_applications_dir();
        let target_path = app_dir.join(filename);

        println!("Iniciando o download do AppImage de {}...", url);

        let status = Command::new("curl")
            .arg("-L")
            .arg("-o")
            .arg(target_path.to_str().unwrap())
            .arg(url)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .map_err(|e| format!("Falha ao executar curl: {}", e))?;

        if !status.success() {
            let _ = fs::remove_file(&target_path);
            return Err("Falha ao baixar o arquivo AppImage.".to_string());
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&target_path)
                .map_err(|e| format!("Erro ao obter metadados do arquivo: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&target_path, perms)
                .map_err(|e| format!("Falha ao aplicar permissões de execução: {}", e))?;
        }

        let sanitized_name = self.sanitize_app_name(filename);
        println!("AppImage instalado em: {:?}", target_path);
        println!("Higienizando atalhos como: {}", sanitized_name);

        let system_applications_dir = self.get_home_dir().join(".local/share/applications");
        if !system_applications_dir.exists() {
            let _ = fs::create_dir_all(&system_applications_dir);
        }
        
        let _ = self.create_desktop_shortcut(&sanitized_name, &target_path, &system_applications_dir);

        let desktop_dir = self.get_desktop_dir();
        if desktop_dir.exists() {
            if let Err(e) = self.create_desktop_shortcut(&sanitized_name, &target_path, &desktop_dir) {
                println!("{} Aviso: Falha ao criar atalho na Área de Trabalho: {}", "".yellow(), e);
            } else {
                println!("Atalho criado na Área de Trabalho com sucesso!");
            }
        }

        Ok(())
    }

    async fn remove(&self, package: &str) -> Result<(), String> {
        let app_dir = self.get_applications_dir();
        let mut app_file_path = None;
        let mut app_filename = String::new();

        if let Ok(entries) = fs::read_dir(&app_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let sanitized = self.sanitize_app_name(&name);
                if name.eq_ignore_ascii_case(package) || sanitized.eq_ignore_ascii_case(package) {
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
        // AppImages são atualizados reinstalando a URL mais recente
        Ok(())
    }

    async fn info(&self, package: &str) -> Result<PackageInfo, String> {
        let app_dir = self.get_applications_dir();
        let mut found = None;

        if let Ok(entries) = fs::read_dir(&app_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let sanitized = self.sanitize_app_name(&name);
                if name.eq_ignore_ascii_case(package) || sanitized.eq_ignore_ascii_case(package) {
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
