use serde::de::DeserializeOwned;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{plugin::PluginApi, AppHandle, Emitter, Runtime};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex as AsyncMutex, RwLock};

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
    write_lock: Arc<AsyncMutex<()>>,
    ttl: Duration,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    content: String,
    timestamp: Instant,
}

impl<R: Runtime> ConfigManager<R> {
    fn config_path_from_env() -> Option<std::path::PathBuf> {
        std::env::var_os("VASAK_CONFIG_PATH").and_then(|value| {
            let path = std::path::PathBuf::from(value);
            if path.as_os_str().is_empty() {
                None
            } else {
                Some(path)
            }
        })
    }

    fn default_scheme_paths() -> crate::Result<Vec<std::path::PathBuf>> {
        Ok(vec![
            Self::home_dir()?.join(".config/vasak/schemes"),
            std::path::PathBuf::from("/usr/share/vasak-schemes"),
        ])
    }

    fn scheme_paths_from_env() -> Option<Vec<std::path::PathBuf>> {
        let raw = std::env::var_os("VASAK_SCHEMES_PATHS")?;
        let paths: Vec<std::path::PathBuf> = std::env::split_paths(&raw)
            .filter(|path| !path.as_os_str().is_empty())
            .collect();

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }

    fn effective_scheme_paths() -> crate::Result<Vec<std::path::PathBuf>> {
        if let Some(paths) = Self::scheme_paths_from_env() {
            return Ok(paths);
        }

        Self::default_scheme_paths()
    }

