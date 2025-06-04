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
        let config_path = Self::home_dir().join(".config/vasak/vasak.conf");
        let config_path = config_path.to_str().ok_or_else(|| {
            crate::Error::Other("No se pudo convertir la ruta del archivo de configuración a una cadena.".to_string())
        })?;
        let config = tokio::fs::read_to_string(config_path)
            .await
            .map_err(|e| crate::Error::Io(e))?;
        Ok(serde_json::from_str(&config).map_err(|e| crate::Error::Json(e))?)
    }
}
