import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { fileURLToPath } from 'node:url';
import { createPinia, setActivePinia } from 'pinia';
import {
	contrastRatio,
	focusOn,
	MIN_NON_TEXT_CONTRAST,
	mixHex,
	type SchemeData,
	strongBorderOn,
	type UiColors,
	useConfigStore,
} from './index';

/**
 * `--ui-focus`: el anillo de foco, teñido de la marca y visible (issue #31).
 *
 * WCAG 1.4.11 pide 3:1 para el indicador de foco contra lo que tenga al lado.
 * El primario del esquema de fábrica en claro, `#dd7878`, no llega sobre el
 * fondo; vue-libvasak tenía que adivinar un respaldo en CSS. Ahora el plugin
 * calcula uno por variante y lo escribe junto a `--text-on-error`.
 *
 * Como en `text-on-error.test.ts`, lo que importa se prueba **aplicando el
 * esquema de verdad** con `loadConfig` y mirando qué quedó escrito: el cálculo
 * puede estar bien y la variable salir con el sufijo equivocado o no salir.
 * Abajo, la función pura por su cuenta, en el estilo de `contraste.test.ts`.
 */

const raiz = fileURLToPath(new URL('..', import.meta.url));
const vasakDefault = (await Bun.file(
	`${raiz}tests/fixtures/schemes/vasak-default.json`
).json()) as SchemeData;

/** Un acento claro, dorado: sobre el fondo claro no llega ni a 2:1. */
const ACENTO_CLARO = '#e5c890';

const g = globalThis as Record<string, unknown>;
let written: Map<string, string>;
let scheme: SchemeData;
let hadWindow = false;

beforeEach(() => {
	written = new Map();
	scheme = structuredClone(vasakDefault);
	hadWindow = 'window' in g;
	if (!hadWindow) g.window = g;

	g.document = {
		documentElement: {
			classList: { add() {}, remove() {} },
			style: {
				fontFamily: '',
				setProperty: (name: string, value: string) => written.set(name, value),
				removeProperty: (name: string) => {
					written.delete(name);
					return '';
				},
			},
		},
	};

	g.__TAURI_INTERNALS__ = {
		invoke: async (cmd: string) => {
			if (cmd === 'plugin:config-manager|read_config') {
				return JSON.stringify({
					style: { darkmode: false, 'color-scheme': scheme.id, radius: 8 },
				});
			}
			if (cmd === 'plugin:config-manager|get_scheme_by_id') {
				return { path: `/usr/share/vasak/schemes/${scheme.id}.json`, scheme };
			}
			throw new Error(`comando inesperado: ${cmd}`);
		},
		transformCallback: () => 0,
	};

	setActivePinia(createPinia());
});

afterEach(() => {
	delete g.__TAURI_INTERNALS__;
	delete g.document;
	if (!hadWindow) delete g.window;
});

