/**
 * Conversiones de color entre sRGB y OKLCH, para generar el esquema desde el
 * fondo de pantalla.
 *
 * Se trabaja en OKLCH (Björn Ottosson, 2020) y no en HSL porque es perceptual:
 * subir la luminosidad `L` en un paso igual se ve como un paso igual en
 * cualquier tono, y cambiar el tono no cambia lo claro que se ve el color. Eso
 * es lo que deja ajustar un color hasta que dé el contraste sin que cambie de
 * carácter —un turquesa sigue siendo turquesa, sólo más claro o más oscuro—.
 *
 * Todo es puro: sin DOM ni Tauri.
 */

export type Rgb = { r: number; g: number; b: number };
export type Oklab = { l: number; a: number; b: number };
export type Oklch = { l: number; c: number; h: number };

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const toLinear = (channel: number) => {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};

const fromLinear = (channel: number) => {
  const c = channel <= 0.0031308 ? 12.92 * channel : 1.055 * channel ** (1 / 2.4) - 0.055;
  return c * 255;
};

/** `#rgb` o `#rrggbb` a sus tres canales, o `null` si no es un color. */
export function hexToRgb(hex: string): Rgb | null {
  const match = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim());
  if (!match) return null;
  let digits = match[1] as string;
  if (digits.length === 3) digits = [...digits].map((d) => d + d).join("");
  const value = Number.parseInt(digits, 16);
  return { r: (value >> 16) & 255, g: (value >> 8) & 255, b: value & 255 };
}

/** Tres canales a `#rrggbb` en minúscula, que es lo que guarda el esquema. */
export function rgbToHex({ r, g, b }: Rgb): string {
  const part = (c: number) =>
    Math.round(clamp(c, 0, 255))
      .toString(16)
      .padStart(2, "0");
  return `#${part(r)}${part(g)}${part(b)}`;
}

export function rgbToOklab({ r, g, b }: Rgb): Oklab {
  const lr = toLinear(r);
  const lg = toLinear(g);
  const lb = toLinear(b);
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  return {
    l: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

/** OKLab a sRGB **lineal** sin recortar: fuera de [0, 1] es fuera de la gama. */
function oklabToLinear({ l, a, b }: Oklab): [number, number, number] {
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ];
}

export function oklabToRgb(lab: Oklab): Rgb {
  const [r, g, b] = oklabToLinear(lab);
  return {
    r: fromLinear(clamp(r, 0, 1)),
    g: fromLinear(clamp(g, 0, 1)),
    b: fromLinear(clamp(b, 0, 1)),
  };
}

export function oklabToOklch({ l, a, b }: Oklab): Oklch {
  const c = Math.hypot(a, b);
  let h = (Math.atan2(b, a) * 180) / Math.PI;
  if (h < 0) h += 360;
  return { l, c, h };
}

export function oklchToOklab({ l, c, h }: Oklch): Oklab {
  const rad = (h * Math.PI) / 180;
  return { l, a: c * Math.cos(rad), b: c * Math.sin(rad) };
}

export function hexToOklch(hex: string): Oklch | null {
  const rgb = hexToRgb(hex);
  return rgb ? oklabToOklch(rgbToOklab(rgb)) : null;
}

const EPSILON = 1e-4;

function inGamut(lch: Oklch): boolean {
  return oklabToLinear(oklchToOklab(lch)).every((c) => c >= -EPSILON && c <= 1 + EPSILON);
}

/**
 * El color en `#rrggbb`, metido en la gama de sRGB **bajando el croma** y
 * dejando la luminosidad y el tono como estaban.
 *
 * Recortar canal por canal cambiaría el tono —un cian muy saturado se iría a
 * verde— y la luminosidad, que es justo lo que se está ajustando para el
 * contraste.
 */
export function oklchToHex(lch: Oklch): string {
  const l = clamp(lch.l, 0, 1);
  const color = { l, c: Math.max(0, lch.c), h: lch.h };
  if (!inGamut(color)) {
    let low = 0;
    let high = color.c;
    for (let i = 0; i < 24; i++) {
      const mid = (low + high) / 2;
      if (inGamut({ ...color, c: mid })) low = mid;
      else high = mid;
    }
    color.c = low;
  }
  return rgbToHex(oklabToRgb(oklchToOklab(color)));
}

/** La distancia más corta entre dos tonos, en grados (0 a 180). */
export function hueDistance(first: number, second: number): number {
  const diff = Math.abs(first - second) % 360;
  return diff > 180 ? 360 - diff : diff;
}
