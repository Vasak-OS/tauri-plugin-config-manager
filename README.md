# Tauri Plugin config-manager

Un plugin de Tauri para gestionar la configuración de la aplicación de forma persistente. Permite leer y escribir un archivo de configuración y notifica a la aplicación cuando el archivo cambia externamente.

## Plataformas Soportadas

- [ ] Windows
- [ ] macOS
- [x] Linux

El archivo de configuración se almacena en: `~/.config/vasak/vasak.conf`

## Instalación

Añade lo siguiente a tu `Cargo.toml`:

```toml
[dependencies]
tauri-plugin-config-manager = { git = "https://github.com/Vasak-OS/tauri-plugin-config-manager" } # O la versión de crates.io si está publicado
```

Y registra el plugin en tu `main.rs`:

```rust
// src-tauri/src/main.rs
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_config_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Instala la libreria del cliente:

```bash
bun install @vasak-group/plugin-config-manager
```

## Uso

El plugin expone funciones para interactuar con la configuración desde el frontend.

`App.vue`

```vue
<script lang="ts" setup>
import { listen } from "@tauri-apps/api/event";
import { useConfigStore } from "@vasak-group/plugin-config-manager";

const configStore = useConfigStore();
let unlistenConfig: Function | null = null;

onMounted(async () => {
  configStore.loadConfig();
  unlistenConfig = await listen("config-changed", async () => {
    configStore.loadConfig();
  });
});

onUnmounted(() => {
  if (unlistenConfig !== null) {
    unlistenConfig();
  }
});
</script>
```

`style.css`

```css
:root {
  --primary-color: #4caf50;
  --border-radius: 4px;
}
```
