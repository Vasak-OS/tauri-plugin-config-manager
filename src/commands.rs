use tauri::{command, AppHandle, Runtime};

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
