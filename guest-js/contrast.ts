/*
 * Contraste (WCAG 2.1), en su propio módulo desde la 2.10.0: lo usan el store,
 * al corregir el texto sobre los colores de marca, y el generador de colores
 * desde el fondo de pantalla (`wallpaper-scheme.ts`). Los nombres de antes
 * (`contraste`, `textoSobre`…) siguen exportados desde `index.ts` como alias.
 */

import type { UiColors } from "./index";

/**
 * Contraste, para que el texto sobre los colores de marca se pueda leer.
 *
 * El esquema trae `on-primary` como un valor fijo, y eso funciona sólo mientras el
 * acento no cambie. El esquema por omisión de VasakOS lo tenía en `#cdd6f4` sobre
 * un primario `#eba0ac`: **1.43:1**, contra el mínimo de 4.5 que pide WCAG 1.4.3.
 * O sea que el texto de cualquier botón de acento era casi invisible, y con
 * cualquier esquema nuevo el problema vuelve, porque nada obliga a quien lo escribe
 * a verificarlo.
 *
 * Así que no se confía: se calcula. Si el valor del esquema cumple, se respeta —es
 * una decisión estética de quien lo hizo—; si no llega, se reemplaza por el color
 * de su propia paleta que mejor contraste dé. Cambiar el acento a lo que sea deja
 * el texto legible sin tocar nada más.
 */
export const MIN_TEXT_CONTRAST = 4.5;
/** WCAG 1.4.11: lo que delimita un control necesita 3:1, no 4.5. */
export const MIN_NON_TEXT_CONTRAST = 3;

export function luminance(hex: string): number | null {
  const trimmed = hex.trim().replace(/^#/, "");
  const full =
    trimmed.length === 3
      ? trimmed
          .split("")
          .map((c) => c + c)
          .join("")
      : trimmed;
  if (!/^[0-9a-fA-F]{6}$/.test(full)) return null;

  const channel = (i: number) => {
    const v = Number.parseInt(full.slice(i, i + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4);
}

export function contrastRatio(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  // Un color que no se puede leer no puede compararse: se informa el peor caso
  // para que nunca se elija por «buen contraste».
  if (la === null || lb === null) return 0;
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/**
 * El mejor color de la lista sobre ese fondo, o `null` si ninguno llega al mínimo.
 *
 * Se recorre en orden y gana el de mayor contraste, no el primero que pase: entre
 * dos que cumplen conviene el más legible, y la diferencia entre 4.6 y 9 se nota.
 */
export function bestOn(
  background: string,
  candidates: Array<string | undefined>,
  minimum: number,
): string | null {
  let chosen: string | null = null;
  let best = 0;
  for (const c of candidates) {
    if (!c) continue;
    const r = contrastRatio(c, background);
    if (r >= minimum && r > best) {
      best = r;
      chosen = c;
    }
  }
  return chosen;
}

/**
 * El color de texto para un fondo de marca.
 *
 * Se respeta el del esquema si cumple. Si no, se busca en su propia paleta —el
 * fondo, la superficie, el texto principal— para no salirse de la familia de
 * colores, y sólo como último recurso se cae a negro o blanco.
 */
export function textOn(
  background: string,
  preferred: string | undefined,
  palette: UiColors,
): string {
  if (preferred && contrastRatio(preferred, background) >= MIN_TEXT_CONTRAST) return preferred;

  // Dos etapas y no una lista sola: `bestOn` se queda con el de más contraste,
  // y el negro puro le gana a cualquier color de la paleta casi siempre. Con una
  // sola lista, un botón de acento terminaba con texto negro aunque el esquema
  // tuviera un color propio perfectamente legible — o sea, salirse de la familia
  // de colores sin necesidad. El negro y el blanco quedan como último recurso.
  const fromPalette = bestOn(
    background,
    [palette.background, palette.text.main, palette.surface],
    MIN_TEXT_CONTRAST,
  );
  if (fromPalette) return fromPalette;

  return bestOn(background, ["#000000", "#ffffff"], MIN_TEXT_CONTRAST) ?? "#000000";
}

/**
 * Un borde que se perciba para lo que delimita un control.
 *
 * El borde del esquema es un separador decorativo —el de VasakOS da 1.14 contra el
 * fondo— y con eso el contorno de un campo no se ve. Se busca en la paleta uno que
 * llegue a 3:1 sin ser tan fuerte como el texto.
 */
export function strongBorderOn(palette: UiColors): string | null {
  return bestOn(
    palette.background,
    [palette.text.muted, palette.surface, palette.text.main],
    MIN_NON_TEXT_CONTRAST,
  );
}