/** El foco escrito para una variante llega a 3:1 contra su fondo y su superficie. */
function expectVisible(variable: string, ui: UiColors) {
	const foco = written.get(variable);
	expect(foco).toBeDefined();
	expect(contrastRatio(foco!, ui.background)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
	expect(contrastRatio(foco!, ui.surface)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
}

describe('--ui-focus al aplicar el esquema', () => {
	test('con el esquema de fábrica, las dos variantes llegan a 3:1 contra fondo y superficie', async () => {
		await useConfigStore().loadConfig();

		expectVisible('--ui-focus', scheme.colors.light.ui);
		expectVisible('--ui-focus-dark', scheme.colors.dark.ui);
	});

	test('en claro, el primario de fábrica se acerca al texto hasta llegar', async () => {
		// #dd7878 sobre #eff1f5 da 2,64:1: no puede ir tal cual. El primer paso
		// que llega a los dos fondos es #956471 (4,28:1 y 3,13:1).
		expect(contrastRatio('#dd7878', '#eff1f5')).toBeLessThan(MIN_NON_TEXT_CONTRAST);

		await useConfigStore().loadConfig();

		expect(written.get('--ui-focus')).toBe('#956471');
	});

	test('cuando el primario ya llega, se usa tal cual', async () => {
		// En oscuro, #eba0ac ya pasa 3:1 contra #1e1e2e y #313244: tocarlo sería
		// alejarse de la marca sin motivo.
		await useConfigStore().loadConfig();

		expect(written.get('--ui-focus-dark')).toBe('#eba0ac');
	});

	test('con un acento claro, el foco sigue llegando a 3:1 contra los dos', async () => {
		scheme.id = 'dorado';
		scheme.colors.light.ui.color.primary = ACENTO_CLARO;
		scheme.colors.dark.ui.color.primary = ACENTO_CLARO;

		await useConfigStore().loadConfig();

		expect(written.get('--ui-focus')).not.toBe(ACENTO_CLARO);
		expectVisible('--ui-focus', scheme.colors.light.ui);
		expectVisible('--ui-focus-dark', scheme.colors.dark.ui);
	});

	test('si ni el texto llega, no se escribe esa variante y queda el respaldo en CSS', async () => {
		// Variante clara rota: texto, primario y superficie casi iguales al fondo.
		// La oscura queda sana, y tiene que seguir escribiéndose: un esquema con
		// una mitad rota no se lleva puesta la otra.
		scheme.id = 'roto';
		scheme.colors.light.ui.text.main = '#e6e9ef';
		scheme.colors.light.ui.color.primary = '#dce0e8';
		scheme.colors.light.ui.surface = '#e6e9ef';

		await useConfigStore().loadConfig();

		expect(written.has('--ui-focus')).toBe(false);
		expect(written.get('--ui-focus-dark')).toBe('#eba0ac');
	});
});

/**
 * Cambiar de esquema: lo que el anterior escribió y el nuevo no puede calcular
 * se **quita**. Si quedara, taparía el respaldo en CSS de vue-libvasak con un
 * color pensado para otro fondo (señalado por CodeRabbit en el PR #34).
 *
 * Por eso cada prueba aplica dos esquemas sobre el mismo `document`: el de
 * fábrica, que sí escribe las dos variantes, y después uno con la clara rota.
 */
describe('al pasar de un esquema que llega a uno que no', () => {
	/** Rompe la variante clara: todo casi igual al fondo `#eff1f5`. */
	function romperClaro() {
		scheme = structuredClone(vasakDefault);
		scheme.id = 'roto';
		scheme.colors.light.ui.text.main = '#e6e9ef';
		scheme.colors.light.ui.text.muted = '#e6e9ef';
		scheme.colors.light.ui.color.primary = '#dce0e8';
		scheme.colors.light.ui.surface = '#e6e9ef';
	}

	test('el --ui-focus del esquema anterior no queda pegado y el oscuro se conserva', async () => {
		await useConfigStore().loadConfig();
		expect(written.has('--ui-focus')).toBe(true);
		const focoOscuro = written.get('--ui-focus-dark');
		expect(focoOscuro).toBeDefined();

		romperClaro();
		expect(focusOn(scheme.colors.light.ui)).toBeNull();
		setActivePinia(createPinia());
		await useConfigStore().loadConfig();

		expect(written.has('--ui-focus')).toBe(false);
		expect(written.get('--ui-focus-dark')).toBe(focoOscuro!);
	});

	test('el --ui-border-strong del esquema anterior no queda pegado y el oscuro se conserva', async () => {
		await useConfigStore().loadConfig();
		expect(written.has('--ui-border-strong')).toBe(true);
		const bordeOscuro = written.get('--ui-border-strong-dark');
		expect(bordeOscuro).toBeDefined();

		romperClaro();
		// Ninguno de los tres candidatos llega a 3:1 contra el fondo claro.
		const claro = scheme.colors.light.ui;
		for (const candidato of [claro.text.muted, claro.surface, claro.text.main]) {
			expect(contrastRatio(candidato, claro.background)).toBeLessThan(MIN_NON_TEXT_CONTRAST);
		}
		expect(strongBorderOn(claro)).toBeNull();
		setActivePinia(createPinia());
		await useConfigStore().loadConfig();

		expect(written.has('--ui-border-strong')).toBe(false);
		expect(written.get('--ui-border-strong-dark')).toBe(bordeOscuro!);
	});
});

/** Las dos paletas del esquema de fábrica, para la función pura. */
const clara = vasakDefault.colors.light.ui;
const oscura = vasakDefault.colors.dark.ui;

describe('focusOn', () => {
	test('en claro devuelve el primer paso que llega a fondo y superficie', () => {
		const foco = focusOn(clara);

		expect(foco).toBe('#956471');
		expect(contrastRatio(foco!, clara.background)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
		expect(contrastRatio(foco!, clara.surface)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
	});

	test('mira también la superficie, que es la más exigente en claro', () => {
		// El paso anterior a #956471, #9c6671, llega contra el fondo (4,08:1)
		// pero no contra la superficie (2,99:1). Si sólo se mirara el fondo,
		// ganaría #c77276 y el anillo se perdería sobre los paneles.
		const anterior = mixHex(clara.color.primary, clara.text.main, 0.45)!;
		expect(contrastRatio(anterior, clara.background)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
		expect(contrastRatio(anterior, clara.surface)).toBeLessThan(MIN_NON_TEXT_CONTRAST);

		const foco = focusOn(clara)!;
		expect(contrastRatio(foco, clara.surface)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
		expect(foco).toBe('#956471');
	});

	test('cuando el primario ya llega, lo devuelve sin tocar', () => {
		expect(focusOn(oscura)).toBe(oscura.color.primary);
	});

	test('con un acento claro llega a 3:1 contra los dos', () => {
		const dorada = { ...clara, color: { ...clara.color, primary: ACENTO_CLARO } };
		const foco = focusOn(dorada);

		expect(foco).not.toBeNull();
		expect(contrastRatio(foco!, dorada.background)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
		expect(contrastRatio(foco!, dorada.surface)).toBeGreaterThanOrEqual(MIN_NON_TEXT_CONTRAST);
	});

	test('si ni el texto llega, devuelve null', () => {
		const rota = {
			...clara,
			color: { ...clara.color, primary: '#dce0e8' },
			text: { ...clara.text, main: '#e6e9ef' },
			surface: '#e6e9ef',
		};

		expect(focusOn(rota)).toBeNull();
	});
});

describe('mixHex', () => {
	test('en 0 da el color de partida y en 1 el de llegada', () => {
		expect(mixHex('#dd7878', '#4c4f69', 0)).toBe('#dd7878');
		expect(mixHex('#dd7878', '#4c4f69', 1)).toBe('#4c4f69');
	});

	test('a mitad de camino da lo mismo que color-mix en sRGB', () => {
		// color-mix(in srgb, #000000, #ffffff 50%) = rgb(127.5 127.5 127.5).
		expect(mixHex('#000000', '#ffffff', 0.5)).toBe('#808080');
	});

	test('un color ilegible no se mezcla', () => {
		expect(mixHex('rojo', '#ffffff', 0.5)).toBeNull();
	});
});
