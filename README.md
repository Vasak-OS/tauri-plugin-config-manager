# @vasakgroup/plugin-config-manager

[![npm version](https://img.shields.io/npm/v/@vasakgroup/plugin-config-manager?logo=npm&label=npm)](https://www.npmjs.com/package/@vasakgroup/plugin-config-manager)
[![npm downloads](https://img.shields.io/npm/dm/@vasakgroup/plugin-config-manager?logo=npm&label=downloads)](https://www.npmjs.com/package/@vasakgroup/plugin-config-manager)
[![license](https://img.shields.io/badge/license-LGPL--3.0--or--later-blue.svg)](https://www.gnu.org/licenses/lgpl-3.0.html)
[![tauri](https://img.shields.io/badge/built%20for-Tauri%20v2-24c8db)](https://tauri.app/)

Plugin de Tauri para persistir la configuración de Vasak, leerla desde el frontend y reaccionar a cambios externos del archivo.

Pensado para npm, con una API limpia para Vue 3 + Pinia y una superficie mínima para consumirlo también sin store.

## Lo que resuelve

| Capacidad | Qué hace |
| --- | --- |
| Persistencia | Guarda y lee configuración sin lógica duplicada en el frontend. |
| Sincronización | Emite eventos cuando el archivo cambia externamente. |
| Compatibilidad | Mantiene soporte con configs previas gracias a defaults y campos opcionales. |
| Integración visual | Sincroniza tema y esquema cuando el entorno lo permite. |

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

El flujo típico es simple: el frontend invoca el plugin, el plugin persiste la configuración y emite `config-changed` cuando detecta cambios externos.

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

## Ejemplo de integración

```vue
<script lang="ts" setup>
import { onMounted, onUnmounted } from "vue";
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

onUnmounted(() => {
  unlistenConfig?.();
});
</script>
```

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

### Recomendado para apps Vasak

```toml
[dependencies]
tauri-plugin-config-manager = { git = "https://github.com/Vasak-OS/tauri-plugin-config-manager" }
```

Si no necesitas sincronización con GNOME/gsettings, desactiva la feature por defecto:

```toml
[dependencies]
tauri-plugin-config-manager = { git = "https://github.com/Vasak-OS/tauri-plugin-config-manager", default-features = false }
```

## Eventos

El plugin emite el evento `config-changed` cuando el archivo de configuración se actualiza.

## Estado de la configuración

La estructura actual soporta:

- `style`: color scheme, dark mode y radius.
- `desktop`: wallpaper, icon size y visibilidad de archivos.
- `fonts`: fuentes para terminal, títulos y apps.

Las claves `fonts.termina`, `fonts.title` y `fonts.apps` se serializan como strings y se mantienen compatibles con configuraciones previas.

## Buenas prácticas de consumo

- Carga la configuración una vez al arrancar la app.
- Escucha `config-changed` si otro proceso puede modificar el archivo.
- Usa `writeConfig` para persistir cambios del usuario.
- Usa `setDarkMode` solo cuando quieras sincronizar también el estado visual del sistema.

## Licencia

LGPL-3.0-or-later
