use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::ConfigManager;
#[cfg(mobile)]
use mobile::ConfigManager;

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
    .invoke_handler(tauri::generate_handler![commands::ping])
    .setup(|app, api| {
      #[cfg(mobile)]
      let config_manager = mobile::init(app, api)?;
      #[cfg(desktop)]
      let config_manager = desktop::init(app, api)?;
      app.manage(config_manager);
      Ok(())
    })
    .build()
}
