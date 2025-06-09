use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<ConfigManager<R>> {
    Ok(ConfigManager(app.clone()))
}

/// Access to the config-manager APIs.
pub struct ConfigManager<R: Runtime>(AppHandle<R>);

impl<R: Runtime> ConfigManager<R> {
    fn home_dir() -> std::path::PathBuf {
        dirs_next::home_dir().expect("No se pudo obtener el directorio home del usuario")
    }

    pub async fn read_config(&self) -> crate::Result<String> {
        let config_path = self.config_path();
        let config_content = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to read config file {}: {}", config_path.display(), e),
            )))?;
        Ok(config_content)
    }

    pub async fn write_config(&self, config: &str) -> crate::Result<()> {
        let config_path = self.config_path();
        tokio::fs::write(&config_path, config)
            .await
            .map_err(|e| crate::Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to write to config file {}: {}", config_path.display(), e),
            )))?;
        Ok(())
    }

    pub fn config_path(&self) -> std::path::PathBuf {
        Self::home_dir().join(".config/vasak/vasak.conf")
    }
}
