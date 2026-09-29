import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { fileURLToPath } from 'node:url';
import { createPinia, setActivePinia } from 'pinia';
import { contraste, MINIMO_TEXTO, type SchemeData, useConfigStore } from './index';

/**
 * `--text-on-error`: el texto sobre `bg-status-error`, calculado como el de los
 * colores de marca.
 *
 * Hasta acá el plugin escribía el fondo del error —el rojo de la terminal— y no
 * el texto que va encima, así que cada aplicación tenía que fijar uno propio.
 * vasak-settings lo hizo con dos colores fijos, que sirven para el esquema por
 * omisión y para ningún otro garantizado.
 *
 * Se prueba **aplicando el esquema de verdad**, con `loadConfig`, y no llamando
 * a `textoSobre` por su cuenta: lo que se rompe acá no es el cálculo, que ya
 * tiene sus pruebas en `contraste.test.ts`, sino que alguien escriba la
 * variable con el color que no es o se olvide de la variante oscura.
 *
 * El `invoke` se reemplaza por el de `__TAURI_INTERNALS__` y no simulando el
 * módulo, por lo mismo que en `save-user-scheme.test.ts`.
 */

const raiz = fileURLToPath(new URL('..', import.meta.url));
const vasakDefault = (await Bun.file(
	`${raiz}tests/fixtures/schemes/vasak-default.json`
).json()) as SchemeData;

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

describe('--text-on-error', () => {
	test('se escribe en las dos variantes, legible sobre el rojo de cada una', async () => {
		await useConfigStore().loadConfig();

		const claro = written.get('--text-on-error');
		const oscuro = written.get('--text-on-error-dark');

		expect(claro).toBeDefined();
		expect(oscuro).toBeDefined();
		expect(contraste(claro!, written.get('--status-error')!)).toBeGreaterThanOrEqual(MINIMO_TEXTO);
		expect(contraste(oscuro!, written.get('--status-error-dark')!)).toBeGreaterThanOrEqual(
			MINIMO_TEXTO
		);
	});

	test('con el esquema por omisión da los mismos que vasak-settings tenía fijos', async () => {
		// #eff1f5 y #1e1e2e son los fondos de las dos variantes: el cálculo se
		// queda en la familia del esquema. Si esto cambia, el token fijo de
		// vasak-settings deja de coincidir con lo que escribe el plugin.
		await useConfigStore().loadConfig();

		expect(written.get('--text-on-error')).toBe('#eff1f5');
		expect(written.get('--text-on-error-dark')).toBe('#1e1e2e');
	});

	test('sigue al rojo del esquema y no a un color fijo', async () => {
		// Un rojo claro en la variante clara y uno oscuro en la oscura: al revés
		// del esquema por omisión. Con un valor fijo, los dos quedarían ilegibles.
		scheme.id = 'invertido';
		scheme.colors.light.terminal.ansi.red = '#ffb3c1';
		scheme.colors.dark.terminal.ansi.red = '#7a0019';

		await useConfigStore().loadConfig();

		const claro = written.get('--text-on-error')!;
		const oscuro = written.get('--text-on-error-dark')!;

		expect(claro).not.toBe('#eff1f5');
		expect(oscuro).not.toBe('#1e1e2e');
		expect(contraste(claro, '#ffb3c1')).toBeGreaterThanOrEqual(MINIMO_TEXTO);
		expect(contraste(oscuro, '#7a0019')).toBeGreaterThanOrEqual(MINIMO_TEXTO);
	});

	test('sale del rojo de la terminal y no del rojo brillante', async () => {
		// `--status-error` es `ansi.red`. Si el texto se calculara sobre otro
		// color, este rojo brillante casi blanco lo llevaría a negro.
		scheme.id = 'brillante';
		scheme.colors.light.terminal.ansi.brightRed = '#ffe0e6';

		await useConfigStore().loadConfig();

		expect(written.get('--status-error')).toBe('#d20f39');
		expect(written.get('--text-on-error')).toBe('#eff1f5');
	});
});
