use notify::{EventKind, RecommendedWatcher, Watcher};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Emitter, Manager, Runtime,
};

mod commands;
#[cfg(desktop)]
mod desktop;
mod error;
#[cfg(all(desktop, feature = "system-theme-sync"))]
mod gtk_settings;
mod models;

pub use error::{Error, Result};
pub use models::*;

#[cfg(desktop)]
use desktop::ConfigManager;

pub const CONFIG_CHANGED_EVENT: &str = "config-changed";

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the config-manager APIs.
pub trait ConfigManagerExt<R: Runtime> {
    fn config_manager(&self) -> &ConfigManager<R>;
}

impl<R: Runtime, T: Manager<R>> crate::ConfigManagerExt<R> for T {
    fn config_manager(&self) -> &ConfigManager<R> {
        self.state::<ConfigManager<R>>().inner()
    }
}

/// Qué hay que refrescar por lo que vio el vigilante.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Refresh {
    /// Cambió `vasak.conf`: releerlo al caché.
    config: bool,
    /// Cambió un esquema del usuario: vaciar `schemes_cache`.
    schemes: bool,
}

impl Refresh {
    fn is_empty(self) -> bool {
        !self.config && !self.schemes
    }
}

/// Si el evento es un cambio de `vasak.conf`.
///
/// Por nombre **y** directorio: el vigilante mira también el de esquemas, y un
/// archivo que se llamara igual ahí no es la configuración.
fn is_config_event(event: &notify::Event, config_file_path: &Path) -> bool {
    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
        return false;
    }
    let file_name = config_file_path.file_name();
    let parent = config_file_path.parent();
    event
        .paths
        .iter()
        .any(|path| path.file_name() == file_name && path.parent() == parent)
}

/// Si el evento es un cambio de un esquema del directorio del usuario.
///
/// A diferencia de la configuración, acá cuenta también **borrar**: si alguien
/// borra `custom.json` a mano, las aplicaciones que lo tenían aplicado tienen
/// que enterarse y caer al de por defecto. Renombrar llega como `Modify(Name)`,
/// que también entra.
///
/// Se ignoran los temporales de la escritura atómica —empiezan con punto y no
/// terminan en `.json`—: el cambio que importa es el `rename` al nombre final,
/// que llega en su propio evento.
fn is_user_scheme_event(event: &notify::Event, user_schemes_dir: &Path) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    event.paths.iter().any(|path| {
        path.parent() == Some(user_schemes_dir)
            && path.extension().is_some_and(|ext| ext == "json")
            && !desktop::is_temporary_file(path)
    })
}

fn classify_event(
    event: &notify::Event,
    config_file_path: &Path,
    user_schemes_dir: Option<&Path>,
) -> Refresh {
    Refresh {
        config: is_config_event(event, config_file_path),
        schemes: user_schemes_dir.is_some_and(|dir| is_user_scheme_event(event, dir)),
    }
}

/// Lo que quedó por refrescar mientras corre el antirrebote.
///
/// Compartido entre la configuración y los esquemas: una ráfaga que toque los
/// dos —Ajustes cambia el esquema elegido y lo guarda— sale como **un**
/// `config-changed`, no dos.
#[derive(Default)]
struct PendingRefresh {
    config: AtomicBool,
    schemes: AtomicBool,
    scheduled: AtomicBool,
}

impl PendingRefresh {
    /// Anota lo que hay que refrescar. Devuelve `true` si es el primero de la
    /// ráfaga y hay que programar el refresco; los siguientes sólo se suman.
    fn mark(&self, refresh: Refresh) -> bool {
        if refresh.config {
            self.config.store(true, Ordering::SeqCst);
        }
        if refresh.schemes {
            self.schemes.store(true, Ordering::SeqCst);
        }
        !self.scheduled.swap(true, Ordering::SeqCst)
    }

    /// Lo que se juntó durante la ráfaga, y deja todo listo para la próxima.
    ///
    /// Se libera la programación **antes** de mirar las marcas: un evento que
    /// llegue desde acá programa otro refresco, y ninguno se pierde.
    fn take(&self) -> Refresh {
        self.scheduled.store(false, Ordering::SeqCst);
        Refresh {
            config: self.config.swap(false, Ordering::SeqCst),
            schemes: self.schemes.swap(false, Ordering::SeqCst),
        }
    }
}

