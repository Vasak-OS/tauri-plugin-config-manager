use serde::de::DeserializeOwned;
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// Sube cada vez que se invalida `schemes_cache`.
    ///
    /// `load_schemes` lee el directorio sin cerrojo y guarda el resultado al
    /// final. Si en el medio alguien guarda un esquema e invalida, la lectura
    /// que ya estaba en curso guardaría la lista **vieja** en un caché recién
    /// vaciado, y ahí se quedaría media hora. Con esto, una lectura sólo guarda
    /// si nadie invalidó mientras leía.
    schemes_generation: Arc<AtomicU64>,
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

    pub fn new(app: AppHandle<R>) -> Self {
        // Default TTL de 30 minutos para evitar lecturas de disco frecuentes.
        Self {
            app,
            cache: Arc::new(RwLock::new(None)),
            schemes_cache: Arc::new(RwLock::new(None)),
            schemes_generation: Arc::new(AtomicU64::new(0)),
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
    fn is_usable(content: &str) -> bool {
        serde_json::from_str::<VSKConfig>(content).is_ok()
    }

    /// La configuración por defecto, serializada.
    fn default_content() -> crate::Result<String> {
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
                extra: Default::default(),
            }),
            fonts: Fonts {
                terminal: String::new(),
                title: String::new(),
                apps: String::new(),
                extra: Default::default(),
            },
            icons: Icons {
                dark: String::new(),
                light: String::new(),
                extra: Default::default(),
            },
            extra: Default::default(),
        };

        serde_json::to_string_pretty(&default_config).map_err(crate::Error::Json)
    }

    /// Aparta el archivo que no sirve y deja uno por defecto en su lugar.
    ///
    /// Devuelve el contenido nuevo. Sin `self` para poder probarla.
    async fn restore_default(config_path: &std::path::Path) -> crate::Result<String> {
        let backup = Self::backup_path(config_path);
        if let Err(error) = tokio::fs::rename(config_path, &backup).await {
            // Que no se pueda apartar no puede dejar al escritorio sin
            // configuración: se sigue, y el archivo se sobrescribe.
            eprintln!(
                "[config-manager] no se pudo apartar la configuración ilegible en {}: {error}",
                backup.display()
            );
        }

        let content = Self::default_content()?;
        write_file_atomically(config_path, &content).await?;
        Ok(content)
    }

    /// Adónde se guarda un archivo de configuración que no se pudo leer.
    ///
    /// No se borra: puede tener el fondo de pantalla, los widgets y las fuentes
    /// que alguien eligió, y perder eso en silencio es peor que el problema que
    /// se está arreglando.
    fn backup_path(config_path: &std::path::Path) -> std::path::PathBuf {
        let mut name = config_path.file_name().unwrap_or_default().to_os_string();
        name.push(".roto");
        config_path.with_file_name(name)
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
    async fn read_usable(&self) -> crate::Result<String> {
        let config_path = self.config_path()?;

        // Camino rápido, sin cerrojo: el archivo está y sirve, que es lo que
        // pasa siempre salvo la primera vez o después de un apagón.
        if config_path.exists() {
            let content = Self::read_file(&config_path).await?;
            if Self::is_usable(&content) {
                return Self::normalize(&content);
            }
        }

        let _write_guard = self.write_lock.lock().await;
        self.read_usable_with_lock_held(&config_path).await
    }

    /// Lo mismo, para quien **ya** tiene el cerrojo de escritura.
    ///
    /// El cerrojo de tokio no es reentrante: `set_darkmode` lo toma antes de
    /// leer, así que si la lectura volviera a pedirlo la recuperación se
    /// quedaría esperándose a sí misma para siempre — justo en el caso en que
    /// alguien intenta cambiar el tema para salir de una configuración rota.
    async fn read_usable_with_lock_held(
        &self,
        config_path: &std::path::Path,
    ) -> crate::Result<String> {
        if !config_path.exists() {
            self.create_default_config().await?;
        }

        // Otro hilo pudo haberlo repuesto mientras se esperaba el cerrojo.
        let content = Self::read_file(config_path).await?;
        if Self::is_usable(&content) {
            return Self::normalize(&content);
        }

        let restored = Self::restore_default(config_path).await?;
        Self::normalize(&restored)
    }

    /// El contenido con los valores de fábrica ya puestos donde faltaban.
    ///
    /// Devolver el texto tal como está en el archivo dejaba a medias el arreglo
    /// de las claves ausentes: `serde` las completa **al parsear en Rust**, pero
    /// quien consume `read_config` recibe el JSON crudo y ahí `style.radius`
    /// sigue sin estar. Normalizando, todos ven una configuración completa.
    fn normalize(content: &str) -> crate::Result<String> {
        let config: VSKConfig = serde_json::from_str(content).map_err(crate::Error::Json)?;
        serde_json::to_string_pretty(&config).map_err(crate::Error::Json)
    }

    async fn read_file(config_path: &std::path::Path) -> crate::Result<String> {
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
        let config_content = self.read_usable().await?;

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
        let parsed_config: VSKConfig = serde_json::from_str(config).map_err(crate::Error::Json)?;

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

        write_file_atomically(config_path.as_path(), config).await?;

        // Las fuentes se aplican **acá**, después de que `vasak.conf` quedó
        // escrito, y no antes con el pack de iconos.
        //
        // Antes iban arriba, y si crear el directorio o la escritura atómica
        // fallaban, `write_config` devolvía el error con las fuentes de GTK y de
        // GSettings **ya cambiadas**: un guardado fallido dejaba el sistema con
        // una tipografía que la configuración no dice en ninguna parte, y sin
        // forma de volver salvo elegir otra y guardar bien.
        Self::try_apply_fonts(&parsed_config.fonts);

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

    /// Cuánto se le da a `gsettings` antes de darlo por perdido.
    ///
    /// Sin tope, cada llamada podía esperar para siempre **con `write_lock`
    /// tomado**, así que un D-Bus trabado no dejaba guardar la configuración
    /// nunca más — y esta rama sumó seis llamadas más al camino de guardado, lo
    /// que agranda esa ventana. Leer o escribir un ajuste es cuestión de
    /// milisegundos; dos segundos ya son un problema del sistema, y ante eso
    /// vale más seguir sin sincronizar que quedarse colgado.
    #[cfg(feature = "system-theme-sync")]
    const GSETTINGS_TIMEOUT: Duration = Duration::from_secs(2);

    #[cfg(feature = "system-theme-sync")]
    fn run_gsettings(args: &[&str]) -> crate::Result<String> {
        let mut child = Command::new("gsettings")
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                crate::Error::Io(std::io::Error::new(
                    e.kind(),
                    format!("Failed to run gsettings {}: {}", args.join(" "), e),
                ))
            })?;

        // Se sondea en lugar de esperar: `wait_with_output` no tiene forma de
        // rendirse. La salida va a tuberías y son dos líneas, así que no hay
        // riesgo de llenar el buffer mientras se sondea.
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(e) => {
                    return Err(crate::Error::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to wait for gsettings {}: {}", args.join(" "), e),
                    )))
                }
            }
            if started.elapsed() >= Self::GSETTINGS_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(crate::Error::Io(std::io::Error::other(format!(
                    "gsettings {} no contestó en {} s",
                    args.join(" "),
                    Self::GSETTINGS_TIMEOUT.as_secs()
                ))));
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let output = child.wait_with_output().map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to read gsettings {}: {}", args.join(" "), e),
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            return Err(crate::Error::Io(std::io::Error::other(format!(
                "gsettings {} failed: {}",
                args.join(" "),
                detail
            ))));
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
            tracing::warn!("gsettings not found; skipping system theme sync");
            return;
        }

        let current_scheme_raw =
            match Self::run_gsettings(&["get", "org.gnome.desktop.interface", "color-scheme"]) {
                Ok(value) => value,
                Err(e) => {
                    tracing::error!("Could not read system color-scheme via gsettings: {}", e);
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
                tracing::error!("Could not set GNOME color-scheme to prefer-dark: {}", e);
                return;
            }

            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "gtk-theme",
                crate::gtk_settings::DARK_GTK_THEME,
            ]) {
                tracing::error!("Could not set GNOME gtk-theme to Adwaita-dark: {}", e);
            }
        } else if !darkmode && current_scheme != "prefer-light" {
            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "color-scheme",
                "prefer-light",
            ]) {
                tracing::error!("Could not set GNOME color-scheme to prefer-light: {}", e);
                return;
            }

            if let Err(e) = Self::run_gsettings(&[
                "set",
                "org.gnome.desktop.interface",
                "gtk-theme",
                crate::gtk_settings::LIGHT_GTK_THEME,
            ]) {
                tracing::error!("Could not set GNOME gtk-theme to Adwaita: {}", e);
            }
        }
    }

    #[cfg(not(feature = "system-theme-sync"))]
    fn try_sync_system_darkmode(_darkmode: bool) {}

    /// Lleva las fuentes elegidas a donde las leen las aplicaciones ajenas.
    ///
    /// `vasak.conf` lo leen las aplicaciones propias; esto es la otra mitad.
    /// Escribe los dos sitios que hacen falta y por motivos distintos:
    ///
    /// * el `settings.ini` de GTK, que es el que se lee **al arrancar** y por lo
    ///   tanto el que decide cómo se ve la sesión después de reiniciar;
    /// * las claves de GSettings, que son las que informa el portal a las
    ///   aplicaciones aisladas y las que usa GTK4.
    ///
    /// Las aplicaciones Qt vienen de arriba: su tema de plataforma es el de GTK
    /// —`QT_QPA_PLATFORMTHEME=gtk3`, que trae `qt6-base`— así que leen esto
    /// mismo y no hace falta un tercer archivo que mantener en sincronía.
    ///
    /// La monoespaciada sale de la fuente de la terminal, que es la que se elige
    /// en Ajustes: tener dos monoespaciadas distintas —una para la terminal y
    /// otra para el resto— sería justo la divergencia que esto viene a cerrar.
    #[cfg(feature = "system-theme-sync")]
    fn try_apply_fonts(fonts: &crate::models::Fonts) {
        crate::gtk_settings::aplicar_fuente(&fonts.apps);

        if !Self::has_gsettings_binary() {
            return;
        }

        // Cada clave con el cuerpo que ya tenía: cambiar de tipografía no tiene
        // por qué devolver a 11 a quien lo subió a 13 para ver mejor.
        let keys = [
            ("font-name", fonts.apps.trim()),
            ("document-font-name", fonts.apps.trim()),
            ("monospace-font-name", fonts.terminal.trim()),
        ];

        for (key, family) in keys {
            if family.is_empty() {
                continue;
            }
            let current = Self::run_gsettings(&["get", "org.gnome.desktop.interface", key])
                .unwrap_or_default();
            let current = current.trim().trim_matches('\'').trim_matches('"');
            let desired = crate::gtk_settings::con_familia(current, family);

            if current == desired {
                continue;
            }
            if let Err(e) =
                Self::run_gsettings(&["set", "org.gnome.desktop.interface", key, &desired])
            {
                tracing::error!("Could not set GNOME {}: {}", key, e);
            }
        }
    }

    #[cfg(not(feature = "system-theme-sync"))]
    fn try_apply_fonts(_fonts: &crate::models::Fonts) {}

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
            tracing::error!("Could not set icon theme to '{}': {}", selected_pack, e);
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
        let config_content = self.read_usable_with_lock_held(&config_path).await?;

        let mut config: VSKConfig =
            serde_json::from_str(&config_content).map_err(crate::Error::Json)?;

        config.style.darkmode = darkmode;

        // Aplicar icon pack asociado al modo actual (dark/light).
        Self::try_apply_icon_pack(&config.icons, darkmode);

        let new_content = serde_json::to_string_pretty(&config).map_err(crate::Error::Json)?;

        write_file_atomically(config_path.as_path(), &new_content).await?;
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
        self.invalidate_schemes_cache().await;
    }

    /// Vacía el caché de esquemas, para que la próxima lectura vaya al disco.
    ///
    /// Lo llaman `save_user_scheme` y el vigilante, cuando cambia un archivo
    /// del directorio de esquemas del usuario.
    pub async fn invalidate_schemes_cache(&self) {
        let mut guard = self.schemes_cache.write().await;
        self.schemes_generation.fetch_add(1, Ordering::SeqCst);
        *guard = None;
    }

    /// El directorio donde se guardan los esquemas del usuario.
    ///
    /// Ver [`user_schemes_dir_from`] para cuál es y por qué.
    pub fn user_schemes_dir(&self) -> crate::Result<std::path::PathBuf> {
        user_schemes_dir_from(&Self::effective_scheme_paths()?)
    }

    /// Guarda un esquema en el directorio del usuario, como `<id>.json`.
    ///
    /// Sólo ahí: nunca en `/usr/share/schemes`, que es del sistema. Si ya hay
    /// uno con ese id en el directorio del usuario se reemplaza, de forma
    /// atómica; uno del sistema con el mismo id queda intacto y pasa a estar
    /// tapado por éste, porque el directorio del usuario va primero.
    ///
    /// No emite `config-changed`: lo emite el vigilante de cada aplicación
    /// —incluida la que guardó— al ver el archivo nuevo, y emitirlo también acá
    /// haría que la que guarda recargue dos veces por cada color que se toca.
    pub async fn save_user_scheme(&self, scheme: SchemeData) -> crate::Result<Scheme> {
        let dir = self.user_schemes_dir()?;
        let path = write_user_scheme(&dir, &scheme).await?;
        self.invalidate_schemes_cache().await;
        Ok(Scheme {
            path: path.to_string_lossy().to_string(),
            scheme,
        })
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

        let config_content = Self::default_content()?;
        write_file_atomically(config_path.as_path(), &config_content).await?;
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

        let generation = self.schemes_generation.load(Ordering::SeqCst);
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
                tracing::warn!("Could not read schemes directory {}", path.display());
            }
        }

        {
            let mut guard = self.schemes_cache.write().await;
            // Con el cerrojo tomado, que es el mismo que toma la invalidación:
            // si la generación cambió, lo que se leyó puede ser anterior al
            // guardado, y se devuelve sin cachearlo.
            if self.schemes_generation.load(Ordering::SeqCst) == generation {
                *guard = Some(SchemesCacheEntry {
                    schemes: schemes.clone(),
                    timestamp: Instant::now(),
                });
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

/// Escribe el archivo pasando por uno temporal y un `rename`.
///
/// Escribir encima del archivo de configuración deja una ventana en la que
/// se lo puede encontrar a medias si la máquina se apaga en el medio, y
/// entonces `read_usable` se topa con un JSON que no parsea y la
/// interfaz se queda sin colores ni fuentes. El temporal se escribe y se
/// sincroniza antes de moverlo, y el `rename` —atómico dentro del mismo
/// directorio— deja el archivo viejo o el nuevo, nunca uno a medias.
///
/// Lo usan `vasak.conf` y los esquemas del usuario: un esquema a medias es
/// igual de malo, porque el vigilante de las demás aplicaciones lo lee en
/// cuanto aparece. El temporal se llama como [`temporary_path`] y empieza
/// con punto, que es lo que el vigilante usa para no hacerle caso.
///
/// Los tres caminos de error borran el temporal por eso: si se lo deja
/// atrás, cada escritura posterior deja otro, y el directorio se llena de
/// archivos que nadie llega a mirar.
async fn write_file_atomically(path: &std::path::Path, content: &str) -> crate::Result<()> {
    let tmp_path = temporary_path(path)?;

    let mut tmp_file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
        crate::Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to create temporary file {}: {}",
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
                "Failed to write temporary file {}: {}",
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
                "Failed to sync temporary file {}: {}",
                tmp_path.display(),
                e
            ),
        )));
    }

    drop(tmp_file);

    if let Err(e) = tokio::fs::rename(&tmp_path, path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(crate::Error::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to atomically replace {}: {}", path.display(), e),
        )));
    }

    Ok(())
}

