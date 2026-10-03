/**
 * Del fondo de pantalla a los colores de interfaz del esquema «Personalizado».
 *
 * Es la parte «Automático» de Vasak-OS/vasak-settings#134 (aquí desde la 2.10.0, para que lo use también el escritorio). Recibe la paleta
 * del fondo (`wallpaper-palette.ts`) y el esquema como está, y devuelve un
 * cambio parcial por variante que se aplica por el mismo camino que el editor
 * (`updateColors` de `useCustomScheme`).
 *
 * # Qué cambia y qué no
 *
 * - Sólo los colores de `ui`. La terminal se queda con la paleta ANSI del
 *   esquema clonado, que ya es una paleta probada.
 * - El **acento** (`primary`) sale del color del fondo que más se nota: el que
 *   combina croma y superficie. El secundario, de otro tono del fondo.
 * - El fondo, la superficie, el borde y los textos **conservan la luminosidad**
 *   que tenían en el esquema y toman un tinte muy suave del acento. Así el
 *   punto de partida sigue siendo el esquema que la persona eligió —claro
 *   donde era claro, con su borde más oscuro que el fondo si así lo tenía— y
 *   el fondo de pantalla sólo le cambia el tono.
 * - Un color **fijado a mano** no se toca: entra a los cálculos de contraste
 *   como dado, y se ajusta lo que está a su alrededor.
 *
 * # El contraste se garantiza, no se espera
 *
 * Después de generar, cada par de texto y fondo se mide con el mismo cálculo
 * que usa el escritorio (`contrastRatio`, el mismo del store), y si no llega al mínimo se
 * mueve la **luminosidad** del color generado —en OKLCH, sin cambiarle el
 * tono— hasta que llegue. Nunca se usa un color del fondo tal cual. Los
 * mínimos son los de WCAG 2.1 AA:
 *
 * - 4,5:1 para el texto (`main` y `muted`) sobre el fondo y sobre la
 *   superficie, y para `on-primary` / `on-secondary` sobre su color;
 * - 3:1 para el primario y el secundario sobre el fondo, que es lo que pide
 *   1.4.11 para un indicador (el espacio activo, la píldora encendida).
 */

import { hexToOklch, hueDistance, type Oklch, oklchToHex } from "./color-space";
import { contrastRatio } from "./contrast";
import type { SchemeData, ThemeVariant } from "./index";
import { SCHEME_VARIANTS, type SchemeColorPatch, type SchemeVariantName } from "./scheme-patch";
import type { PaletteColor } from "./wallpaper-palette";

export const TEXT_CONTRAST = 4.5;
export const INDICATOR_CONTRAST = 3;

/** Debajo de este croma un color es gris: no sirve de acento. */
const MIN_ACCENT_CHROMA = 0.035;

/** Un color de `ui`, nombrado por su camino dentro de la variante. */
export type UiColorPath =
  | "ui.color.primary"
  | "ui.color.secondary"
  | "ui.text.main"
  | "ui.text.muted"
  | "ui.text.on-primary"
  | "ui.text.on-secondary"
  | "ui.background"
  | "ui.surface"
  | "ui.border";

export const UI_COLOR_PATHS: readonly UiColorPath[] = [
  "ui.color.primary",
  "ui.color.secondary",
  "ui.text.main",
  "ui.text.muted",
  "ui.text.on-primary",
  "ui.text.on-secondary",
  "ui.background",
  "ui.surface",
  "ui.border",
];

/** Los colores fijados a mano, por variante. */
export type PinnedColors = Record<SchemeVariantName, UiColorPath[]>;

/** Lo que el modo automático guarda en `custom.json`, al lado de los colores. */
export type WallpaperColorsState = {
  /** «Seguir al fondo»: si un cambio de fondo recalcula los colores. */
  follow: boolean;
  /** El fondo del que salieron los colores la última vez. */
  source: string;
  pinned: PinnedColors;
};

/**
 * La clave de `custom.json` donde vive el estado del modo automático.
 *
 * En el esquema y no en `vasak.conf`: es del esquema —los colores fijados son
 * colores de este archivo— y así viaja con él si se copia o se comparte. El
 * plugin conserva las claves que no conoce (`SchemeData.extra`).
 */
export const WALLPAPER_STATE_KEY = "wallpaper-colors";

const emptyPinned = (): PinnedColors => ({ dark: [], light: [] });

const isUiPath = (value: unknown): value is UiColorPath =>
  typeof value === "string" && (UI_COLOR_PATHS as readonly string[]).includes(value);

