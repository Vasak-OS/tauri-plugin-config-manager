# @vasakgroup/plugin-config-manager

Plugin de Tauri para persistir la configuración de Vasak, leerla desde el frontend y reaccionar a cambios externos del archivo. El paquete está pensado para publicarse y consumirse desde npm, con una API simple para Vue 3 + Pinia, aunque también puede usarse sin el store.

## Instalación

### Frontend

```bash
npm install @vasakgroup/plugin-config-manager
```

O con Bun:

```bash
bun add @vasakgroup/plugin-config-manager
```

### Backend Tauri

En tu proyecto Rust agrega el plugin y registra la inicialización:

```toml
[dependencies]
tauri-plugin-config-manager = { git = "https://github.com/Vasak-OS/tauri-plugin-config-manager" }
```

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_config_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Uso rápido

### Con el store incluido

```ts
import { onMounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import { useConfigStore } from "@vasakgroup/plugin-config-manager";

const configStore = useConfigStore();
let unlistenConfig: null | (() => void) = null;

onMounted(async () => {
  await configStore.loadConfig();

  unlistenConfig = await listen("config-changed", async () => {
    await configStore.loadConfig();
  });
});
```

### Sin el store

```ts
import {
  readConfig,
  writeConfig,
  setDarkMode,
  getSchemes,
  getSchemeById,
} from "@vasakgroup/plugin-config-manager";

const config = await readConfig();
await writeConfig({
  style: {
    darkmode: true,
    "color-scheme": "vasak-default",
    radius: 8,
  },
  desktop: {
    wallpaper: [],
    iconsize: 48,
    showfiles: true,
    showhiddenfiles: false,
  },
  fonts: {
    termina: "JetBrains Mono",
    title: "Inter",
    apps: "Noto Sans",
  },
});
```

## API pública

### `readConfig(): Promise<VSKConfig | null>`
Lee el archivo de configuración y lo parsea como JSON.

### `writeConfig(value: VSKConfig): Promise<void>`
Guarda la configuración completa.

### `setDarkMode(darkmode: boolean): Promise<void>`
Actualiza el modo oscuro en la configuración y, cuando corresponde, intenta sincronizar el tema del sistema.

### `getSchemes(): Promise<Scheme[]>`
Lista todos los esquemas disponibles.

### `getSchemeById(schemeId: string): Promise<Scheme | null>`
Busca un esquema por ID.

### `useConfigStore()`
Store de Pinia que carga la configuración y aplica las variables visuales del tema.

## Esquema de configuración

```ts
export type VSKConfig = {
  style: {
    darkmode: boolean;
    "color-scheme": string;
    radius: number;
  };
  desktop: {
    wallpaper: string[];
    iconsize: number;
    showfiles: boolean;
    showhiddenfiles: boolean;
  };
  fonts: {
    termina: string;
    title: string;
    apps: string;
  };
};
```

Ejemplo completo:

```json
{
  "style": {
    "darkmode": false,
    "color-scheme": "vasak-default",
    "radius": 8
  },
  "desktop": {
    "wallpaper": [],
    "iconsize": 48,
    "showfiles": true,
    "showhiddenfiles": false
  },
  "fonts": {
    "termina": "JetBrains Mono",
    "title": "Inter",
    "apps": "Noto Sans"
  }
}
```

## Compatibilidad y runtime

- Linux es la plataforma principal soportada.
- `setDarkMode` detecta el backend desktop usando `XDG_CURRENT_DESKTOP` y `DESKTOP_SESSION`.
- La sincronización del tema del sistema solo se intenta en GNOME.
- Si `gsettings` no está disponible, la persistencia de configuración sigue funcionando.
- La ruta del archivo de configuración puede sobrescribirse con `VASAK_CONFIG_PATH`.
- La búsqueda de schemes puede sobrescribirse con `VASAK_SCHEMES_PATHS`.

Prioridad de búsqueda de schemes:

1. Orden definido en `VASAK_SCHEMES_PATHS` si existe.
2. Orden por defecto: `~/.config/vasak/schemes` y luego `/usr/share/vasak-schemes`.

## Feature Flags

El crate Rust expone la feature `system-theme-sync`.

- `system-theme-sync` habilitada por defecto.
- Deshabilítala si no quieres sincronizar el tema del sistema.

```toml
[dependencies]
tauri-plugin-config-manager = { git = "https://github.com/Vasak-OS/tauri-plugin-config-manager", default-features = false }
```

## Eventos

El plugin emite el evento `config-changed` cuando el archivo de configuración se actualiza.

## Licencia

GPL-3.0-or-later
