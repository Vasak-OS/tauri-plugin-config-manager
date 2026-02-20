use serde::de::DeserializeOwned;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{plugin::PluginApi, AppHandle, Emitter, Runtime};
use tokio::sync::RwLock;

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<ConfigManager<R>> {
    Ok(ConfigManager::new(app.clone()))
}

/// Access to the config-manager APIs with an internal TTL cache.
#[derive(Clone)]
pub struct ConfigManager<R: Runtime> {
    app: AppHandle<R>,
    cache: Arc<RwLock<Option<CacheEntry>>>,
    ttl: Duration,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    content: String,
    timestamp: Instant,
}

impl<R: Runtime> ConfigManager<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        // Default TTL de 30 minutos para evitar lecturas de disco frecuentes.
        Self {
            app,
            cache: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(30 * 60),
        }
    }

    fn home_dir() -> std::path::PathBuf {
        dirs_next::home_dir().expect("No se pudo obtener el directorio home del usuario")
    }

    /// Returns true if the cache is present and not expired.
    async fn is_cache_valid(&self) -> bool {
        let guard = self.cache.read().await;
        if let Some(entry) = guard.as_ref() {
            entry.timestamp.elapsed() < self.ttl
        } else {
            false
        }
    }

    /// Read configuration using cache-first strategy.
    pub async fn read_config(&self) -> crate::Result<String> {
        if self.is_cache_valid().await {
            let guard = self.cache.read().await;
            if let Some(entry) = guard.as_ref() {
                return Ok(entry.content.clone());
            }
        }

        // Cache inválido o inexistente: leer de disco y actualizar cache.
        let config_path = self.config_path();

        // Si el archivo no existe, crearlo con una configuración por defecto
        if !config_path.exists() {
            self.create_default_config().await?;
        }

        let config_content = tokio::fs::read_to_string(&config_path).await.map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                ),
            ))
        })?;

        {
            let mut guard = self.cache.write().await;
            *guard = Some(CacheEntry {
                content: config_content.clone(),
                timestamp: Instant::now(),
            });
        }

        Ok(config_content)
    }

    pub async fn write_config(&self, config: &str) -> crate::Result<()> {
        let config_path = self.config_path();

        // Crear el directorio padre si no existe
        if let Some(parent) = config_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                crate::Error::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to create config directory {}: {}",
                        parent.display(),
                        e
                    ),
                ))
            })?;
        }

        tokio::fs::write(&config_path, config).await.map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to write to config file {}: {}",
                    config_path.display(),
                    e
                ),
            ))
        })?;
        // Actualizar cache inmediatamente con el contenido provisto
        {
            let mut guard = self.cache.write().await;
            *guard = Some(CacheEntry {
                content: config.to_string(),
                timestamp: Instant::now(),
            });
        }
        // Emitir evento para que frontends reaccionen
        let _ = self.app.emit("config-changed", ());
        Ok(())
    }

    pub fn config_path(&self) -> std::path::PathBuf {
        Self::home_dir().join(".config/vasak/vasak.conf")
    }

    fn run_gsettings(args: &[&str]) -> crate::Result<String> {
        let output = Command::new("gsettings").args(args).output().map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to run gsettings {}: {}", args.join(" "), e),
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            return Err(crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("gsettings {} failed: {}", args.join(" "), detail),
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub async fn set_darkmode(&self, darkmode: bool) -> crate::Result<()> {
        let current_scheme_raw =
            Self::run_gsettings(&["get", "org.gnome.desktop.interface", "color-scheme"][..])?;
        let current_scheme = current_scheme_raw
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        if darkmode && current_scheme != "prefer-dark" {
            Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "color-scheme",
                "prefer-dark",
            ][..])?;
            Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "gtk-theme",
                "Adwaita-dark",
            ][..])?;
        } else if !darkmode && current_scheme != "prefer-light" {
            Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "color-scheme",
                "prefer-light",
            ][..])?;
            Self::run_gsettings(&["set", "org.gnome.desktop.interface", "gtk-theme", "Adwaita"][..])?;
        }

        let config_path = self.config_path();

        // Si el archivo no existe, crearlo con una configuración por defecto
        if !config_path.exists() {
            self.create_default_config().await?;
        }

        let config_content = tokio::fs::read_to_string(&config_path).await.map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                ),
            ))
        })?;

        let mut config: VSKConfig =
            serde_json::from_str(&config_content).map_err(|e| crate::Error::Json(e))?;

        config.style.darkmode = darkmode;

        let new_content =
            serde_json::to_string_pretty(&config).map_err(|e| crate::Error::Json(e))?;

        tokio::fs::write(&config_path, new_content)
            .await
            .map_err(|e| {
                crate::Error::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to write to config file {}: {}",
                        config_path.display(),
                        e
                    ),
                ))
            })?;
        // Actualizar cache con el nuevo contenido
        {
            let mut guard = self.cache.write().await;
            *guard = Some(CacheEntry {
                content: serde_json::to_string_pretty(&config)
                    .map_err(|e| crate::Error::Json(e))?,
                timestamp: Instant::now(),
            });
        }
        Ok(())
    }

    /// Limpia el cache manualmente.
    pub async fn clear_cache(&self) {
        let mut guard = self.cache.write().await;
        *guard = None;
    }

    /// Fuerza refrescar el cache leyendo desde disco.
    pub async fn refresh_cache_from_file(&self) -> crate::Result<()> {
        let config_path = self.config_path();

        // Si el archivo no existe, crearlo con una configuración por defecto
        if !config_path.exists() {
            self.create_default_config().await?;
        }

        let content = tokio::fs::read_to_string(&config_path).await.map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                ),
            ))
        })?;
        let mut guard = self.cache.write().await;
        *guard = Some(CacheEntry {
            content,
            timestamp: Instant::now(),
        });
        Ok(())
    }

    /// Crea el archivo de configuración con valores por defecto.
    async fn create_default_config(&self) -> crate::Result<()> {
        let config_path = self.config_path();

        // Crear el directorio padre si no existe
        if let Some(parent) = config_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                crate::Error::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to create config directory {}: {}",
                        parent.display(),
                        e
                    ),
                ))
            })?;
        }

        // Configuración por defecto
        let default_config = VSKConfig {
            style: Style {
                darkmode: false,
                primarycolor: "#007acc".to_string(),
                radius: 8,
            },
            desktop: Some(Desktop {
                wallpaper: vec![],
                iconsize: 48,
                showfiles: true,
                showhiddenfiles: false,
            }),
        };

        let config_content =
            serde_json::to_string_pretty(&default_config).map_err(|e| crate::Error::Json(e))?;

        tokio::fs::write(&config_path, &config_content)
            .await
            .map_err(|e| {
                crate::Error::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to write default config file {}: {}",
                        config_path.display(),
                        e
                    ),
                ))
            })?;

        Ok(())
    }

    /// Busca y carga todos los esquemas JSON desde /usr/share/vasak-schemes y ~/.config/vasak/schemes
    pub async fn load_schemes(&self) -> crate::Result<Vec<Scheme>> {
        let mut schemes = Vec::new();

        // Rutas donde buscar esquemas
        let system_schemes_path = std::path::PathBuf::from("/usr/share/vasak-schemes");
        let user_schemes_path = Self::home_dir().join(".config/vasak/schemes");

        // Crear directorios si no existen
        for path in &[&system_schemes_path, &user_schemes_path] {
            tokio::fs::create_dir_all(path).await.ok(); // Ignoramos errores de creación (ej: permisos)
        }

        // Buscar esquemas en ambas ubicaciones
        for path in &[system_schemes_path, user_schemes_path] {
            if let Ok(mut entries) = tokio::fs::read_dir(path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_file() {
                            if let Some(filename) = entry.file_name().to_str() {
                                if filename.ends_with(".json") {
                                    let file_path = entry.path();
                                    if let Ok(content) = tokio::fs::read_to_string(&file_path).await {
                                        if let Ok(scheme_data) =
                                            serde_json::from_str::<SchemeData>(&content)
                                        {
                                            schemes.push(Scheme {
                                                path: file_path
                                                    .to_string_lossy()
                                                    .to_string(),
                                                scheme: scheme_data,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(schemes)
    }
}
