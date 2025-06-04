use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

#[cfg(desktop)]
mod desktop;
mod commands;
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

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("config-manager")
    .invoke_handler(tauri::generate_handler![commands::read_config])
    .setup(|app, api| {
      #[cfg(desktop)]
      let config_manager = desktop::init(app, api)?;
      app.manage(config_manager);
      Ok(())
    })
    .build()
}