/// Cuánto se espera a que termine una ráfaga de eventos antes de refrescar.
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(250);

fn watch_config_file<R: Runtime + 'static>(
    app: &tauri::AppHandle<R>,
    config_file_path: std::path::PathBuf,
    user_schemes_dir: Option<std::path::PathBuf>,
) -> Box<dyn FnMut(notify::Result<notify::Event>) + Send + 'static> {
    let app_handle = app.clone();
    let pending = Arc::new(PendingRefresh::default());
    Box::new(move |res: notify::Result<notify::Event>| {
        let event = match res {
            Ok(event) => event,
            Err(e) => {
                tracing::error!("Error watching config file: {:?}", e);
                return;
            }
        };

        let refresh = classify_event(&event, &config_file_path, user_schemes_dir.as_deref());
        if refresh.is_empty() {
            return;
        }
        // El antirrebote va por el **final** de la ráfaga, no por el principio.
        // Antes se atendía el primer evento y se tiraban los que llegaran en los
        // 250 ms siguientes, y con eso el último estado podía no llegar nunca:
        // un editor que guarda mientras se arrastra un color dejaba a las demás
        // aplicaciones con un color intermedio. Ahora el primer evento programa
        // el refresco, los siguientes se suman, y el refresco lee el disco
        // cuando la ráfaga ya pasó.
        if !pending.mark(refresh) {
            return;
        }

        let app_for_async = app_handle.clone();
        let pending = Arc::clone(&pending);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(DEBOUNCE_WINDOW).await;
            let refresh = pending.take();
            if refresh.is_empty() {
                return;
            }

            let state = app_for_async.state::<desktop::ConfigManager<R>>();
            if refresh.config {
                if let Err(e) = state.inner().refresh_cache_from_file().await {
                    tracing::error!("Failed to refresh config cache: {}", e);
                }
            }
            if refresh.schemes {
                state.inner().invalidate_schemes_cache().await;
            }
            // El mismo evento para los dos casos: las aplicaciones ya recargan
            // la configuración y reaplican el esquema al recibirlo, y un esquema
            // editado con el mismo id no cambia `vasak.conf`, así que sin esto
            // nadie se enteraba.
            app_for_async
                .emit(CONFIG_CHANGED_EVENT, ())
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to emit config-changed event: {}", e);
                });
        });
    })
}

