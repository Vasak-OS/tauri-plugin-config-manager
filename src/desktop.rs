use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{plugin::PluginApi, AppHandle, Emitter, Runtime};
use tokio::sync::RwLock;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<ConfigManager<R>> {
    Ok(ConfigManager::new(app.clone()))
}

/// Access to the config-manager APIs with an internal TTL cache.
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

#[derive(Debug, Serialize, Deserialize)]
pub struct VSKConfig {
    pub style: Style,
    pub desktop: Option<Desktop>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Desktop {
    pub wallpaper: Vec<String>,
    pub iconsize: u32,
    pub showfiles: bool,
    pub showhiddenfiles: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Style {
    pub darkmode: bool,
    pub primarycolor: String,
    pub radius: u32,
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

    pub async fn set_darkmode(&self, darkmode: bool) -> crate::Result<()> {
        let config_path = self.config_path();
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
}
