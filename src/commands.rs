use tauri::{AppHandle, command, Runtime};

use crate::Result;
use crate::ConfigManagerExt;

// remember to call `.manage(MyState::default())`
#[tauri::command]
pub async fn read_config<R: Runtime>(app: AppHandle<R>) -> Result<String> {
   app.config_manager().read_config().await
}