/// Cuántas escrituras atómicas lleva el proceso.
///
/// El nombre del temporal lleva el pid y la hora en nanosegundos, y con eso
/// alcanzaba mientras sólo se escribía `vasak.conf`, siempre con `write_lock`
/// tomado. Los esquemas se guardan sin ese cerrojo —el editor guarda mientras
/// se arrastra un color—, y dos guardados en el mismo nanosegundo compartirían
/// temporal: uno escribiría encima del otro antes del `rename`.
static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Dónde va el temporal de una escritura atómica de `path`.
///
/// Al lado del destino, porque el `rename` sólo es atómico dentro del mismo
/// sistema de archivos, y con un punto adelante, que es lo que
/// [`is_temporary_file`] mira para que el vigilante no reaccione a él.
fn temporary_path(path: &std::path::Path) -> crate::Result<std::path::PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let parent = path.parent().ok_or_else(|| {
        crate::Error::Other(format!("Path has no parent directory: {}", path.display()))
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| crate::Error::Other(format!("Path has no file name: {}", path.display())))?
        .to_string_lossy();

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| crate::Error::Other(format!("System time error: {}", e)))?
        .as_nanos();
    let counter = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(parent.join(format!(
        ".{}.tmp-{}-{}-{}",
        file_name,
        std::process::id(),
        nanos,
        counter
    )))
}

