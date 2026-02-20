const COMMANDS: &[&str] = &["read_config", "write_config", "set_darkmode", "get_schemes", "get_scheme_by_id"];

fn main() {
  tauri_plugin::Builder::new(COMMANDS)
    .android_path("android")
    .ios_path("ios")
    .build();
}
