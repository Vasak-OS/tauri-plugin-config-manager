use tauri::{command, AppHandle, Runtime};

use crate::models::Scheme;
use crate::ConfigManagerExt;
use crate::Result;

#[command]
pub(crate) async fn write_config<R: Runtime>(app: AppHandle<R>, payload: String) -> Result<()> {
    app.config_manager().write_config(&payload).await
}

// remember to call `.manage(MyState::default())`
#[command]
pub async fn read_config<R: Runtime>(app: AppHandle<R>) -> Result<String> {
    app.config_manager().read_config().await
}

#[command]
pub async fn set_darkmode<R: Runtime>(app: AppHandle<R>, darkmode: bool) -> Result<()> {
    app.config_manager().set_darkmode(darkmode).await
}

#[command]
pub async fn get_schemes<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Scheme>> {
    app.config_manager().load_schemes().await
}
