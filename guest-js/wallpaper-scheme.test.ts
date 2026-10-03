import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { hexToOklch, hexToRgb, oklchToHex, rgbToHex, rgbToOklab } from './color-space';
import { contrastRatio as contraste } from './contrast';
import type { SchemeData as SchemeFile, ThemeVariant as SchemeVariantColors } from './index';
import { applyColorPatch, SCHEME_VARIANTS } from './scheme-patch';
import { extractPalette, type WallpaperPixels } from './wallpaper-palette';
import {
	buildWallpaperPatches,
	ensureContrast,
	omitPaths,
	pickAccents,
	readWallpaperState,
	uiPathsOf,
	WALLPAPER_STATE_KEY,
	withWallpaperState,
} from './wallpaper-scheme';

/**
 * «Automático»: del fondo de pantalla a los colores de interfaz del esquema
 * «Personalizado» (Vasak-OS/vasak-settings#134).
 *
 * Los fondos son imágenes sintéticas de 96×54, el tamaño de la muestra que da
 * `wallpaper_pixels`: un bosque, un fondo rojo y un atardecer, que son los
 * tres del video de referencia, y los extremos que pide el issue.
 */

const FIXTURE = new URL('../tests/fixtures/schemes/vasak-default.json', import.meta.url);
const baseScheme = (): SchemeFile => JSON.parse(readFileSync(FIXTURE, 'utf8')) as SchemeFile;

const W = 96;
const H = 54;
type Painter = (x: number, y: number) => [number, number, number];

function image(paint: Painter): WallpaperPixels {
	const data = new Uint8Array(W * H * 3);
	for (let y = 0; y < H; y++) {
		for (let x = 0; x < W; x++) {
			const [r, g, b] = paint(x, y);
			const index = (y * W + x) * 3;
			data[index] = r;
			data[index + 1] = g;
			data[index + 2] = b;
		}
	}
	return { width: W, height: H, data };
}

const WALLPAPERS: Record<string, Painter> = {
	// Copas verde azuladas abajo, cielo claro arriba.
	forest: (x, y) => (y < 18 ? [168, 200, 210] : [18 + (x % 9), 92 + (y % 13), 84 + (x % 7)]),
	red: (x) => [176 + (x % 24), 28, 40],
	// Cielo naranja que se degrada, agua azul oscura abajo.
	sunset: (_x, y) => (y < 26 ? [250, 150 - y * 2, 50] : [24, 36, 76]),
	nearBlack: () => [3, 3, 5],
	nearWhite: () => [252, 252, 250],
	monochrome: (x) => [x * 2, x * 2, x * 2],
	saturated: (x) => (x < 48 ? [0, 255, 0] : [255, 0, 255]),
};

const applied = (scheme: SchemeFile, painter: Painter): SchemeFile => {
	const built = buildWallpaperPatches(scheme, extractPalette(image(painter)));
	if (!built) throw new Error('sin paleta');
	let next = scheme;
	for (const variant of SCHEME_VARIANTS)
		next = applyColorPatch(next, variant, built.patches[variant]);
	return next;
};

const hueOf = (hex: string) => hexToOklch(hex)?.h ?? Number.NaN;

/** Los pares que tienen que dar el mínimo, con su mínimo. */
function contrastPairs(colors: SchemeVariantColors): Array<[string, number, number]> {
	const ui = colors.ui;
	const pairs: Array<[string, number, number]> = [
		['main/background', contraste(ui.text.main, ui.background), 4.5],
		['main/surface', contraste(ui.text.main, ui.surface), 4.5],
		['muted/background', contraste(ui.text.muted, ui.background), 4.5],
		['muted/surface', contraste(ui.text.muted, ui.surface), 4.5],
		['on-primary/primary', contraste(ui.text['on-primary'], ui.color.primary), 4.5],
		['primary/background', contraste(ui.color.primary, ui.background), 3],
		['secondary/background', contraste(ui.color.secondary, ui.background), 3],
	];
	const onSecondary = ui.text['on-secondary'];
	if (typeof onSecondary === 'string') {
		pairs.push(['on-secondary/secondary', contraste(onSecondary, ui.color.secondary), 4.5]);
	}
	return pairs;
}

