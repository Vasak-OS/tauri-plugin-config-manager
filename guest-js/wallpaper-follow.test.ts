import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  bordeFuerteSobre,
  contraste,
  contrastRatio,
  enableSchemeTransition,
  followWallpaper,
  type FollowWallpaperDeps,
  luminancia,
  luminance,
  MIN_TEXT_CONTRAST,
  MINIMO_TEXTO,
  readWallpaperState,
  SCHEME_TRANSITION_CLASS,
  type SchemeData,
  strongBorderOn,
  textOn,
  textoSobre,
  type VSKConfig,
  type WallpaperPixels,
  withWallpaperState,
} from "./index";

/**
 * «Seguir al fondo» como lo corre el escritorio: `followWallpaper` con la
 * configuración, el esquema, la lectura del fondo y el guardado en dobles.
 * Nada escribe en `~/.config` ni en `vasak.conf`.
 */

const FIXTURE = new URL("../tests/fixtures/schemes/vasak-default.json", import.meta.url);
const custom = (follow: boolean, extra: Partial<{ source: string; pinned: string[] }> = {}) => {
  const base = JSON.parse(readFileSync(FIXTURE, "utf8")) as SchemeData;
  base.id = "custom";
  return withWallpaperState(base, {
    follow,
    source: extra.source ?? "",
    pinned: { dark: (extra.pinned ?? []) as never[], light: [] },
  });
};

const solid = (r: number, g: number, b: number): WallpaperPixels => {
  const data = new Uint8Array(96 * 54 * 3);
  for (let i = 0; i < data.length; i += 3) {
    data[i] = r;
    data[i + 1] = g;
    data[i + 2] = b;
  }
  return { width: 96, height: 54, data };
};

function harness(scheme: SchemeData | null, wallpaper: string, colorScheme = "custom") {
  const saved: SchemeData[] = [];
  const read: string[] = [];
  let onDisk = scheme;
  const config = {
    style: { darkmode: true, "color-scheme": colorScheme, radius: 8 },
    desktop: { wallpaper: wallpaper ? [wallpaper] : [], iconsize: 48, showfiles: true, showhiddenfiles: false },
  } as unknown as VSKConfig;
  const deps: FollowWallpaperDeps = {
    readConfig: async () => config,
    loadScheme: async (id) => (id === "custom" && onDisk ? JSON.parse(JSON.stringify(onDisk)) : null),
    readPixels: async (path) => {
      read.push(path);
      if (path.includes("roto")) throw new Error("no decodifica");
      return path.includes("rojo") ? solid(190, 30, 40) : solid(30, 120, 110);
    },
    save: async (next) => {
      saved.push(next);
      onDisk = next;
    },
  };
  return { deps, saved, read };
}