    async fn write_file_atomically(path: &std::path::Path, content: &str) -> crate::Result<()> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let parent = path.parent().ok_or_else(|| {
            crate::Error::Other(format!(
                "Config path has no parent directory: {}",
                path.display()
            ))
        })?;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| crate::Error::Other(format!("System time error: {}", e)))?
            .as_nanos();
        let tmp_path = parent.join(format!(".vasak.conf.tmp-{}-{}", std::process::id(), nonce));

        let mut tmp_file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to create temporary config file {}: {}",
                    tmp_path.display(),
                    e
                ),
            ))
        })?;

        if let Err(e) = tmp_file.write_all(content.as_bytes()).await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to write temporary config file {}: {}",
                    tmp_path.display(),
                    e
                ),
            )));
        }

        if let Err(e) = tmp_file.sync_all().await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to sync temporary config file {}: {}",
                    tmp_path.display(),
                    e
                ),
            )));
        }

        drop(tmp_file);

        tokio::fs::rename(&tmp_path, path).await.map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to atomically replace config file {}: {}",
                    path.display(),
                    e
                ),
            ))
        })
    }

    pub fn new(app: AppHandle<R>) -> Self {
        // Default TTL de 30 minutos para evitar lecturas de disco frecuentes.
        Self {
            app,
            cache: Arc::new(RwLock::new(None)),
            write_lock: Arc::new(AsyncMutex::new(())),
            ttl: Duration::from_secs(30 * 60),
        }
    }

    fn home_dir() -> crate::Result<std::path::PathBuf> {
        dirs_next::home_dir().ok_or_else(|| {
            crate::Error::Other("No se pudo obtener el directorio home del usuario".to_string())
        })
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
        let config_path = self.config_path()?;

        // Si el archivo no existe, crearlo con una configuración por defecto
        if !config_path.exists() {
            let _write_guard = self.write_lock.lock().await;
            if !config_path.exists() {
                self.create_default_config().await?;
            }
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
        let config_path = self.config_path()?;

        // Validar semánticamente el payload antes de persistir.
        serde_json::from_str::<VSKConfig>(config).map_err(crate::Error::Json)?;

        let _write_guard = self.write_lock.lock().await;

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

        Self::write_file_atomically(config_path.as_path(), config).await?;
        // Actualizar cache inmediatamente con el contenido provisto
        {
            let mut guard = self.cache.write().await;
            *guard = Some(CacheEntry {
                content: config.to_string(),
                timestamp: Instant::now(),
            });
        }
        // Emitir evento para que frontends reaccionen
        let _ = self.app.emit(crate::CONFIG_CHANGED_EVENT, ());
        Ok(())
    }

    pub fn config_path(&self) -> crate::Result<std::path::PathBuf> {
        if let Some(path) = Self::config_path_from_env() {
            return Ok(path);
        }

        Ok(Self::home_dir()?.join(".config/vasak/vasak.conf"))
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

    fn try_sync_system_darkmode(darkmode: bool) {
        let current_scheme_raw = match Self::run_gsettings(&[
            "get",
            "org.gnome.desktop.interface",
            "color-scheme",
        ]) {
            Ok(value) => value,
            Err(e) => {
                eprintln!(
                    "[ConfigManager::set_darkmode] Could not read system color-scheme via gsettings: {}",
                    e
                );
                return;
            }
        };

        let current_scheme = current_scheme_raw
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        if darkmode && current_scheme != "prefer-dark" {
            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "color-scheme",
                "prefer-dark",
            ]) {
                eprintln!(
                    "[ConfigManager::set_darkmode] Could not set GNOME color-scheme to prefer-dark: {}",
                    e
                );
                return;
            }

            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "gtk-theme",
                "Adwaita-dark",
            ]) {
                eprintln!(
                    "[ConfigManager::set_darkmode] Could not set GNOME gtk-theme to Adwaita-dark: {}",
                    e
                );
            }
        } else if !darkmode && current_scheme != "prefer-light" {
            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "color-scheme",
                "prefer-light",
            ]) {
                eprintln!(
                    "[ConfigManager::set_darkmode] Could not set GNOME color-scheme to prefer-light: {}",
                    e
                );
                return;
            }

            if let Err(e) =
                Self::run_gsettings(&["set", "org.gnome.desktop.interface", "gtk-theme", "Adwaita"])
            {
                eprintln!(
                    "[ConfigManager::set_darkmode] Could not set GNOME gtk-theme to Adwaita: {}",
                    e
                );
            }
        }
    }

    pub async fn set_darkmode(&self, darkmode: bool) -> crate::Result<()> {
        let _write_guard = self.write_lock.lock().await;

        // Intentamos sincronizar con GNOME si está disponible, pero sin bloquear
        // la persistencia de configuración cuando no existe gsettings o falla.
        Self::try_sync_system_darkmode(darkmode);

        let config_path = self.config_path()?;

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
            serde_json::from_str(&config_content).map_err(crate::Error::Json)?;

        config.style.darkmode = darkmode;

        let new_content = serde_json::to_string_pretty(&config).map_err(crate::Error::Json)?;

        Self::write_file_atomically(config_path.as_path(), &new_content).await?;
        // Actualizar cache con el nuevo contenido
        {
            let mut guard = self.cache.write().await;
            *guard = Some(CacheEntry {
                content: serde_json::to_string_pretty(&config).map_err(crate::Error::Json)?,
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
        let config_path = self.config_path()?;

        // Si el archivo no existe, crearlo con una configuración por defecto
        if !config_path.exists() {
            let _write_guard = self.write_lock.lock().await;
            if !config_path.exists() {
                self.create_default_config().await?;
            }
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
        let config_path = self.config_path()?;

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
                color_scheme: "vasak-default".to_string(),
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

        Self::write_file_atomically(config_path.as_path(), &config_content).await?;

        Ok(())
    }

    /// Busca y carga todos los esquemas JSON desde /usr/share/vasak-schemes y ~/.config/vasak/schemes
    pub async fn load_schemes(&self) -> crate::Result<Vec<Scheme>> {
        let mut schemes = Vec::new();
        let paths = Self::effective_scheme_paths()?;

        // Crear directorios si no existen
        for path in &paths {
            if let Err(e) = tokio::fs::create_dir_all(path).await {
                eprintln!(
                    "[ConfigManager::load_schemes] Could not ensure schemes directory {}: {}",
                    path.display(),
                    e
                );
            }
        }

        // Buscar esquemas en las rutas efectivas.
        for path in &paths {
            if let Ok(mut entries) = tokio::fs::read_dir(path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_file() {
                            if let Some(filename) = entry.file_name().to_str() {
                                if filename.ends_with(".json") {
                                    let file_path = entry.path();
                                    if let Ok(content) = tokio::fs::read_to_string(&file_path).await
                                    {
                                        match serde_json::from_str::<SchemeData>(&content) {
                                            Ok(scheme_data) => {
                                                schemes.push(Scheme {
                                                    path: file_path.to_string_lossy().to_string(),
                                                    scheme: scheme_data,
                                                });
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "[ConfigManager::load_schemes] Invalid scheme JSON in {}: {}",
                                                    file_path.display(),
                                                    e
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                eprintln!(
                    "[ConfigManager::load_schemes] Could not read schemes directory {}",
                    path.display()
                );
            }
        }

        Ok(schemes)
    }

    /// Obtiene un esquema específico por su ID.
    /// Prioridad:
    /// 1) orden de VASAK_SCHEMES_PATHS (si existe)
    /// 2) orden por defecto: ~/.config/vasak/schemes y luego /usr/share/vasak-schemes
    pub async fn get_scheme_by_id(&self, scheme_id: &str) -> crate::Result<Option<Scheme>> {
        let schemes = self.load_schemes().await?;
        let preferred_paths = Self::effective_scheme_paths()?;

        // Buscar esquemas que coincidan con el ID
        let matching_schemes: Vec<Scheme> = schemes
            .into_iter()
            .filter(|s| s.scheme.id == scheme_id)
            .collect();

        if matching_schemes.is_empty() {
            return Ok(None);
        }

        for preferred in preferred_paths {
            let preferred_prefix = preferred.to_string_lossy().to_string();
            for scheme in &matching_schemes {
                if scheme.path.starts_with(&preferred_prefix) {
                    return Ok(Some(scheme.clone()));
                }
            }
        }

        // Fallback por seguridad.
        Ok(matching_schemes.into_iter().next())
    }
}