/// Crea el directorio de esquemas del usuario para poder vigilarlo.
///
/// `inotify` no puede vigilar un directorio que no existe, y sin vigilarlo el
/// primer esquema que se guarde no le llega a nadie. Si no se puede, se sigue
/// sin vigilarlo: la configuración tiene que andar igual.
fn prepare_user_schemes_dir(dir: crate::Result<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    let dir = match dir {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!("No user schemes directory to watch: {}", e);
            return None;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(
            "Could not create user schemes directory {}: {}",
            dir.display(),
            e
        );
        return None;
    }
    Some(dir)
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("config-manager")
        .invoke_handler(tauri::generate_handler![
            commands::read_config,
            commands::write_config,
            commands::set_darkmode,
            commands::get_schemes,
            commands::get_scheme_by_id,
            commands::save_user_scheme
        ])
        .setup(|app, api| {
            let config_manager = desktop::init(app, api)?;
            let config_path = config_manager.config_path()?;
            let user_schemes_dir = prepare_user_schemes_dir(config_manager.user_schemes_dir());
            app.manage(config_manager);

            let watch_target = config_path
                .parent()
                .map(std::path::Path::to_path_buf)
                .ok_or_else(|| {
                    Error::Other(format!(
                        "Invalid config path without parent: {}",
                        config_path.display()
                    ))
                })?;

            let app_handle_for_watcher = app.clone();
            let event_handler = watch_config_file(
                &app_handle_for_watcher,
                config_path.clone(),
                user_schemes_dir.clone(),
            );

            let mut watcher: RecommendedWatcher = notify::recommended_watcher(event_handler)
                .map_err(|e| {
                    Error::Other(format!("Cannot create watcher for config file: {}", e))
                })?;

            watcher
                .watch(watch_target.as_path(), notify::RecursiveMode::NonRecursive)
                .map_err(|e| {
                    Error::Other(format!(
                        "Failed to watch config path {}: {}",
                        watch_target.display(),
                        e
                    ))
                })?;

            // El de esquemas no es fatal: sin él, la configuración anda igual y
            // sólo se pierde enterarse al vuelo de un esquema editado.
            if let Some(dir) = user_schemes_dir.filter(|dir| dir != &watch_target) {
                if let Err(e) = watcher.watch(dir.as_path(), notify::RecursiveMode::NonRecursive) {
                    tracing::warn!("Failed to watch schemes path {}: {}", dir.display(), e);
                }
            }

            app.manage(Mutex::new(watcher));

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod watcher_tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind, RenameMode};
    use notify::Event;
    use std::path::PathBuf;

    fn config() -> PathBuf {
        PathBuf::from("/home/alguien/.config/vasak/vasak.conf")
    }

    fn schemes() -> PathBuf {
        PathBuf::from("/home/alguien/.config/vasak/schemes")
    }

    fn event(kind: EventKind, path: &str) -> Event {
        Event::new(kind).add_path(PathBuf::from(path))
    }

    fn classify(event: &Event) -> Refresh {
        classify_event(event, &config(), Some(&schemes()))
    }

    #[test]
    fn modificar_vasak_conf_refresca_la_configuracion() {
        let e = event(
            EventKind::Modify(ModifyKind::Any),
            "/home/alguien/.config/vasak/vasak.conf",
        );
        assert_eq!(
            classify(&e),
            Refresh {
                config: true,
                schemes: false
            }
        );
    }

    #[test]
    fn el_rename_de_la_escritura_atomica_de_vasak_conf_cuenta() {
        // El temporal se renombra al final: llega como Modify(Name) con los
        // dos caminos, y el de destino es vasak.conf.
        let e = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(PathBuf::from(
                "/home/alguien/.config/vasak/.vasak.conf.tmp-1-2-3",
            ))
            .add_path(config());
        assert!(classify(&e).config);
    }

    #[test]
    fn el_temporal_de_vasak_conf_no_refresca_nada() {
        let e = event(
            EventKind::Create(CreateKind::File),
            "/home/alguien/.config/vasak/.vasak.conf.tmp-1-2-3",
        );
        assert!(classify(&e).is_empty());
    }

    #[test]
    fn guardar_un_esquema_del_usuario_vacia_el_cache_de_esquemas() {
        for kind in [
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Any),
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
        ] {
            let e = event(kind, "/home/alguien/.config/vasak/schemes/custom.json");
            assert_eq!(
                classify(&e),
                Refresh {
                    config: false,
                    schemes: true
                },
                "{kind:?}"
            );
        }
    }

    #[test]
    fn borrar_un_esquema_del_usuario_tambien_cuenta() {
        // Quien lo tenía aplicado tiene que enterarse y caer al de por defecto.
        let e = event(
            EventKind::Remove(RemoveKind::File),
            "/home/alguien/.config/vasak/schemes/custom.json",
        );
        assert!(classify(&e).schemes);
    }

    #[test]
    fn borrar_vasak_conf_no_refresca_la_configuracion() {
        // Como antes: releer un archivo que no está lo recrearía por defecto
        // en medio de, por ejemplo, un reemplazo hecho a mano.
        let e = event(
            EventKind::Remove(RemoveKind::File),
            "/home/alguien/.config/vasak/vasak.conf",
        );
        assert!(classify(&e).is_empty());
    }

    #[test]
    fn el_temporal_de_un_esquema_no_refresca_nada() {
        let e = event(
            EventKind::Create(CreateKind::File),
            "/home/alguien/.config/vasak/schemes/.custom.json.tmp-1-2-3",
        );
        assert!(classify(&e).is_empty());
    }

    #[test]
    fn un_archivo_que_no_es_json_en_esquemas_no_cuenta() {
        let e = event(
            EventKind::Create(CreateKind::File),
            "/home/alguien/.config/vasak/schemes/notas.txt",
        );
        assert!(classify(&e).is_empty());
    }

    #[test]
    fn un_json_fuera_del_directorio_de_esquemas_no_cuenta() {
        // Ni en el directorio de la configuración ni en uno de adentro.
        for path in [
            "/home/alguien/.config/vasak/otro.json",
            "/home/alguien/.config/vasak/schemes/viejos/custom.json",
        ] {
            let e = event(EventKind::Create(CreateKind::File), path);
            assert!(classify(&e).is_empty(), "{path}");
        }
    }

    #[test]
    fn un_vasak_conf_en_el_directorio_de_esquemas_no_es_la_configuracion() {
        let e = event(
            EventKind::Modify(ModifyKind::Any),
            "/home/alguien/.config/vasak/schemes/vasak.conf",
        );
        assert!(classify(&e).is_empty());
    }

    #[test]
    fn leer_un_archivo_no_refresca_nada() {
        // Cada aplicación lee los esquemas al arrancar: si leer disparara un
        // config-changed, se dispararían entre ellas sin parar.
        for path in [
            "/home/alguien/.config/vasak/vasak.conf",
            "/home/alguien/.config/vasak/schemes/custom.json",
        ] {
            let e = event(EventKind::Access(AccessKind::Any), path);
            assert!(classify(&e).is_empty(), "{path}");
        }
    }

    #[test]
    fn sin_directorio_de_esquemas_solo_se_mira_la_configuracion() {
        let e = event(
            EventKind::Create(CreateKind::File),
            "/home/alguien/.config/vasak/schemes/custom.json",
        );
        assert!(classify_event(&e, &config(), None).is_empty());
    }

    #[test]
    fn una_rafaga_programa_un_solo_refresco_con_todo_lo_que_junto() {
        let pending = PendingRefresh::default();
        let config = Refresh {
            config: true,
            schemes: false,
        };
        let schemes = Refresh {
            config: false,
            schemes: true,
        };

        assert!(pending.mark(schemes), "el primero programa");
        assert!(!pending.mark(schemes), "los siguientes se suman");
        assert!(!pending.mark(config));

        assert_eq!(
            pending.take(),
            Refresh {
                config: true,
                schemes: true
            }
        );
    }

    #[test]
    fn despues_de_refrescar_el_siguiente_evento_programa_otro() {
        let pending = PendingRefresh::default();
        let schemes = Refresh {
            config: false,
            schemes: true,
        };

        assert!(pending.mark(schemes));
        pending.take();
        assert!(pending.mark(schemes), "no queda trabado como programado");
        assert_eq!(pending.take(), schemes);
        assert!(pending.take().is_empty(), "y no repite lo que ya refrescó");
    }

    #[test]
    fn un_evento_que_llega_mientras_se_refresca_no_se_pierde() {
        // El caso del arrastre de un color: el refresco en curso ya tomó lo
        // suyo, y el guardado siguiente tiene que programar uno nuevo en vez de
        // quedar colgado de uno que ya terminó de leer.
        let pending = PendingRefresh::default();
        let schemes = Refresh {
            config: false,
            schemes: true,
        };

        assert!(pending.mark(schemes));
        let first = pending.take();
        assert_eq!(first, schemes);
        assert!(pending.mark(schemes), "programa otro");
        assert_eq!(pending.take(), schemes, "con el último estado");
    }

    #[test]
    fn prepare_crea_el_directorio_que_falta() {
        let base =
            std::env::temp_dir().join(format!("config-manager-vigilante-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("vasak/schemes");

        assert_eq!(prepare_user_schemes_dir(Ok(dir.clone())), Some(dir.clone()));
        assert!(
            dir.is_dir(),
            "el directorio tiene que existir para vigilarlo"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn prepare_sin_directorio_no_vigila_esquemas() {
        let error = Err(Error::Other("sin directorio".into()));
        assert_eq!(prepare_user_schemes_dir(error), None);
    }
}