/**
 * El estado del modo automático de un esquema, con valores sanos aunque el
 * archivo se haya editado a mano y diga cualquier cosa. Por omisión, apagado:
 * el modo se prende eligiendo «Automático».
 */
export function readWallpaperState(scheme: SchemeData | null): WallpaperColorsState {
  const raw = (scheme as Record<string, unknown> | null)?.[WALLPAPER_STATE_KEY];
  const state: WallpaperColorsState = { follow: false, source: "", pinned: emptyPinned() };
  if (!raw || typeof raw !== "object") return state;
  const value = raw as Record<string, unknown>;
  state.follow = value.follow === true;
  state.source = typeof value.source === "string" ? value.source : "";
  const pinned = value.pinned as Record<string, unknown> | undefined;
  for (const variant of SCHEME_VARIANTS) {
    const list = pinned?.[variant];
    state.pinned[variant] = Array.isArray(list) ? [...new Set(list.filter(isUiPath))] : [];
  }
  return state;
}

/** El esquema con el estado del modo automático reemplazado. No toca el recibido. */
export function withWallpaperState<T extends SchemeData>(
  scheme: T,
  state: WallpaperColorsState,
): T {
  return {
    ...scheme,
    [WALLPAPER_STATE_KEY]: {
      follow: state.follow,
      source: state.source,
      pinned: { dark: [...state.pinned.dark], light: [...state.pinned.light] },
    },
  };
}

/** Los caminos de `ui` que nombra un cambio parcial: lo que el editor tocó. */
export function uiPathsOf(patch: SchemeColorPatch): UiColorPath[] {
  const paths: UiColorPath[] = [];
  const ui = patch.ui;
  if (!ui) return paths;
  if (ui.color?.primary !== undefined) paths.push("ui.color.primary");
  if (ui.color?.secondary !== undefined) paths.push("ui.color.secondary");
  if (ui.text?.main !== undefined) paths.push("ui.text.main");
  if (ui.text?.muted !== undefined) paths.push("ui.text.muted");
  if (ui.text?.["on-primary"] !== undefined) paths.push("ui.text.on-primary");
  if (ui.text?.["on-secondary"] !== undefined) paths.push("ui.text.on-secondary");
  if (ui.background !== undefined) paths.push("ui.background");
  if (ui.surface !== undefined) paths.push("ui.surface");
  if (ui.border !== undefined) paths.push("ui.border");
  return paths;
}

/** El valor de un color de `ui` en una variante. */
export function readUiColor(colors: ThemeVariant, path: UiColorPath): string | undefined {
  const ui = colors.ui;
  switch (path) {
    case "ui.color.primary":
      return ui.color.primary;
    case "ui.color.secondary":
      return ui.color.secondary;
    case "ui.text.main":
      return ui.text.main;
    case "ui.text.muted":
      return ui.text.muted;
    case "ui.text.on-primary":
      return ui.text["on-primary"];
    case "ui.text.on-secondary":
      return ui.text["on-secondary"];
    case "ui.background":
      return ui.background;
    case "ui.surface":
      return ui.surface;
    case "ui.border":
      return ui.border;
  }
}

/** Un cambio parcial sin los colores nombrados: lo que el recálculo no pisa. */
export function omitPaths(
  patch: SchemeColorPatch,
  paths: readonly UiColorPath[],
): SchemeColorPatch {
  if (!patch.ui || paths.length === 0) return patch;
  const skip = new Set(paths);
  const ui = patch.ui;
  const next: NonNullable<SchemeColorPatch["ui"]> = {};
  const color = { ...ui.color };
  if (skip.has("ui.color.primary")) delete color.primary;
  if (skip.has("ui.color.secondary")) delete color.secondary;
  if (Object.keys(color).length) next.color = color;
  const text = { ...ui.text };
  if (skip.has("ui.text.main")) delete text.main;
  if (skip.has("ui.text.muted")) delete text.muted;
  if (skip.has("ui.text.on-primary")) delete text["on-primary"];
  if (skip.has("ui.text.on-secondary")) delete text["on-secondary"];
  if (Object.keys(text).length) next.text = text;
  if (ui.background !== undefined && !skip.has("ui.background")) next.background = ui.background;
  if (ui.surface !== undefined && !skip.has("ui.surface")) next.surface = ui.surface;
  if (ui.border !== undefined && !skip.has("ui.border")) next.border = ui.border;
  return { ...patch, ui: next };
}