describe("followWallpaper", () => {
  test("con otro esquema en uso no hace nada", async () => {
    const h = harness(custom(true), "/f/bosque.jpg", "vasak-default");
    expect(await followWallpaper(h.deps)).toBe("not-custom");
    expect(h.saved).toHaveLength(0);
  });

  test("apagado no lee el fondo ni guarda", async () => {
    const h = harness(custom(false), "/f/bosque.jpg");
    expect(await followWallpaper(h.deps)).toBe("off");
    expect(h.read).toHaveLength(0);
    expect(h.saved).toHaveLength(0);
  });

  test("sin Personalizado o sin fondo, nada", async () => {
    expect(await followWallpaper(harness(null, "/f/bosque.jpg").deps)).toBe("no-scheme");
    expect(await followWallpaper(harness(custom(true), "").deps)).toBe("no-wallpaper");
  });

  test("prendido, saca los colores del fondo, anota el origen y no toca la terminal", async () => {
    const before = custom(true);
    const h = harness(before, "/f/bosque.jpg");
    expect(await followWallpaper(h.deps)).toBe("applied");
    const after = h.saved[0] as SchemeData;
    expect(after.colors.dark.ui.color.primary).not.toBe(before.colors.dark.ui.color.primary);
    expect(after.colors.dark.terminal).toEqual(before.colors.dark.terminal);
    expect(readWallpaperState(after).source).toBe("/f/bosque.jpg");
    for (const variant of ["dark", "light"] as const) {
      const ui = after.colors[variant].ui;
      expect(contrastRatio(ui.text["on-primary"], ui.color.primary)).toBeGreaterThanOrEqual(4.5);
      expect(contrastRatio(ui.text.main, ui.surface)).toBeGreaterThanOrEqual(4.5);
    }
  });

  test("su propio guardado no entra en un bucle", async () => {
    const h = harness(custom(true), "/f/bosque.jpg");
    await followWallpaper(h.deps);
    expect(await followWallpaper(h.deps)).toBe("unchanged");
    expect(h.read).toEqual(["/f/bosque.jpg"]);
  });

  test("un color fijado a mano sobrevive", async () => {
    const pinned = custom(true, { pinned: ["ui.color.primary"] });
    pinned.colors.dark.ui.color.primary = "#ffd700";
    const h = harness(pinned, "/f/rojo.jpg");
    expect(await followWallpaper(h.deps)).toBe("applied");
    expect(h.saved[0]?.colors.dark.ui.color.primary).toBe("#ffd700");
  });

  test("un fondo que no se puede leer no escribe nada", async () => {
    const h = harness(custom(true), "/f/roto.mp4");
    expect(await followWallpaper(h.deps)).toBe("unreadable");
    expect(h.saved).toHaveLength(0);
  });

  test("un guardado que falla se informa", async () => {
    const h = harness(custom(true), "/f/bosque.jpg");
    h.deps.save = async () => {
      throw new Error("disco lleno");
    };
    expect(await followWallpaper(h.deps)).toBe("save-failed");
  });
});

describe("los nombres de antes del contraste", () => {
  test("son las mismas funciones y valores", () => {
    expect(contraste).toBe(contrastRatio);
    expect(luminancia).toBe(luminance);
    expect(textoSobre).toBe(textOn);
    expect(bordeFuerteSobre).toBe(strongBorderOn);
    expect(MINIMO_TEXTO).toBe(MIN_TEXT_CONTRAST);
  });
});

describe("el fundido del esquema", () => {
  type FakeRoot = { classes: Set<string>; classList: { contains(c: string): boolean; add(c: string): void } };
  const fakeRoot = (): FakeRoot => {
    const classes = new Set<string>();
    return {
      classes,
      classList: { contains: (c) => classes.has(c), add: (c) => void classes.add(c) },
    };
  };

  test("la clase llega dos cuadros después, no en el acto", () => {
    const frames: FrameRequestCallback[] = [];
    const original = globalThis.requestAnimationFrame;
    globalThis.requestAnimationFrame = (cb: FrameRequestCallback) => frames.push(cb);
    try {
      const root = fakeRoot();
      enableSchemeTransition(root as unknown as HTMLElement);
      expect(root.classes.has(SCHEME_TRANSITION_CLASS)).toBe(false);
      frames.shift()?.(0);
      expect(root.classes.has(SCHEME_TRANSITION_CLASS)).toBe(false);
      frames.shift()?.(16);
      expect(root.classes.has(SCHEME_TRANSITION_CLASS)).toBe(true);
      // Ya puesta, no se vuelve a pedir nada.
      enableSchemeTransition(root as unknown as HTMLElement);
      expect(frames).toHaveLength(0);
    } finally {
      globalThis.requestAnimationFrame = original;
    }
  });

  test("sin pintado no se pone", () => {
    const original = globalThis.requestAnimationFrame;
    // @ts-expect-error: un entorno sin requestAnimationFrame
    globalThis.requestAnimationFrame = undefined;
    try {
      const root = fakeRoot();
      enableSchemeTransition(root as unknown as HTMLElement);
      expect(root.classes.size).toBe(0);
    } finally {
      globalThis.requestAnimationFrame = original;
    }
  });

  test("la pone loadConfig del store, después de aplicar el esquema", () => {
    const source = readFileSync(new URL("./index.ts", import.meta.url), "utf8");
    const load = source.slice(source.indexOf("const loadConfig = async"), source.indexOf("const setMode"));
    expect(load.indexOf("await setProperties()")).toBeLessThan(load.indexOf("enableSchemeTransition()"));
  });
});