describe('el espacio de color', () => {
	test('un hex va y vuelve por OKLCH sin cambiar', () => {
		for (const hex of ['#000000', '#ffffff', '#eba0ac', '#1e1e2e', '#2a9d8f', '#ff00ff']) {
			const lch = hexToOklch(hex);
			expect(lch).not.toBeNull();
			expect(oklchToHex(lch as NonNullable<typeof lch>)).toBe(hex);
		}
	});

	test('un color fuera de la gama baja el croma y conserva el tono', () => {
		const hex = oklchToHex({ l: 0.75, c: 0.4, h: 190 });
		expect(hexToRgb(hex)).not.toBeNull();
		expect(Math.abs(hueOf(hex) - 190)).toBeLessThan(3);
	});

	test('el blanco tiene luminosidad 1 y el negro 0', () => {
		expect(rgbToOklab({ r: 255, g: 255, b: 255 }).l).toBeCloseTo(1, 3);
		expect(rgbToOklab({ r: 0, g: 0, b: 0 }).l).toBeCloseTo(0, 5);
		expect(rgbToHex({ r: 300, g: -5, b: 127.6 })).toBe('#ff0080');
	});
});

describe('la paleta del fondo', () => {
	test('el mismo fondo da siempre la misma paleta', () => {
		const first = extractPalette(image(WALLPAPERS.sunset as Painter));
		const second = extractPalette(image(WALLPAPERS.sunset as Painter));
		expect(second).toEqual(first);
	});

	test('las superficies suman uno y van de la mayor a la menor', () => {
		const palette = extractPalette(image(WALLPAPERS.forest as Painter));
		const total = palette.reduce((sum, color) => sum + color.population, 0);
		expect(total).toBeCloseTo(1, 6);
		for (let i = 1; i < palette.length; i++) {
			expect(palette[i - 1]?.population ?? 0).toBeGreaterThanOrEqual(palette[i]?.population ?? 0);
		}
	});

	test('cada color dice dónde está en la imagen', () => {
		// Mitad izquierda verde, mitad derecha magenta.
		const palette = extractPalette(image(WALLPAPERS.saturated as Painter));
		const green = palette.find((color) => color.hex === '#00ff00');
		const magenta = palette.find((color) => color.hex === '#ff00ff');
		expect(green?.x).toBeLessThan(0.5);
		expect(magenta?.x).toBeGreaterThan(0.5);
		expect(green?.y).toBeCloseTo(0.5, 1);
	});

	test('una muestra más corta que su tamaño declarado no da paleta', () => {
		const pixels = image(WALLPAPERS.forest as Painter);
		expect(extractPalette({ ...pixels, data: Array.from(pixels.data).slice(0, 96 * 3 * 10) })).toEqual([]);
	});

	test('una imagen vacía da una paleta vacía, y entonces no se cambia nada', () => {
		expect(extractPalette({ width: 0, height: 0, data: [] })).toEqual([]);
		expect(buildWallpaperPatches(baseScheme(), [])).toBeNull();
	});
});

describe('el acento sigue al fondo', () => {
	test('con el bosque, un verde azulado', () => {
		const scheme = applied(baseScheme(), WALLPAPERS.forest as Painter);
		const hue = hueOf(scheme.colors.dark.ui.color.primary);
		expect(hue).toBeGreaterThan(140);
		expect(hue).toBeLessThan(215);
	});

	test('con el fondo rojo, un rosa: el rojo aclarado en oscuro', () => {
		const scheme = applied(baseScheme(), WALLPAPERS.red as Painter);
		const dark = hexToOklch(scheme.colors.dark.ui.color.primary);
		expect(hueOf(scheme.colors.dark.ui.color.primary)).toBeLessThan(40);
		expect(dark?.l).toBeGreaterThan(0.7);
	});

	test('con el atardecer, naranja aunque el agua oscura ocupe más', () => {
		const scheme = applied(baseScheme(), WALLPAPERS.sunset as Painter);
		const hue = hueOf(scheme.colors.light.ui.color.primary);
		expect(hue).toBeGreaterThan(25);
		expect(hue).toBeLessThan(80);
	});

	test('un fondo gris conserva el tono del primario del esquema', () => {
		const palette = extractPalette(image(WALLPAPERS.monochrome as Painter));
		const accents = pickAccents(palette, '#eba0ac');
		expect(accents.fallback).toBe(true);
		expect(Math.abs(accents.primary.hue - hueOf('#eba0ac'))).toBeLessThan(1);
	});

	test('el color del fondo nunca se usa tal cual', () => {
		const palette = extractPalette(image(WALLPAPERS.saturated as Painter));
		const scheme = applied(baseScheme(), WALLPAPERS.saturated as Painter);
		const hexes = palette.map((color) => color.hex);
		for (const variant of SCHEME_VARIANTS) {
			expect(hexes).not.toContain(scheme.colors[variant].ui.color.primary);
		}
	});
});

