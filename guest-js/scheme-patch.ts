/**
 * Cambiar colores de un esquema con un cambio parcial (2.10.0).
 *
 * Lo usan el editor del esquema «Personalizado» de vasak-settings y el modo
 * «Seguir al fondo», que corre en el escritorio. Estaba en vasak-settings;
 * vive acá para que las dos escriban el esquema con la misma regla.
 *
 * Todo es puro: sin Tauri ni Vue.
 */

import type { AnsiColors, SchemeData } from "./index";

export type SchemeVariantName = "dark" | "light";

/** Las dos variantes, en el orden en que las muestra el editor. */
export const SCHEME_VARIANTS: readonly SchemeVariantName[] = ["dark", "light"];

/**
 * Un cambio parcial de colores sobre una variante: lo que se nombra se cambia,
 * lo demás queda como estaba.
 */
export type SchemeColorPatch = {
  ui?: {
    color?: { primary?: string; secondary?: string };
    text?: { main?: string; muted?: string; "on-primary"?: string; "on-secondary"?: string };
    background?: string;
    border?: string;
    surface?: string;
  };
  terminal?: {
    foreground?: string;
    background?: string;
    cursor?: string;
    ansi?: Partial<Record<keyof AnsiColors, string>>;
  };
};

/**
 * Una copia profunda de un esquema.
 *
 * Por JSON y no con `structuredClone`: el esquema suele llegar desde el estado
 * de Vue, envuelto en un `Proxy` reactivo, y `structuredClone` no copia un
 * `Proxy` —tira `DataCloneError`—. El esquema es un JSON, así que ida y vuelta
 * por JSON no pierde nada.
 */
export function cloneScheme<T>(scheme: T): T {
  return JSON.parse(JSON.stringify(scheme)) as T;
}

const HEX_COLOR = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

/** Si el valor es un color que el esquema acepta: `#rgb` o `#rrggbb`. */
export function isHexColor(value: unknown): value is string {
  return typeof value === "string" && HEX_COLOR.test(value.trim());
}

/**
 * Copia en `target` los colores de `patch` que sean colores de verdad.
 *
 * Lo que no pasa `isHexColor` se descarta: un campo a medio escribir no puede
 * llegar al archivo y dejar a todo el escritorio sin un color. Las claves que
 * `target` ya tiene y el cambio no nombra quedan como estaban.
 */
function mergeColors(target: Record<string, unknown>, patch: Record<string, unknown>): void {
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) continue;
    if (value !== null && typeof value === "object") {
      const current = target[key];
      const nested =
        current !== null && typeof current === "object" ? (current as Record<string, unknown>) : {};
      mergeColors(nested, value as Record<string, unknown>);
      target[key] = nested;
    } else if (isHexColor(value)) {
      target[key] = value.trim();
    }
  }
}

/**
 * El esquema con los colores del cambio aplicados sobre una variante.
 *
 * Devuelve un esquema nuevo y deja el recibido intacto, para que quien lo
 * tenga guardado —el estado de Vue, una prueba— no lo vea cambiar por debajo.
 * Conserva el tipo del esquema que recibe, con sus claves de más.
 */
export function applyColorPatch<T extends SchemeData>(
  scheme: T,
  variant: SchemeVariantName,
  patch: SchemeColorPatch,
): T {
  const next = cloneScheme(scheme);
  mergeColors(
    next.colors[variant] as unknown as Record<string, unknown>,
    patch as Record<string, unknown>,
  );
  return next;
}
