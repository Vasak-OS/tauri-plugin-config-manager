use tauri::{command, AppHandle, Runtime};

use crate::models::{Scheme, SchemeData};
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

#[command]
pub async fn get_scheme_by_id<R: Runtime>(
    app: AppHandle<R>,
    scheme_id: String,
) -> Result<Option<Scheme>> {
    app.config_manager().get_scheme_by_id(&scheme_id).await
}

/// Guarda un esquema en el directorio de esquemas del usuario.
///
/// Su permiso, `allow-save-user-scheme`, **no** está en el conjunto por
/// defecto: escribir esquemas no es algo que toda aplicación deba poder, así
/// que la que lo necesite lo declara en su capability.
#[command]
pub async fn save_user_scheme<R: Runtime>(app: AppHandle<R>, scheme: SchemeData) -> Result<Scheme> {
    app.config_manager().save_user_scheme(scheme).await
}