/// Si el archivo es un temporal de [`write_file_atomically`] (o cualquier
/// otro archivo oculto).
///
/// El vigilante lo usa para no disparar una recarga por un archivo que existe
/// unos milisegundos: el cambio que importa es el `rename` al nombre final, y
/// ése llega aparte.
pub(crate) fn is_temporary_file(path: &std::path::Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

/// El largo máximo de un id de esquema.
pub const MAX_SCHEME_ID_LEN: usize = 64;

/// Si `id` sirve como id de un esquema del usuario: `^[a-z0-9][a-z0-9-]{0,63}$`.
///
/// El id es también el nombre del archivo, así que esto es lo que impide que
/// un id como `../../.bashrc` o `/etc/algo` saque la escritura del directorio
/// de esquemas. Por eso se valida a mano y carácter por carácter, en ASCII: una
/// letra con tilde o una barra no pasan, y un punto tampoco, así que no hay
/// forma de armar `..`.
pub fn is_valid_scheme_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_SCHEME_ID_LEN {
        return false;
    }
    let is_alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    is_alnum(bytes[0]) && bytes[1..].iter().all(|&b| is_alnum(b) || b == b'-')
}

/// El directorio de esquemas «del usuario», dadas las rutas efectivas.
///
/// Es **la primera** de la lista: la que tiene prioridad al buscar un esquema
/// por id, así que un esquema guardado ahí es el que gana. Sin
/// `VASAK_SCHEMES_PATHS` es `~/.config/vasak/schemes`; con la variable, es su
/// primera entrada no vacía (el mismo criterio con que se la lee para buscar).
///
/// Se rechaza que caiga dentro de `/usr`: ahí viven los esquemas que instala
/// el sistema, y guardar encima de uno de ellos —aunque se tuviera permiso—
/// lo pisaría en la próxima actualización del paquete.
fn user_schemes_dir_from(paths: &[std::path::PathBuf]) -> crate::Result<std::path::PathBuf> {
    let dir = paths
        .first()
        .ok_or_else(|| crate::Error::Other("No schemes directory configured".to_string()))?;
    if dir.starts_with("/usr") {
        return Err(crate::Error::Other(format!(
            "Refusing to write user schemes into system directory {}",
            dir.display()
        )));
    }
    Ok(dir.clone())
}

