use notify::{EventKind, RecommendedWatcher, Watcher};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Emitter, Manager, Runtime,
};

mod commands;
#[cfg(desktop)]
mod desktop;
mod error;
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

fn should_handle_event(event: &notify::Event, watched_file_path: &Path) -> bool {
    let is_relevant_kind = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_)
    );

    is_relevant_kind && event.paths.iter().any(|path| path == watched_file_path)
}

fn watch_config_file<R: Runtime + 'static>(
    app: &tauri::AppHandle<R>,
    watched_file_path: std::path::PathBuf,
) -> Box<dyn FnMut(notify::Result<notify::Event>) + Send + 'static> {
    let app_handle = app.clone();
    let debounce_window = Duration::from_millis(250);
    let last_refresh = Arc::new(Mutex::new(None::<Instant>));
    Box::new(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else {
            if let Err(e) = res {
                eprintln!(
                    "[Config Watcher Callback] Error watching config file: {:?}",
                    e
                );
            }
            return;
        };

        if should_handle_event(&event, watched_file_path.as_path()) {
            let lock_state = last_refresh.lock();
            let Ok(mut last_refresh_at) = lock_state else {
                eprintln!("[Config Watcher Callback] Debounce mutex poisoned");
                return;
            };

            if last_refresh_at
                .as_ref()
                .map(|at| at.elapsed() < debounce_window)
                .unwrap_or(false)
            {
                return;
            }

            *last_refresh_at = Some(Instant::now());

            // Refrescar el caché del plugin leyendo de disco y luego emitir el evento.
            let app_for_async = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // Obtener el estado del ConfigManager y actualizar su cache.
                let state = app_for_async.state::<desktop::ConfigManager<R>>();
                if let Err(e) = state.inner().refresh_cache_from_file().await {
                    eprintln!(
                        "[Config Watcher Callback] Failed to refresh config cache: {}",
                        e
                    );
                }
                // Emitir evento para frontends
                app_for_async
                    .emit(CONFIG_CHANGED_EVENT, ())
                    .unwrap_or_else(|e| {
                        eprintln!(
                            "[Config Watcher Callback] Failed to emit config-changed event: {}",
                            e
                        );
                    });
            });
        }
    })
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("config-manager")
        .invoke_handler(tauri::generate_handler![
            commands::read_config,
            commands::write_config,
            commands::set_darkmode,
            commands::get_schemes,
            commands::get_scheme_by_id
        ])
        .setup(|app, api| {
            let config_manager = desktop::init(app, api)?;
            let config_path = config_manager.config_path()?;
            app.manage(config_manager);

            let watch_target = if config_path.exists() {
                config_path.clone()
            } else {
                config_path
                    .parent()
                    .map(std::path::Path::to_path_buf)
                    .ok_or_else(|| {
                        Error::Other(format!(
                            "Invalid config path without parent: {}",
                            config_path.display()
                        ))
                    })?
            };

            let app_handle_for_watcher = app.clone();
            let event_handler = watch_config_file(&app_handle_for_watcher, config_path.clone());

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

            app.manage(Mutex::new(watcher));

            Ok(())
        })
        .build()
}