/** Los dos tonos de los que sale la interfaz. */
export type Accents = {
  primary: { hue: number; chroma: number };
  secondary: { hue: number; chroma: number };
  /** Si el fondo no tenía ningún color con croma y se usó el del esquema. */
  fallback: boolean;
  /** El color del fondo del que salió el primario, para marcarlo en la vista previa. */
  source: PaletteColor | null;
};

/**
 * Cuánto se nota un color del fondo: croma por superficie, con la superficie
 * amortiguada para que una mancha chica y muy viva —el sol de un atardecer—
 * pueda ganarle a un cielo enorme y apagado, pero no a uno entero del mismo
 * tono.
 */
const score = (color: PaletteColor) => color.population ** 0.4 * Math.min(color.chroma, 0.25);

/** Dos colores de la paleta con tonos a menos de esto son el mismo tono. */
const SAME_HUE = 20;

/**
 * Los colores con croma, con los del mismo tono sumados.
 *
 * k-medias parte un degradé —el naranja de un atardecer, de claro a oscuro—
 * en dos o tres colores parecidos, cada uno con una parte de la superficie.
 * Sin juntarlos, ese naranja compite partido contra un cielo azul entero y
 * pierde aunque sea lo que más se ve.
 */
function hueGroups(palette: readonly PaletteColor[]): PaletteColor[] {
  const groups: PaletteColor[] = [];
  const chromatic = palette
    .filter((color) => color.chroma >= MIN_ACCENT_CHROMA)
    .sort((first, second) => second.chroma - first.chroma);
  for (const color of chromatic) {
    const group = groups.find((item) => hueDistance(item.hue, color.hue) < SAME_HUE);
    // El grupo conserva su color más vivo (el primero, por el orden) y suma
    // la superficie de los demás.
    if (group) group.population += color.population;
    else groups.push({ ...color });
  }
  return groups.sort((first, second) => score(second) - score(first));
}

/**
 * El acento y el secundario a partir de la paleta.
 *
 * Si el fondo es gris —una foto en blanco y negro, un fondo liso neutro— no
 * hay de dónde sacar un acento, y uno gris dejaría la interfaz sin nada que
 * marque lo activo: se conserva el tono del primario del esquema.
 */
