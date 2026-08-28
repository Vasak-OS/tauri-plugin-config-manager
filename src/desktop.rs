use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{plugin::PluginApi, AppHandle, Emitter, Runtime};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex as AsyncMutex, RwLock};

#[cfg(feature = "system-theme-sync")]
use std::process::Command;

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
    schemes_cache: Arc<RwLock<Option<SchemesCacheEntry>>>,
    write_lock: Arc<AsyncMutex<()>>,
    ttl: Duration,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    content: String,
    timestamp: Instant,
}

#[derive(Debug, Clone)]
struct SchemesCacheEntry {
    schemes: Vec<Scheme>,
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
            std::path::PathBuf::from("/usr/share/schemes"),
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
            schemes_cache: Arc::new(RwLock::new(None)),
            write_lock: Arc::new(AsyncMutex::new(())),
            ttl: Duration::from_secs(30 * 60),
        }
    }

    fn home_dir() -> crate::Result<std::path::PathBuf> {
        home::home_dir().ok_or_else(|| {
            crate::Error::Other("No se pudo obtener el directorio home del usuario".to_string())
        })
    }

    /// Si este contenido sirve como configuración.
    ///
    /// Es lo que decide entre usar el archivo y reponerlo. Un JSON parcial sí
    /// sirve —los campos que faltan tienen `#[serde(default)]`—; lo que no
    /// sirve es lo que no parsea o lo que tiene tipos que no corresponden.
    fn es_utilizable(contenido: &str) -> bool {
        serde_json::from_str::<VSKConfig>(contenido).is_ok()
    }

    /// La configuración por defecto, serializada.
    fn contenido_por_defecto() -> crate::Result<String> {
        let default_config = VSKConfig {
            // Los valores de fábrica viven en el modelo, con los `serde(default)`
            // que completan un archivo al que le falte una clave: si se
            // escribieran acá también, las dos copias podrían discrepar.
            style: Style::default(),
            desktop: Some(Desktop {
                wallpaper: vec![],
                iconsize: 48,
                showfiles: true,
                showhiddenfiles: false,
            }),
            fonts: Fonts {
                terminal: String::new(),
                title: String::new(),
                apps: String::new(),
            },
            icons: Icons {
                dark: String::new(),
                light: String::new(),
            },
        };

        serde_json::to_string_pretty(&default_config).map_err(crate::Error::Json)
    }

    /// Aparta el archivo que no sirve y deja uno por defecto en su lugar.
    ///
    /// Devuelve el contenido nuevo. Sin `self` para poder probarla.
    async fn reponer_por_defecto(config_path: &std::path::Path) -> crate::Result<String> {
        let respaldo = Self::ruta_de_respaldo(config_path);
        if let Err(error) = tokio::fs::rename(config_path, &respaldo).await {
            // Que no se pueda apartar no puede dejar al escritorio sin
            // configuración: se sigue, y el archivo se sobrescribe.
            eprintln!(
                "[config-manager] no se pudo apartar la configuración ilegible en {}: {error}",
                respaldo.display()
            );
        }

        let contenido = Self::contenido_por_defecto()?;
        Self::write_file_atomically(config_path, &contenido).await?;
        Ok(contenido)
    }

    /// Adónde se guarda un archivo de configuración que no se pudo leer.
    ///
    /// No se borra: puede tener el fondo de pantalla, los widgets y las fuentes
    /// que alguien eligió, y perder eso en silencio es peor que el problema que
    /// se está arreglando.
    fn ruta_de_respaldo(config_path: &std::path::Path) -> std::path::PathBuf {
        let mut nombre = config_path.file_name().unwrap_or_default().to_os_string();
        nombre.push(".roto");
        config_path.with_file_name(nombre)
    }

    /// El contenido del archivo, garantizando que se pueda usar.
    ///
    /// El caso de «no existe» ya estaba cubierto —se crea uno por defecto—, pero
    /// el de «existe y no sirve» no, y es el peor de los dos: un archivo cortado
    /// por un apagón o editado a mano devolvía texto que no parsea, la interfaz
    /// se quedaba sin colores ni fuentes, y **no se recuperaba nunca**, porque
    /// nada lo reescribía. Cada arranque volvía a estar roto.
    ///
    /// Ahora se comprueba que el contenido sea una configuración válida; si no
    /// lo es, se aparta a un `.roto` y se repone el archivo por defecto.
    async fn leer_utilizable(&self) -> crate::Result<String> {
        let config_path = self.config_path()?;

        // Camino rápido, sin cerrojo: el archivo está y sirve, que es lo que
        // pasa siempre salvo la primera vez o después de un apagón.
        if config_path.exists() {
            let contenido = Self::leer_archivo(&config_path).await?;
            if Self::es_utilizable(&contenido) {
                return Self::normalizar(&contenido);
            }
        }

        let _write_guard = self.write_lock.lock().await;
        self.leer_utilizable_con_el_cerrojo_tomado(&config_path).await
    }

    /// Lo mismo, para quien **ya** tiene el cerrojo de escritura.
    ///
    /// El cerrojo de tokio no es reentrante: `set_darkmode` lo toma antes de
    /// leer, así que si la lectura volviera a pedirlo la recuperación se
    /// quedaría esperándose a sí misma para siempre — justo en el caso en que
    /// alguien intenta cambiar el tema para salir de una configuración rota.
    async fn leer_utilizable_con_el_cerrojo_tomado(
        &self,
        config_path: &std::path::Path,
    ) -> crate::Result<String> {
        if !config_path.exists() {
            self.create_default_config().await?;
        }

        // Otro hilo pudo haberlo repuesto mientras se esperaba el cerrojo.
        let contenido = Self::leer_archivo(config_path).await?;
        if Self::es_utilizable(&contenido) {
            return Self::normalizar(&contenido);
        }

        let repuesto = Self::reponer_por_defecto(config_path).await?;
        Self::normalizar(&repuesto)
    }

    /// El contenido con los valores de fábrica ya puestos donde faltaban.
    ///
    /// Devolver el texto tal como está en el archivo dejaba a medias el arreglo
    /// de las claves ausentes: `serde` las completa **al parsear en Rust**, pero
    /// quien consume `read_config` recibe el JSON crudo y ahí `style.radius`
    /// sigue sin estar. Normalizando, todos ven una configuración completa.
    fn normalizar(contenido: &str) -> crate::Result<String> {
        let config: VSKConfig = serde_json::from_str(contenido).map_err(crate::Error::Json)?;
        serde_json::to_string_pretty(&config).map_err(crate::Error::Json)
    }

    async fn leer_archivo(config_path: &std::path::Path) -> crate::Result<String> {
        tokio::fs::read_to_string(config_path).await.map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                ),
            ))
        })
    }

    /// Read configuration using cache-first strategy.
    pub async fn read_config(&self) -> crate::Result<String> {
        // Single atomic cache lookup
        {
            let guard = self.cache.read().await;
            if let Some(entry) = guard.as_ref() {
                if entry.timestamp.elapsed() < self.ttl {
                    return Ok(entry.content.clone());
                }
            }
        }

        // Cache inválido o inexistente: leer de disco y actualizar cache.
        let config_content = self.leer_utilizable().await?;

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
        let parsed_config: VSKConfig =
            serde_json::from_str(config).map_err(crate::Error::Json)?;

        let _write_guard = self.write_lock.lock().await;

        // Aplicar icon pack en runtime según el modo actual guardado.
        Self::try_apply_icon_pack(&parsed_config.icons, parsed_config.style.darkmode);

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

    #[cfg(feature = "system-theme-sync")]
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
            return Err(crate::Error::Io(std::io::Error::other(
                format!("gsettings {} failed: {}", args.join(" "), detail),
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[cfg(feature = "system-theme-sync")]
    fn has_gsettings_binary() -> bool {
        Command::new("gsettings").arg("help").output().is_ok()
    }

    #[cfg(feature = "system-theme-sync")]
    fn try_sync_system_darkmode(darkmode: bool) {
        if !Self::has_gsettings_binary() {
                tracing::warn!(
                "gsettings not found; skipping system theme sync"
            );
            return;
        }

        let current_scheme_raw = match Self::run_gsettings(&[
            "get",
            "org.gnome.desktop.interface",
            "color-scheme",
        ]) {
            Ok(value) => value,
            Err(e) => {
                tracing::error!(
                    "Could not read system color-scheme via gsettings: {}",
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
                tracing::error!(
                    "Could not set GNOME color-scheme to prefer-dark: {}",
                    e
                );
                return;
            }

            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "gtk-theme",
                crate::gtk_settings::DARK_GTK_THEME,
            ]) {
                tracing::error!(
                    "Could not set GNOME gtk-theme to Adwaita-dark: {}",
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
                tracing::error!(
                    "Could not set GNOME color-scheme to prefer-light: {}",
                    e
                );
                return;
            }

            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "gtk-theme",
                crate::gtk_settings::LIGHT_GTK_THEME,
            ]) {
                tracing::error!(
                    "Could not set GNOME gtk-theme to Adwaita: {}",
                    e
                );
            }
        }
    }

    #[cfg(not(feature = "system-theme-sync"))]
    fn try_sync_system_darkmode(_darkmode: bool) {}

    #[cfg(feature = "system-theme-sync")]
    fn try_apply_icon_pack(icons: &Icons, darkmode: bool) {
        let selected_pack = if darkmode {
            icons.dark.trim()
        } else {
            icons.light.trim()
        };

        // Written first and unconditionally: this is the store GTK reads when an
        // application starts, so it is what decides how the session looks after a
        // reboot. gsettings only reaches programs that are already running, and
        // only where a settings daemon is there to forward it.
        crate::gtk_settings::apply(darkmode, selected_pack);

        if !Self::has_gsettings_binary() || selected_pack.is_empty() {
            return;
        }

        if let Err(e) = Self::run_gsettings(&[
            "set",
            "org.gnome.desktop.interface",
            "icon-theme",
            selected_pack,
        ]) {
            tracing::error!(
                "Could not set icon theme to '{}': {}",
                selected_pack,
                e
            );
        }
    }

    #[cfg(not(feature = "system-theme-sync"))]
    fn try_apply_icon_pack(_icons: &Icons, _darkmode: bool) {}

    pub async fn set_darkmode(&self, darkmode: bool) -> crate::Result<()> {
        let _write_guard = self.write_lock.lock().await;

        // Intentamos sincronizar con GNOME si está disponible, pero sin bloquear
        // la persistencia de configuración cuando no existe gsettings o falla.
        Self::try_sync_system_darkmode(darkmode);

        let config_path = self.config_path()?;

        // Por el mismo camino que la lectura: con un archivo ilegible esto
        // fallaba, así que ni siquiera se podía cambiar el tema para salir del
        // problema. Con la variante que no vuelve a pedir el cerrojo, que acá ya
        // está tomado.
        let config_content = self
            .leer_utilizable_con_el_cerrojo_tomado(&config_path)
            .await?;

        let mut config: VSKConfig =
            serde_json::from_str(&config_content).map_err(crate::Error::Json)?;

        config.style.darkmode = darkmode;

        // Aplicar icon pack asociado al modo actual (dark/light).
        Self::try_apply_icon_pack(&config.icons, darkmode);

        let new_content = serde_json::to_string_pretty(&config).map_err(crate::Error::Json)?;

        Self::write_file_atomically(config_path.as_path(), &new_content).await?;
        // Actualizar cache con el nuevo contenido
        {
            let mut guard = self.cache.write().await;
            *guard = Some(CacheEntry {
                content: new_content,
                timestamp: Instant::now(),
            });
        }
        // Emitir evento para que frontends reaccionen
        let _ = self.app.emit(crate::CONFIG_CHANGED_EVENT, ());
        Ok(())
    }

    /// Limpia el cache manualmente.
    pub async fn clear_cache(&self) {
        {
            let mut guard = self.cache.write().await;
            *guard = None;
        }
        {
            let mut guard = self.schemes_cache.write().await;
            *guard = None;
        }
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

        let config_content = Self::contenido_por_defecto()?;
        Self::write_file_atomically(config_path.as_path(), &config_content).await?;
        Ok(())
    }

    /// Busca y carga todos los esquemas JSON desde /usr/share/vasak-schemes y ~/.config/vasak/schemes
    pub async fn load_schemes(&self) -> crate::Result<Vec<Scheme>> {
        // Cache lookup atómico
        {
            let guard = self.schemes_cache.read().await;
            if let Some(entry) = guard.as_ref() {
                if entry.timestamp.elapsed() < self.ttl {
                    return Ok(entry.schemes.clone());
                }
            }
        }

        let mut schemes = Vec::new();
        let paths = Self::effective_scheme_paths()?;

        // Crear directorios si no existen
        for path in &paths {
            if let Err(e) = tokio::fs::create_dir_all(path).await {
                tracing::warn!(
                    "Could not ensure schemes directory {}: {}",
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
                                                tracing::warn!(
                                                    "Invalid scheme JSON in {}: {}",
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
                tracing::warn!(
                    "Could not read schemes directory {}",
                    path.display()
                );
            }
        }

        {
            let mut guard = self.schemes_cache.write().await;
            *guard = Some(SchemesCacheEntry {
                schemes: schemes.clone(),
                timestamp: Instant::now(),
            });
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

#[cfg(test)]
mod pruebas_de_reposicion {
    use super::*;
    use tauri::test::MockRuntime;

    type Manager = ConfigManager<MockRuntime>;

    #[test]
    fn una_configuracion_completa_sirve() {
        let completa = r#"{"style":{"darkmode":true,"color-scheme":"vasak-default","radius":10},
            "desktop":{"wallpaper":[],"iconsize":36,"showfiles":true,"showhiddenfiles":false},
            "fonts":{"terminal":"","title":"","apps":""},
            "icons":{"dark":"VasakOS-dark","light":"VasakOS-light"}}"#;
        assert!(Manager::es_utilizable(completa));
    }

    #[test]
    fn una_configuracion_parcial_tambien_sirve() {
        // Los campos que faltan tienen `#[serde(default)]`: reponer el archivo
        // por esto sería tirar lo que la persona sí había elegido.
        assert!(Manager::es_utilizable(r#"{"style":{"darkmode":true}}"#));
        assert!(Manager::es_utilizable("{}"));
    }

    #[test]
    fn lo_que_no_parsea_no_sirve() {
        // El caso real: un archivo cortado por un apagón o editado a mano. Antes
        // esto devolvía el texto tal cual, la interfaz se quedaba sin colores ni
        // fuentes, y no se recuperaba nunca porque nada lo reescribía.
        assert!(!Manager::es_utilizable(r#"{"style":{"darkmode":true,"color-sch"#));
        assert!(!Manager::es_utilizable(""));
        assert!(!Manager::es_utilizable("no soy json"));
        // Y un tipo que no corresponde: `radius` es un número.
        assert!(!Manager::es_utilizable(r#"{"style":{"radius":"diez"}}"#));
    }

    #[test]
    fn el_respaldo_va_al_lado_del_original() {
        let ruta = std::path::Path::new("/home/alguien/.config/vasak/vasak.conf");
        assert_eq!(
            Manager::ruta_de_respaldo(ruta),
            std::path::PathBuf::from("/home/alguien/.config/vasak/vasak.conf.roto")
        );
    }

    #[test]
    fn el_contenido_por_defecto_es_utilizable() {
        // Si no, reponer dejaría el archivo tan roto como estaba y el escritorio
        // entraría en un ciclo de reponer y volver a fallar.
        let contenido = Manager::contenido_por_defecto().expect("se serializa");
        assert!(Manager::es_utilizable(&contenido));
    }

    #[test]
    fn a_una_clave_que_falta_se_le_pone_el_valor_de_fabrica() {
        // Y no se repone el archivo: adentro puede estar el fondo de pantalla,
        // los widgets y las fuentes que la persona eligió, y perder todo eso
        // porque falta `radius` sería peor que el problema.
        let sin_radio = r#"{"style":{"darkmode":true,"color-scheme":"vasak-default"},
            "desktop":{"wallpaper":["/un/fondo.jpg"],"iconsize":36,
                       "showfiles":true,"showhiddenfiles":false}}"#;
        let config: VSKConfig = serde_json::from_str(sin_radio).expect("tiene que parsear");

        assert_eq!(config.style.radius, 8, "el radio de fábrica");
        assert!(config.style.darkmode, "lo que sí estaba se respeta");
        assert_eq!(
            config.desktop.expect("el escritorio").wallpaper,
            vec!["/un/fondo.jpg".to_string()],
            "y el fondo no se pierde"
        );
    }

    #[test]
    fn normalizar_completa_lo_que_falta() {
        // Lo que faltaba del arreglo anterior: `serde` completa las claves al
        // parsear en Rust, pero quien consume `read_config` recibía el JSON
        // crudo y ahí `radius` seguía sin estar. Ahora se devuelve normalizado.
        let sin_radio = r#"{"style":{"darkmode":true,"color-scheme":"vasak-default"}}"#;
        let normalizado = Manager::normalizar(sin_radio).expect("normaliza");
        let valor: serde_json::Value = serde_json::from_str(&normalizado).expect("parsea");

        assert_eq!(valor["style"]["radius"], 8, "el radio de fábrica, ya escrito");
        assert_eq!(valor["style"]["darkmode"], true, "y lo que sí estaba se respeta");
    }

    #[test]
    fn los_archivos_del_escritorio_se_muestran_si_no_dice_lo_contrario() {
        // `#[serde(default)]` sobre un `bool` da `false`: a un archivo al que le
        // faltara esta clave se le escondían los archivos del escritorio sin que
        // nadie lo hubiera pedido.
        let sin_showfiles = r#"{"style":{},"desktop":{"wallpaper":[],"iconsize":36}}"#;
        let config: VSKConfig = serde_json::from_str(sin_showfiles).expect("parsea");

        assert!(config.desktop.expect("el escritorio").showfiles);
    }

    #[tokio::test]
    async fn reponer_aparta_el_roto_y_deja_uno_que_sirve() {
        let base = std::env::temp_dir().join(format!("config-manager-prueba-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("directorio de prueba");
        let ruta = base.join("vasak.conf");

        let roto = r#"{"style":{"darkmode":true,"color-sch"#;
        std::fs::write(&ruta, roto).expect("escribir el roto");

        let contenido = Manager::reponer_por_defecto(&ruta).await.expect("repone");

        assert!(Manager::es_utilizable(&contenido));
        assert!(Manager::es_utilizable(
            &std::fs::read_to_string(&ruta).expect("leer el nuevo")
        ));

        // Y lo que había no se pierde: puede tener el fondo de pantalla, los
        // widgets y las fuentes que alguien eligió.
        let respaldo = std::fs::read_to_string(Manager::ruta_de_respaldo(&ruta))
            .expect("el respaldo tiene que estar");
        assert_eq!(respaldo, roto);

        let _ = std::fs::remove_dir_all(&base);
    }
}