/// Escribe `scheme` como `<id>.json` en `dir`, creándolo si falta.
///
/// Sin `self` para poder probarla contra un directorio temporal.
async fn write_user_scheme(
    dir: &std::path::Path,
    scheme: &SchemeData,
) -> crate::Result<std::path::PathBuf> {
    if !is_valid_scheme_id(&scheme.id) {
        return Err(crate::Error::Other(format!(
            "Invalid scheme id {:?}: it must match ^[a-z0-9][a-z0-9-]{{0,63}}$",
            scheme.id
        )));
    }

    tokio::fs::create_dir_all(dir).await.map_err(|e| {
        crate::Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to create schemes directory {}: {}",
                dir.display(),
                e
            ),
        ))
    })?;

    let path = dir.join(format!("{}.json", scheme.id));
    let mut content = serde_json::to_string_pretty(scheme).map_err(crate::Error::Json)?;
    // Con salto de línea al final, como los que instala el sistema: es un
    // archivo que se puede abrir y editar a mano.
    content.push('\n');
    write_file_atomically(&path, &content).await?;
    Ok(path)
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
        assert!(Manager::is_usable(completa));
    }

    #[test]
    fn una_configuracion_parcial_tambien_sirve() {
        // Los campos que faltan tienen `#[serde(default)]`: reponer el archivo
        // por esto sería tirar lo que la persona sí había elegido.
        assert!(Manager::is_usable(r#"{"style":{"darkmode":true}}"#));
        assert!(Manager::is_usable("{}"));
    }

    #[test]
    fn lo_que_no_parsea_no_sirve() {
        // El caso real: un archivo cortado por un apagón o editado a mano. Antes
        // esto devolvía el texto tal cual, la interfaz se quedaba sin colores ni
        // fuentes, y no se recuperaba nunca porque nada lo reescribía.
        assert!(!Manager::is_usable(
            r#"{"style":{"darkmode":true,"color-sch"#
        ));
        assert!(!Manager::is_usable(""));
        assert!(!Manager::is_usable("no soy json"));
        // Y un tipo que no corresponde: `radius` es un número.
        assert!(!Manager::is_usable(r#"{"style":{"radius":"diez"}}"#));
    }

    #[test]
    fn el_respaldo_va_al_lado_del_original() {
        let ruta = std::path::Path::new("/home/alguien/.config/vasak/vasak.conf");
        assert_eq!(
            Manager::backup_path(ruta),
            std::path::PathBuf::from("/home/alguien/.config/vasak/vasak.conf.roto")
        );
    }

    #[test]
    fn el_contenido_por_defecto_es_utilizable() {
        // Si no, reponer dejaría el archivo tan roto como estaba y el escritorio
        // entraría en un ciclo de reponer y volver a fallar.
        let contenido = Manager::default_content().expect("se serializa");
        assert!(Manager::is_usable(&contenido));
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
        let normalizado = Manager::normalize(sin_radio).expect("normaliza");
        let valor: serde_json::Value = serde_json::from_str(&normalizado).expect("parsea");

        assert_eq!(
            valor["style"]["radius"], 8,
            "el radio de fábrica, ya escrito"
        );
        assert_eq!(
            valor["style"]["darkmode"], true,
            "y lo que sí estaba se respeta"
        );
    }

    #[test]
    fn normalizar_no_se_come_los_widgets() {
        // El síntoma: acomodar los widgets del escritorio y después cambiar
        // cualquier cosa en Ajustes —el tema, la fuente— los devolvía a la
        // disposición de fábrica. `read_config` no devuelve el archivo, devuelve
        // este modelo reserializado, y el modelo no conoce `desktop.widgets`: la
        // clave desaparecía de lo que ve la interfaz, y el `writeConfig` que
        // viene después la borraba también del disco.
        let con_widgets = r#"{"style":{},
            "desktop":{"wallpaper":[],"iconsize":48,
                       "widgets":[{"id":"clock-1","type":"clock","x":0,"y":0,"w":2,"h":2}]},
            "panel":{"posicion":"abajo"}}"#;
        let normalizado = Manager::normalize(con_widgets).expect("normaliza");
        let valor: serde_json::Value = serde_json::from_str(&normalizado).expect("parsea");

        assert_eq!(
            valor["desktop"]["widgets"][0]["id"], "clock-1",
            "el widget acomodado sigue ahí"
        );
        assert_eq!(valor["desktop"]["widgets"][0]["w"], 2, "y con su tamaño");
        assert_eq!(
            valor["panel"]["posicion"], "abajo",
            "y cualquier otra sección que el modelo no conozca, también"
        );
        assert_eq!(
            valor["style"]["radius"], 8,
            "sin dejar de completar lo que falta"
        );
    }

    #[test]
    fn cambiar_el_tema_no_se_come_los_widgets() {
        // `set_darkmode` no reescribe el archivo tal cual: lo parsea a este
        // modelo y lo vuelve a serializar. Es el otro camino por el que se
        // perdían los widgets, y directo en el disco.
        let con_widgets = r#"{"style":{"darkmode":false},
            "desktop":{"widgets":[{"id":"clock-1","type":"clock","x":0,"y":0,"w":2,"h":2}]}}"#;
        let mut config: VSKConfig = serde_json::from_str(con_widgets).expect("parsea");

        config.style.darkmode = true;

        let escrito = serde_json::to_string_pretty(&config).expect("se serializa");
        let valor: serde_json::Value = serde_json::from_str(&escrito).expect("parsea");

        assert_eq!(valor["style"]["darkmode"], true, "el modo oscuro se aplicó");
        assert_eq!(
            valor["desktop"]["widgets"][0]["id"], "clock-1",
            "y los widgets siguen acomodados"
        );
    }

    #[test]
    fn una_seccion_a_medias_no_cuesta_el_archivo() {
        // A `fonts` le faltaban los valores de fábrica por campo: un archivo con
        // una sola fuente escrita no parseaba, y no parsear cuesta el archivo
        // entero —se aparta a `.roto` y se repone—, o sea el fondo de pantalla y
        // los widgets de quien lo tuviera así.
        let a_medias = r#"{"style":{},"fonts":{"terminal":"Fira Code"}}"#;
        assert!(Manager::is_usable(a_medias));

        let config: VSKConfig = serde_json::from_str(a_medias).expect("parsea");
        assert_eq!(config.fonts.terminal, "Fira Code");
        assert!(config.fonts.apps.is_empty(), "lo que falta queda vacío");
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
        let base =
            std::env::temp_dir().join(format!("config-manager-prueba-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("directorio de prueba");
        let ruta = base.join("vasak.conf");

        let roto = r#"{"style":{"darkmode":true,"color-sch"#;
        std::fs::write(&ruta, roto).expect("escribir el roto");

        let contenido = Manager::restore_default(&ruta).await.expect("repone");

        assert!(Manager::is_usable(&contenido));
        assert!(Manager::is_usable(
            &std::fs::read_to_string(&ruta).expect("leer el nuevo")
        ));

        // Y lo que había no se pierde: puede tener el fondo de pantalla, los
        // widgets y las fuentes que alguien eligió.
        let respaldo = std::fs::read_to_string(Manager::backup_path(&ruta))
            .expect("el respaldo tiene que estar");
        assert_eq!(respaldo, roto);

        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod scheme_tests {
    use super::*;
    use tauri::test::MockRuntime;

    /// El esquema que instala `vasak-desktop-settings`, copiado tal cual.
    ///
    /// Está acá y no se lee sólo de `/usr/share/schemes` porque en el CI ese
    /// directorio no existe: la prueba pasaría sin haber mirado nada.
    const VASAK_DEFAULT: &str = include_str!("../tests/fixtures/schemes/vasak-default.json");

    fn round_trip(json: &str) -> (serde_json::Value, serde_json::Value) {
        let original: serde_json::Value = serde_json::from_str(json).expect("el JSON parsea");
        let scheme: SchemeData = serde_json::from_str(json).expect("el esquema parsea");
        let back = serde_json::to_value(&scheme).expect("se serializa");
        (original, back)
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("config-manager-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn scheme_with_id(id: &str) -> SchemeData {
        let mut scheme: SchemeData = serde_json::from_str(VASAK_DEFAULT).expect("parsea");
        scheme.id = id.to_string();
        scheme
    }

    #[test]
    fn vasak_default_vuelve_igual_despues_de_leerlo_y_guardarlo() {
        // Antes `text.on-secondary` se perdía en el camino: `TextColors` no lo
        // declaraba, así que clonar el esquema en uso para editarlo lo borraba.
        let (original, back) = round_trip(VASAK_DEFAULT);
        assert_eq!(original, back);
        assert_eq!(
            back["colors"]["dark"]["ui"]["text"]["on-secondary"],
            "#1e1e2e"
        );
    }

    #[test]
    fn cada_esquema_instalado_vuelve_igual() {
        // Los que haya en la máquina, además del de la copia: si mañana se
        // instala uno con un campo nuevo, esto lo mira.
        let Ok(entries) = std::fs::read_dir("/usr/share/schemes") else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |ext| ext != "json") {
                continue;
            }
            let json = std::fs::read_to_string(&path).expect("se lee");
            let (original, back) = round_trip(&json);
            assert_eq!(original, back, "{}", path.display());
        }
    }

    #[test]
    fn un_campo_que_el_modelo_no_conoce_sobrevive_en_cualquier_nivel() {
        let mut value: serde_json::Value = serde_json::from_str(VASAK_DEFAULT).expect("parsea");
        value["license"] = "MIT".into();
        value["colors"]["contrast"] = "high".into();
        value["colors"]["dark"]["gtk"] = serde_json::json!({ "accent": "#ff0000" });
        value["colors"]["dark"]["ui"]["shadow"] = "#000000".into();
        value["colors"]["dark"]["ui"]["color"]["tertiary"] = "#00ff00".into();
        value["colors"]["dark"]["ui"]["text"]["on-surface"] = "#ffffff".into();
        value["colors"]["light"]["terminal"]["selection"] = "#cccccc".into();
        value["colors"]["light"]["terminal"]["ansi"]["orange"] = "#ff8800".into();

        let (original, back) = round_trip(&value.to_string());
        assert_eq!(original, back);
    }

    #[test]
    fn un_esquema_sin_on_secondary_no_lo_inventa() {
        let mut value: serde_json::Value = serde_json::from_str(VASAK_DEFAULT).expect("parsea");
        for variant in ["dark", "light"] {
            value["colors"][variant]["ui"]["text"]
                .as_object_mut()
                .expect("text es un objeto")
                .remove("on-secondary");
        }
        let (original, back) = round_trip(&value.to_string());
        assert_eq!(original, back, "no aparece un on-secondary nulo");
    }

    #[test]
    fn ids_validos() {
        for id in [
            "custom",
            "vasak-default",
            "a",
            "0",
            "9-lives",
            &"a".repeat(64),
        ] {
            assert!(is_valid_scheme_id(id), "{id:?}");
        }
    }

    #[test]
    fn ids_invalidos() {
        for id in [
            "",
            "-custom",
            "Custom",
            "mi esquema",
            "custom.json",
            "..",
            "../../.bashrc",
            "/etc/passwd",
            "a/b",
            "esquema_propio",
            "ñandú",
            &"a".repeat(65),
        ] {
            assert!(!is_valid_scheme_id(id), "{id:?}");
        }
    }

    #[test]
    fn el_directorio_del_usuario_es_el_primero() {
        let paths = vec![
            std::path::PathBuf::from("/home/alguien/.config/vasak/schemes"),
            std::path::PathBuf::from("/usr/share/schemes"),
        ];
        assert_eq!(
            user_schemes_dir_from(&paths).expect("hay uno"),
            std::path::PathBuf::from("/home/alguien/.config/vasak/schemes")
        );
    }

    #[test]
    fn nunca_se_escribe_en_usr() {
        // Si VASAK_SCHEMES_PATHS pone primero el del sistema, no se guarda
        // ahí: se falla.
        for first in ["/usr/share/schemes", "/usr/local/share/schemes"] {
            let paths = vec![std::path::PathBuf::from(first)];
            assert!(user_schemes_dir_from(&paths).is_err(), "{first}");
        }
        // Pero un directorio que sólo *empieza* con esas letras no es /usr.
        let paths = vec![std::path::PathBuf::from("/usrdata/schemes")];
        assert!(user_schemes_dir_from(&paths).is_ok());
    }

    #[test]
    fn sin_directorios_no_hay_directorio_del_usuario() {
        assert!(user_schemes_dir_from(&[]).is_err());
    }

    #[test]
    fn el_temporal_va_al_lado_oculto_y_no_termina_en_json() {
        let path = std::path::Path::new("/home/alguien/.config/vasak/schemes/custom.json");
        let a = temporary_path(path).expect("hay temporal");
        let b = temporary_path(path).expect("hay temporal");

        assert_eq!(
            a.parent(),
            path.parent(),
            "al lado, para que el rename sea atómico"
        );
        assert!(
            is_temporary_file(&a),
            "oculto, para que el vigilante no lo mire"
        );
        assert_ne!(a.extension().and_then(|e| e.to_str()), Some("json"));
        assert_ne!(a, b, "dos escrituras seguidas no comparten temporal");
        assert!(!is_temporary_file(path));
    }

    #[tokio::test]
    async fn write_user_scheme_crea_el_directorio_y_escribe_id_json() {
        let dir = temp_dir("escribir-esquema").join("vasak/schemes");
        let scheme = scheme_with_id("custom");

        let path = write_user_scheme(&dir, &scheme).await.expect("guarda");

        assert_eq!(path, dir.join("custom.json"));
        let written = std::fs::read_to_string(&path).expect("está");
        assert!(written.ends_with('\n'));
        let (original, back) = round_trip(&written);
        assert_eq!(original, back);
        assert_eq!(back["id"], "custom");

        // Sin temporales olvidados.
        let names: Vec<_> = std::fs::read_dir(&dir)
            .expect("se lee")
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("custom.json")]);

        let _ = std::fs::remove_dir_all(dir.parent().and_then(|p| p.parent()).expect("base"));
    }

    #[tokio::test]
    async fn write_user_scheme_reemplaza_el_que_habia() {
        let dir = temp_dir("reemplazar-esquema");
        let mut scheme = scheme_with_id("custom");
        write_user_scheme(&dir, &scheme).await.expect("guarda");

        scheme.colors.dark.ui.color.primary = "#123456".to_string();
        let path = write_user_scheme(&dir, &scheme)
            .await
            .expect("guarda otra vez");

        let back: SchemeData =
            serde_json::from_str(&std::fs::read_to_string(path).expect("está")).expect("parsea");
        assert_eq!(back.colors.dark.ui.color.primary, "#123456");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn write_user_scheme_rechaza_un_id_que_saldria_del_directorio() {
        let base = temp_dir("id-malo");
        let dir = base.join("schemes");
        let error = write_user_scheme(&dir, &scheme_with_id("../escapado"))
            .await
            .expect_err("no guarda");

        assert!(error.to_string().contains("Invalid scheme id"), "{error}");
        assert!(!base.join("escapado.json").exists());
        assert!(!dir.exists(), "ni siquiera crea el directorio");
    }

    #[tokio::test]
    async fn save_user_scheme_guarda_y_la_siguiente_lectura_lo_ve() {
        // La única prueba que toca VASAK_SCHEMES_PATHS: ninguna otra la lee,
        // así que correr en paralelo no las cruza.
        let base = temp_dir("guardar-y-leer");
        let user = base.join("user");
        let system = base.join("system");
        std::fs::create_dir_all(&system).expect("directorio del sistema");
        std::fs::write(system.join("vasak-default.json"), VASAK_DEFAULT)
            .expect("esquema del sistema");
        std::env::set_var(
            "VASAK_SCHEMES_PATHS",
            std::env::join_paths([&user, &system]).expect("se juntan"),
        );

        let app = tauri::test::mock_app();
        let manager = ConfigManager::<MockRuntime>::new(app.handle().clone());

        // Llena el caché con lo que hay antes de guardar.
        let before = manager.load_schemes().await.expect("carga");
        assert_eq!(before.len(), 1);

        let mut custom = scheme_with_id("custom");
        custom.name = "Personalizado".to_string();
        let saved = manager.save_user_scheme(custom).await.expect("guarda");
        assert_eq!(saved.path, user.join("custom.json").to_string_lossy());
        assert!(
            !system.join("custom.json").exists(),
            "nunca en el del sistema"
        );

        // Sin invalidar, esto devolvería la lista vieja durante media hora.
        let found = manager
            .get_scheme_by_id("custom")
            .await
            .expect("busca")
            .expect("lo encuentra");
        assert_eq!(found.scheme.name, "Personalizado");
        assert_eq!(found.path, saved.path);

        // Un id del sistema guardado por el usuario lo tapa, sin tocar el otro.
        let mut edited = scheme_with_id("vasak-default");
        edited.colors.light.ui.color.primary = "#abcdef".to_string();
        manager.save_user_scheme(edited).await.expect("guarda");
        let found = manager
            .get_scheme_by_id("vasak-default")
            .await
            .expect("busca")
            .expect("lo encuentra");
        assert_eq!(found.scheme.colors.light.ui.color.primary, "#abcdef");
        assert_eq!(
            std::fs::read_to_string(system.join("vasak-default.json")).expect("sigue"),
            VASAK_DEFAULT
        );

        std::env::remove_var("VASAK_SCHEMES_PATHS");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn invalidar_sube_la_generacion_y_vacia_el_cache() {
        let app = tauri::test::mock_app();
        let manager = ConfigManager::<MockRuntime>::new(app.handle().clone());

        let before = manager.schemes_generation.load(Ordering::SeqCst);
        manager.invalidate_schemes_cache().await;
        assert_eq!(
            manager.schemes_generation.load(Ordering::SeqCst),
            before + 1
        );
        assert!(manager.schemes_cache.read().await.is_none());
    }
}
