/**
 * La paleta de un fondo de pantalla: los colores que más pesan en la imagen y
 * de qué parte de ella salió cada uno.
 *
 * Recibe los píxeles crudos que da `wallpaper_pixels` (RGB, tres bytes por
 * píxel, unos cinco mil en total) y devuelve unos pocos colores con su peso.
 * El croma, el tono y la luminosidad de cada uno van en OKLCH, que es con lo
 * que después se elige el acento (`wallpaper-scheme.ts`).
 *
 * # Por qué así y no `color_thief`
 *
 * vasak-resonance saca el color de las carátulas con el crate `color_thief`,
 * que da un color dominante. Acá hace falta más: una paleta, el peso de cada
 * color, y **dónde está** en la imagen, porque la vista previa marca de dónde
 * salió cada uno. Además, en TypeScript se prueba sin compilar nada y lo puede
 * usar cualquier otra pantalla del escritorio.
 *
 * # El método
 *
 * 1. Los píxeles se agrupan en cubetas de 4 bits por canal (4096 como mucho):
 *    es lo que hace que lo de abajo cueste lo mismo para cualquier imagen.
 * 2. k-medias en OKLab sobre las cubetas, pesadas por cuántos píxeles tienen.
 *    En OKLab porque ahí la distancia es la que ve el ojo; en RGB dos verdes
 *    que se ven iguales quedan lejos y un gris y un azul oscuro, cerca.
 * 3. La semilla de k-medias es determinista —la cubeta más pesada y después
 *    la más lejana de las elegidas, pesada—: el mismo fondo da siempre la
 *    misma paleta, y un recálculo no cambia los colores por azar.
 */

import { type Oklab, oklabToOklch, oklabToRgb, rgbToHex, rgbToOklab } from "./color-space";

export type PaletteColor = {
  hex: string;
  /** La fracción de la imagen que tiene este color, de 0 a 1. */
  population: number;
  /** Dónde está el color en la imagen, de 0 a 1 desde la izquierda y desde arriba. */
  x: number;
  y: number;
  /** OKLCH. */
  lightness: number;
  chroma: number;
  hue: number;
};

export type WallpaperPixels = {
  width: number;
  height: number;
  data: ArrayLike<number>;
};

/** Cuántos colores da la paleta como mucho. */
export const PALETTE_SIZE = 6;

const ITERATIONS = 12;

type Bucket = {
  count: number;
  lab: Oklab;
  x: number;
  y: number;
};

function bucketize({ width, height, data }: WallpaperPixels): Bucket[] {
  const sums = new Map<
    number,
    { n: number; r: number; g: number; b: number; x: number; y: number }
  >();
  const pixels = Math.min(width * height, Math.floor(data.length / 3));
  for (let index = 0; index < pixels; index++) {
    const r = data[index * 3] as number;
    const g = data[index * 3 + 1] as number;
    const b = data[index * 3 + 2] as number;
    const key = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
    const x = (index % width) + 0.5;
    const y = Math.floor(index / width) + 0.5;
    const sum = sums.get(key);
    if (sum) {
      sum.n += 1;
      sum.r += r;
      sum.g += g;
      sum.b += b;
      sum.x += x;
      sum.y += y;
    } else {
      sums.set(key, { n: 1, r, g, b, x, y });
    }
  }
  const buckets: Bucket[] = [];
  // Por clave, para que el orden no dependa del orden de los píxeles.
  for (const key of [...sums.keys()].sort((first, second) => first - second)) {
    const sum = sums.get(key);
    if (!sum) continue;
    buckets.push({
      count: sum.n,
      lab: rgbToOklab({ r: sum.r / sum.n, g: sum.g / sum.n, b: sum.b / sum.n }),
      x: sum.x / sum.n / width,
      y: sum.y / sum.n / height,
    });
  }
  return buckets;
}

const distance2 = (first: Oklab, second: Oklab) =>
  (first.l - second.l) ** 2 + (first.a - second.a) ** 2 + (first.b - second.b) ** 2;

function seeds(buckets: Bucket[], k: number): Oklab[] {
  const heaviest = buckets.reduce(
    (best, bucket) => (bucket.count > best.count ? bucket : best),
    buckets[0] as Bucket,
  );
  const chosen: Oklab[] = [heaviest.lab];
  while (chosen.length < k) {
    let best: Bucket | null = null;
    let bestScore = 0;
    for (const bucket of buckets) {
      const nearest = Math.min(...chosen.map((center) => distance2(center, bucket.lab)));
      const score = nearest * bucket.count;
      if (score > bestScore) {
        bestScore = score;
        best = bucket;
      }
    }
    // Todo lo que queda es igual a algo ya elegido: no hay más colores.
    if (!best) break;
    chosen.push(best.lab);
  }
  return chosen;
}

/**
 * Los colores del fondo, del que más pesa al que menos.
 *
 * Una imagen vacía da una paleta vacía; quien la use se queda con los colores
 * que ya tenía.
 */
export function extractPalette(pixels: WallpaperPixels, size = PALETTE_SIZE): PaletteColor[] {
  if (pixels.width <= 0 || pixels.height <= 0) return [];
  // Una muestra más corta que lo que dice medir es un fondo que no se leyó
  // entero: sacar colores de una parte, con las posiciones calculadas sobre el
  // tamaño declarado, daría una paleta falsa. Mejor ninguna.
  if (pixels.data.length < pixels.width * pixels.height * 3) return [];
  const buckets = bucketize(pixels);
  if (buckets.length === 0) return [];
  const total = buckets.reduce((sum, bucket) => sum + bucket.count, 0);

  let centers = seeds(buckets, Math.min(size, buckets.length));
  let assignment = new Array<number>(buckets.length).fill(0);

  for (let iteration = 0; iteration < ITERATIONS; iteration++) {
    assignment = buckets.map((bucket) => {
      let best = 0;
      let bestDistance = Number.POSITIVE_INFINITY;
      centers.forEach((center, index) => {
        const d = distance2(center, bucket.lab);
        if (d < bestDistance) {
          bestDistance = d;
          best = index;
        }
      });
      return best;
    });
    centers = centers.map((center, index) => {
      let weight = 0;
      const sum = { l: 0, a: 0, b: 0 };
      buckets.forEach((bucket, bucketIndex) => {
        if (assignment[bucketIndex] !== index) return;
        weight += bucket.count;
        sum.l += bucket.lab.l * bucket.count;
        sum.a += bucket.lab.a * bucket.count;
        sum.b += bucket.lab.b * bucket.count;
      });
      return weight > 0 ? { l: sum.l / weight, a: sum.a / weight, b: sum.b / weight } : center;
    });
  }

  const colors: PaletteColor[] = [];
  centers.forEach((center, index) => {
    let count = 0;
    let x = 0;
    let y = 0;
    buckets.forEach((bucket, bucketIndex) => {
      if (assignment[bucketIndex] !== index) return;
      count += bucket.count;
      x += bucket.x * bucket.count;
      y += bucket.y * bucket.count;
    });
    if (count === 0) return;
    const lch = oklabToOklch(center);
    colors.push({
      hex: rgbToHex(oklabToRgb(center)),
      population: count / total,
      x: x / count,
      y: y / count,
      lightness: lch.l,
      chroma: lch.c,
      hue: lch.h,
    });
  });

  return colors.sort((first, second) => second.population - first.population);
}
