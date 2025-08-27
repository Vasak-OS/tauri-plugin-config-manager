use notify::{
    event::{DataChange, ModifyKind},
    EventKind, RecommendedWatcher, Watcher,
};
use std::sync::Mutex;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Emitter, Manager, Runtime,
};

mod commands;
#[cfg(desktop)]
mod desktop;
mod error;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::ConfigManager;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the config-manager APIs.
pub trait ConfigManagerExt<R: Runtime> {
    fn config_manager(&self) -> &ConfigManager<R>;
}

impl<R: Runtime, T: Manager<R>> crate::ConfigManagerExt<R> for T {
    fn config_manager(&self) -> &ConfigManager<R> {
        self.state::<ConfigManager<R>>().inner()
    }
}

fn watch_config_file<R: Runtime + 'static>(
    app: &tauri::AppHandle<R>,
) -> Box<dyn FnMut(notify::Result<notify::Event>) + Send + 'static> {
    let app_handle = app.clone();
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

        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Data(DataChange::Any))
        ) {
            app_handle.emit("config-changed", ()).unwrap_or_else(|e| {
                eprintln!(
                    "[Config Watcher Callback] Failed to emit config-changed event: {}",
                    e
                );
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
            commands::set_darkmode
        ])
        .setup(|app, api| {
            let config_manager = desktop::init(app, api)?;
            let config_path = config_manager.config_path().to_path_buf();
            app.manage(config_manager);

            let app_handle_for_watcher = app.clone();
            let event_handler = watch_config_file(&app_handle_for_watcher);

            let mut watcher: RecommendedWatcher = notify::recommended_watcher(event_handler)
                .map_err(|e| {
                    Error::Other(format!("Cannot create watcher for config file: {}", e))
                })?;

            watcher
                .watch(config_path.as_path(), notify::RecursiveMode::Recursive)
                .map_err(|e| {
                    Error::Other(format!(
                        "Failed to watch config file {}: {}",
                        config_path.display(),
                        e
                    ))
                })?;

            app.manage(Mutex::new(watcher));

            Ok(())
        })
        .build()
}