export function pickAccents(palette: readonly PaletteColor[], fallbackPrimary: string): Accents {
  const chromatic = hueGroups(palette);

  const fallback = hexToOklch(fallbackPrimary) ?? { l: 0.7, c: 0.12, h: 20 };
  const best = chromatic[0];
  if (!best) {
    return {
      primary: { hue: fallback.h, chroma: Math.max(fallback.c, 0.08) },
      secondary: { hue: (fallback.h + 40) % 360, chroma: Math.max(fallback.c, 0.08) * 0.85 },
      fallback: true,
      source: null,
    };
  }

  const other = chromatic.find((color) => hueDistance(color.hue, best.hue) >= 35);
  return {
    primary: { hue: best.hue, chroma: best.chroma },
    secondary: other
      ? { hue: other.hue, chroma: other.chroma }
      : { hue: (best.hue + 40) % 360, chroma: best.chroma * 0.85 },
    fallback: false,
    source: palette.find((color) => color.hex === best.hex) ?? null,
  };
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** La luminosidad del acento en cada variante: viva en oscuro, honda en claro. */
const ACCENT_LIGHTNESS: Record<SchemeVariantName, { primary: number; secondary: number }> = {
  dark: { primary: 0.8, secondary: 0.76 },
  light: { primary: 0.54, secondary: 0.5 },
};

/** El croma del tinte de cada neutro: lo justo para que se note el tono. */
const NEUTRAL_CHROMA = {
  background: 0.014,
  surface: 0.02,
  border: 0.022,
  main: 0.012,
  muted: 0.028,
} as const;

const accentChroma = (chroma: number) => clamp(chroma * 1.1, 0.07, 0.17);

const relightOrDefault = (hex: string | undefined, fallbackL: number) =>
  hexToOklch(hex ?? "")?.l ?? fallbackL;

const tinted = (lightness: number, chroma: number, hue: number) =>
  oklchToHex({ l: lightness, c: chroma, h: hue });

/**
 * Mueve la luminosidad de `color` hasta que dé `minimum` contra **todos** los
 * fondos de `against`, cambiando lo menos posible.
 *
 * Prueba primero alejándose de los fondos —un texto claro sobre oscuro se
 * aclara— y si llega al extremo sin lograrlo, prueba para el otro lado. Si ya
 * cumple, lo devuelve tal cual. Si no hay forma (un fondo muy claro y otro muy
 * oscuro a la vez), devuelve el que más se acercó.
 */
export function ensureContrast(color: string, against: readonly string[], minimum: number): string {
  const worst = (hex: string) =>
    Math.min(...against.map((background) => contrastRatio(hex, background)));
  if (against.length === 0 || worst(color) >= minimum) return color;

  const lch = hexToOklch(color);
  if (!lch) return color;
  const mean =
    against.reduce((sum, background) => sum + (hexToOklch(background)?.l ?? 0.5), 0) /
    against.length;
  const firstDirection = lch.l >= mean ? 1 : -1;

  let best = { hex: color, ratio: worst(color) };
  for (const direction of [firstDirection, -firstDirection]) {
    const found = walkLightness(lch, direction, worst, minimum);
    if (found.ratio >= minimum) return found.hex;
    if (found.ratio > best.ratio) best = found;
  }
  return best.hex;
}

/**
 * Un lado de la búsqueda de `ensureContrast`: aclara (`direction` 1) u
 * oscurece (-1) de a 0,005 hasta llegar al mínimo o al extremo. Devuelve el
 * primero que llega, o el mejor que encontró.
 */
function walkLightness(
  lch: Oklch,
  direction: number,
  worst: (hex: string) => number,
  minimum: number,
): { hex: string; ratio: number } {
  let best = { hex: oklchToHex(lch), ratio: 0 };
  for (let step = 1; step <= 200; step++) {
    const l = lch.l + direction * step * 0.005;
    if (l < 0 || l > 1) break;
    const hex = oklchToHex({ ...lch, l });
    const ratio = worst(hex);
    if (ratio >= minimum) return { hex, ratio };
    if (ratio > best.ratio) best = { hex, ratio };
  }
  // El extremo exacto, por si el paso de 0,005 lo saltea.
  const edge = oklchToHex({ ...lch, l: direction > 0 ? 1 : 0, c: 0 });
  const edgeRatio = worst(edge);
  return edgeRatio >= best.ratio ? { hex: edge, ratio: edgeRatio } : best;
}

type UiDraft = {
  primary: string;
  secondary: string;
  main: string;
  muted: string;
  onPrimary: string;
  onSecondary?: string;
  background: string;
  surface: string;
  border: string;
};

const PATH_OF: Record<keyof UiDraft, UiColorPath> = {
  primary: "ui.color.primary",
  secondary: "ui.color.secondary",
  main: "ui.text.main",
  muted: "ui.text.muted",
  onPrimary: "ui.text.on-primary",
  onSecondary: "ui.text.on-secondary",
  background: "ui.background",
  surface: "ui.surface",
  border: "ui.border",
};

type IsPinned = (key: keyof UiDraft) => boolean;

/** Lo que sale del fondo, sin mirar lo fijado ni el contraste todavía. */
function generatedDraft(
  current: ThemeVariant,
  variant: SchemeVariantName,
  accents: Accents,
): UiDraft {
  const ui = current.ui;
  const hue = accents.primary.hue;
  const tone = ACCENT_LIGHTNESS[variant];
  const neutral = variant === "dark" ? 0.24 : 0.96;
  const relit = (hex: string | undefined, fallback: number, chroma: number) =>
    tinted(relightOrDefault(hex, fallback), chroma, hue);
  return {
    primary: tinted(tone.primary, accentChroma(accents.primary.chroma), hue),
    secondary: tinted(
      tone.secondary,
      accentChroma(accents.secondary.chroma),
      accents.secondary.hue,
    ),
    background: relit(ui.background, neutral, NEUTRAL_CHROMA.background),
    surface: relit(ui.surface, neutral, NEUTRAL_CHROMA.surface),
    border: relit(ui.border, neutral, NEUTRAL_CHROMA.border),
    main: relit(ui.text.main, 1 - neutral, NEUTRAL_CHROMA.main),
    muted: relit(ui.text.muted, 0.6, NEUTRAL_CHROMA.muted),
    onPrimary: "",
    onSecondary: undefined,
  };
}

/** El texto sobre un acento: el neutro del extremo que más contraste, con su tinte. */
function textOn(background: string, hue: number): string {
  const dark = tinted(0.22, NEUTRAL_CHROMA.main, hue);
  const light = tinted(0.985, NEUTRAL_CHROMA.main / 2, hue);
  return contrastRatio(dark, background) >= contrastRatio(light, background) ? dark : light;
}

/**
 * Los textos sobre la ventana y las tarjetas. Si el texto está fijado, se
 * mueven el fondo y la superficie, cada uno contra el texto.
 */
function enforceTexts(draft: UiDraft, isPinned: IsPinned): void {
  const surfaces = () => [draft.background, draft.surface];
  for (const key of ["main", "muted"] as const) {
    if (!isPinned(key)) {
      draft[key] = ensureContrast(draft[key], surfaces(), TEXT_CONTRAST);
      continue;
    }
    for (const surface of ["background", "surface"] as const) {
      if (!isPinned(surface))
        draft[surface] = ensureContrast(draft[surface], [draft[key]], TEXT_CONTRAST);
    }
  }
  // Mover un fondo por el `muted` puede haber dejado corto al `main`.
  if (!isPinned("main")) draft.main = ensureContrast(draft.main, surfaces(), TEXT_CONTRAST);
}

/**
 * Cada acento sobre la ventana, como indicador, y su texto encima. Si el texto
 * está fijado, se mueve el acento.
 */
function enforceAccent(
  draft: UiDraft,
  isPinned: IsPinned,
  accent: "primary" | "secondary",
  text: "onPrimary" | "onSecondary",
): void {
  if (!isPinned(accent))
    draft[accent] = ensureContrast(draft[accent], [draft.background], INDICATOR_CONTRAST);
  const textValue = draft[text];
  if (textValue === undefined) return;
  if (!isPinned(text)) draft[text] = ensureContrast(textValue, [draft[accent]], TEXT_CONTRAST);
  else if (!isPinned(accent))
    draft[accent] = ensureContrast(draft[accent], [textValue], TEXT_CONTRAST);
}

/**
 * Los colores de `ui` de una variante a partir de los acentos, con los fijados
 * respetados y el contraste garantizado.
 */
export function generateVariantUi(
  current: ThemeVariant,
  variant: SchemeVariantName,
  accents: Accents,
  pinned: readonly UiColorPath[] = [],
): UiDraft {
  const isPinned: IsPinned = (key) => pinned.includes(PATH_OF[key]);
  const hue = accents.primary.hue;

  // Lo fijado entra como está, y todo lo que se calcula después lo toma como
  // dado.
  const draft = generatedDraft(current, variant, accents);
  for (const key of Object.keys(PATH_OF) as (keyof UiDraft)[]) {
    const value = isPinned(key) ? readUiColor(current, PATH_OF[key]) : undefined;
    if (value) draft[key] = value;
  }

  if (!isPinned("onPrimary")) draft.onPrimary = textOn(draft.primary, hue);
  const hasOnSecondary = typeof current.ui.text["on-secondary"] === "string";
  if (!hasOnSecondary) draft.onSecondary = undefined;
  else if (!isPinned("onSecondary")) draft.onSecondary = textOn(draft.secondary, hue);

  enforceTexts(draft, isPinned);
  enforceAccent(draft, isPinned, "primary", "onPrimary");
  enforceAccent(draft, isPinned, "secondary", "onSecondary");
  return draft;
}

/** El cambio parcial de `ui` con lo generado, sin lo fijado. */
export function draftToPatch(
  draft: UiDraft,
  pinned: readonly UiColorPath[] = [],
): SchemeColorPatch {
  const patch: SchemeColorPatch = {
    ui: {
      color: { primary: draft.primary, secondary: draft.secondary },
      text: { main: draft.main, muted: draft.muted, "on-primary": draft.onPrimary },
      background: draft.background,
      surface: draft.surface,
      border: draft.border,
    },
  };
  if (draft.onSecondary !== undefined && patch.ui?.text) {
    patch.ui.text["on-secondary"] = draft.onSecondary;
  }
  return omitPaths(patch, pinned);
}

export type WallpaperPatches = {
  patches: Record<SchemeVariantName, SchemeColorPatch>;
  accents: Accents;
};

/**
 * Los cambios de las dos variantes para un fondo.
 *
 * Una paleta vacía —un fondo que no se pudo leer— da `null`: no se cambia
 * nada, que es mejor que inventar colores.
 */
export function buildWallpaperPatches(
  scheme: SchemeData,
  palette: readonly PaletteColor[],
  pinned: PinnedColors = emptyPinned(),
): WallpaperPatches | null {
  if (palette.length === 0) return null;
  const accents = pickAccents(palette, scheme.colors.dark.ui.color.primary);
  const patches = {} as Record<SchemeVariantName, SchemeColorPatch>;
  for (const variant of SCHEME_VARIANTS) {
    const draft = generateVariantUi(scheme.colors[variant], variant, accents, pinned[variant]);
    patches[variant] = draftToPatch(draft, pinned[variant]);
  }
  return { patches, accents };
}