describe('el contraste se garantiza en claro y en oscuro', () => {
	for (const [name, painter] of Object.entries(WALLPAPERS)) {
		test(`con el fondo ${name}`, () => {
			const scheme = applied(baseScheme(), painter);
			for (const variant of SCHEME_VARIANTS) {
				for (const [pair, ratio, minimum] of contrastPairs(scheme.colors[variant])) {
					expect({ variant, pair, ok: ratio >= minimum }).toEqual({ variant, pair, ok: true });
				}
			}
		});
	}

	test('también si el esquema de partida tenía pares que no llegaban', () => {
		const weak = baseScheme();
		weak.colors.light.ui.text.muted = '#c0c0c8';
		weak.colors.dark.ui.text.main = '#3a3a48';
		const scheme = applied(weak, WALLPAPERS.forest as Painter);
		for (const variant of SCHEME_VARIANTS) {
			for (const [pair, ratio, minimum] of contrastPairs(scheme.colors[variant])) {
				expect({ variant, pair, ok: ratio >= minimum }).toEqual({ variant, pair, ok: true });
			}
		}
	});

	test('ajustar sólo mueve la luminosidad, y no toca lo que ya cumple', () => {
		expect(ensureContrast('#ffffff', ['#000000'], 4.5)).toBe('#ffffff');
		const fixed = ensureContrast('#7a8a9a', ['#1e1e2e'], 7);
		expect(contraste(fixed, '#1e1e2e')).toBeGreaterThanOrEqual(7);
		expect(Math.abs(hueOf(fixed) - hueOf('#7a8a9a'))).toBeLessThan(8);
	});

	test('la terminal no cambia y la luminosidad de los neutros sigue la del esquema', () => {
		const base = baseScheme();
		const scheme = applied(base, WALLPAPERS.red as Painter);
		for (const variant of SCHEME_VARIANTS) {
			expect(scheme.colors[variant].terminal).toEqual(base.colors[variant].terminal);
			const before = hexToOklch(base.colors[variant].ui.background)?.l ?? 0;
			const after = hexToOklch(scheme.colors[variant].ui.background)?.l ?? 0;
			expect(Math.abs(after - before)).toBeLessThan(0.02);
		}
	});
});

describe('los colores fijados a mano', () => {
	test('un color fijado no se recalcula, y lo de alrededor se ajusta a él', () => {
		const base = baseScheme();
		base.colors.dark.ui.color.primary = '#ffd700';
		const built = buildWallpaperPatches(base, extractPalette(image(WALLPAPERS.forest as Painter)), {
			dark: ['ui.color.primary'],
			light: [],
		});
		const dark = built?.patches.dark.ui;
		expect(dark?.color?.primary).toBeUndefined();
		const onPrimary = dark?.text?.['on-primary'] as string;
		expect(contraste(onPrimary, '#ffd700')).toBeGreaterThanOrEqual(4.5);
		// La otra variante no tiene nada fijado: se recalcula entera.
		expect(built?.patches.light.ui?.color?.primary).toBeDefined();
	});

	test('omitPaths saca sólo lo nombrado', () => {
		const patch = {
			ui: { color: { primary: '#111111', secondary: '#222222' }, background: '#333333' },
		};
		expect(omitPaths(patch, ['ui.color.primary', 'ui.background'])).toEqual({
			ui: { color: { secondary: '#222222' } },
		});
		expect(uiPathsOf(patch)).toEqual(['ui.color.primary', 'ui.color.secondary', 'ui.background']);
	});
});

describe('el estado del modo automático en custom.json', () => {
	test('por omisión, apagado y sin nada fijado', () => {
		expect(readWallpaperState(baseScheme())).toEqual({
			follow: false,
			source: '',
			pinned: { dark: [], light: [] },
		});
	});

	test('un archivo editado a mano con basura se lee sano', () => {
		const scheme = baseScheme();
		(scheme as Record<string, unknown>)[WALLPAPER_STATE_KEY] = {
			follow: 'sí',
			source: 42,
			pinned: { dark: ['ui.color.primary', 'cualquier.cosa', 'ui.color.primary'], light: 'no' },
		};
		expect(readWallpaperState(scheme)).toEqual({
			follow: false,
			source: '',
			pinned: { dark: ['ui.color.primary'], light: [] },
		});
	});

	test('escribirlo no toca el esquema recibido ni el resto de las claves', () => {
		const scheme = baseScheme();
		const state = {
			follow: true,
			source: '/f.jpg',
			pinned: { dark: [], light: ['ui.surface' as const] },
		};
		const next = withWallpaperState(scheme, state);
		expect((scheme as Record<string, unknown>)[WALLPAPER_STATE_KEY]).toBeUndefined();
		expect(readWallpaperState(next)).toEqual(state);
		expect(next.colors).toEqual(scheme.colors);
	});
});
