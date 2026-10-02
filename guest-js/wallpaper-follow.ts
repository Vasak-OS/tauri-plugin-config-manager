/**
 * «Seguir al fondo»: lo que hace quien vigila el fondo de pantalla cuando
 * cambia (2.10.0, Vasak-OS/vasak-settings#134).
 *
 * Lo corre vasak-desktop, que está siempre abierto: así el acento sigue al
 * fondo aunque Configuración esté cerrada. Configuración usa
 * `applyWallpaperColors` para «Volver a sacar del fondo».
 *
 * El camino es el de siempre: el esquema se guarda con `saveUserScheme` y el
 * vigilante de cada aplicación abierta lo reaplica. La lectura de los píxeles
 * la pone quien llama (`readPixels`): es un comando de Rust de la aplicación,
 * porque decodificar una imagen o un cuadro de video no se hace en la página.
 */

import type { SchemeData, VSKConfig } from "./index";
import { applyColorPatch, SCHEME_VARIANTS } from "./scheme-patch";
import { extractPalette, type PaletteColor, type WallpaperPixels } from "./wallpaper-palette";
import {
  type Accents,
  buildWallpaperPatches,
  omitPaths,
  readWallpaperState,
  withWallpaperState,
} from "./wallpaper-scheme";

/** El id del esquema que sigue al fondo: el «Personalizado» del usuario. */
export const CUSTOM_SCHEME_ID = "custom";

/** La clave de `style` en `vasak.conf` con el esquema en uso. */
const SCHEME_KEY = "color-scheme";

export type WallpaperColorsResult<T extends SchemeData> = {
  scheme: T;
  palette: PaletteColor[];
  accents: Accents;
};

/**
 * El esquema con los colores sacados de la paleta, sin los fijados a mano, y
 * con el fondo de origen anotado. `null` si la paleta está vacía: no se
 * inventan colores.
 */
export function applyWallpaperColors<T extends SchemeData>(
  scheme: T,
  palette: PaletteColor[],
  source: string,
): WallpaperColorsResult<T> | null {
  const state = readWallpaperState(scheme);
  const built = buildWallpaperPatches(scheme, palette, state.pinned);
  if (!built) return null;
  let next = scheme;
  for (const variant of SCHEME_VARIANTS) {
    next = applyColorPatch(next, variant, omitPaths(built.patches[variant], state.pinned[variant]));
  }
  return {
    scheme: withWallpaperState(next, { ...state, source }),
    palette,
    accents: built.accents,
  };
}

/** Qué pasó en un `followWallpaper`. Sólo `applied` escribió algo. */
export type FollowOutcome =
  | "applied"
  | "not-custom"
  | "no-wallpaper"
  | "no-scheme"
  | "off"
  | "unchanged"
  | "unreadable"
  | "save-failed";

export type FollowWallpaperDeps = {
  readConfig: () => Promise<VSKConfig | null>;
  loadScheme: (id: string) => Promise<SchemeData | null>;
  readPixels: (path: string) => Promise<WallpaperPixels>;
  save: (scheme: SchemeData) => Promise<unknown>;
};

/**
 * Si el «Personalizado» está en uso, tiene «Seguir al fondo» prendido y el
 * fondo de `vasak.conf` es otro que el de la última vez, saca los colores del
 * fondo y guarda el esquema.
 *
 * Se puede llamar en cada `config-changed`: el guardado anota el fondo de
 * origen, así que el `config-changed` que provoca el propio guardado vuelve
 * `unchanged` y no hay bucle. Si el fondo no se puede leer, no se escribe nada.
 */
export async function followWallpaper(deps: FollowWallpaperDeps): Promise<FollowOutcome> {
  const config = await deps.readConfig();
  if (config?.style?.[SCHEME_KEY] !== CUSTOM_SCHEME_ID) return "not-custom";
  const path = config.desktop?.wallpaper?.[0] ?? "";
  if (!path) return "no-wallpaper";

  const scheme = await deps.loadScheme(CUSTOM_SCHEME_ID);
  if (!scheme) return "no-scheme";
  const state = readWallpaperState(scheme);
  if (!state.follow) return "off";
  if (state.source === path) return "unchanged";

  let palette: PaletteColor[];
  try {
    palette = extractPalette(await deps.readPixels(path));
  } catch {
    return "unreadable";
  }
  const result = applyWallpaperColors(scheme, palette, path);
  if (!result) return "unreadable";

  try {
    await deps.save(result.scheme);
  } catch {
    return "save-failed";
  }
  return "applied";
}
