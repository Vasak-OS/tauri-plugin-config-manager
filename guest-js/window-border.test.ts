import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { fileURLToPath } from 'node:url';
import { createPinia, setActivePinia } from 'pinia';
import * as plugin from './index';
import { type SchemeData, useConfigStore } from './index';

/**
 * `style.border`: el borde de afuera —la ventana entera, el panel, el centro de
 * control y los emergentes del escritorio—.
 *
 * Dos partes: la función pura que traduce la configuración a las dos variables,
 * y el store, que es quien las escribe en `:root` al cargar. Lo que se protege
 * es lo que vería quien usa el sistema: elegir «grueso» o «de acento» cambia el
 * borde, y un archivo sin la clave, o con un valor que no se reconoce, deja el
 * borde de siempre en lugar de una ventana sin borde.
 *
 * El `invoke` se reemplaza por el de `__TAURI_INTERNALS__`, como en
 * `text-on-error.test.ts`.
 */

const raiz = fileURLToPath(new URL('..', import.meta.url));
const vasakDefault = (await Bun.file(
	`${raiz}tests/fixtures/schemes/vasak-default.json`
).json()) as SchemeData;

/**
 * Se toma del módulo y no con un `import { windowBorderProperties }`: sin el
 * cambio la exportación no existe, y así la prueba falla en la aserción en vez
 * de no cargar.
 */
const windowBorderProperties = (border: unknown) => {
	const fn = (plugin as Record<string, unknown>).windowBorderProperties;
	expect(typeof fn).toBe('function');
	return (fn as (b: unknown) => { width: string; color: string | null })(border);
};

describe('windowBorderProperties', () => {
	test('el borde de siempre es fino y del color del esquema', () => {
		expect(windowBorderProperties({ width: 'normal', color: 'scheme' })).toEqual({
			width: '1px',
			color: null,
		});
	});

	test('grueso son 2 px', () => {
		expect(windowBorderProperties({ width: 'thick', color: 'scheme' }).width).toBe('2px');
	});

	test('el de acento sigue al primario del esquema y no a un color resuelto', () => {
		// Como referencia a la variable sigue al modo oscuro sin reescribirse.
		expect(windowBorderProperties({ width: 'normal', color: 'accent' }).color).toBe(
			'var(--use-primary)'
		);
	});

	test('sin la clave, un archivo anterior tiene el borde de siempre', () => {
		for (const ausente of [undefined, null, {}]) {
			expect(windowBorderProperties(ausente)).toEqual({ width: '1px', color: null });
		}
	});

	test('un valor que no se reconoce cae en el borde de siempre, nunca sin borde', () => {
		expect(windowBorderProperties({ width: 'huge', color: 'rojo' })).toEqual({
			width: '1px',
			color: null,
		});
	});
});

const g = globalThis as Record<string, unknown>;
let written: Map<string, string>;
let removed: string[];
let border: unknown;
let schemeAvailable: boolean;
let hadWindow = false;

beforeEach(() => {
	written = new Map();
	removed = [];
	border = undefined;
	schemeAvailable = true;
	hadWindow = 'window' in g;
	if (!hadWindow) g.window = g;

	g.document = {
		documentElement: {
			classList: { add() {}, remove() {} },
			style: {
				fontFamily: '',
				setProperty: (name: string, value: string) => written.set(name, value),
				removeProperty: (name: string) => {
					removed.push(name);
					written.delete(name);
					return '';
				},
			},
		},
	};

	g.__TAURI_INTERNALS__ = {
		invoke: async (cmd: string) => {
			if (cmd === 'plugin:config-manager|read_config') {
				const style: Record<string, unknown> = {
					darkmode: false,
					'color-scheme': vasakDefault.id,
					radius: 8,
				};
				if (border !== undefined) style.border = border;
				return JSON.stringify({ style });
			}
			if (cmd === 'plugin:config-manager|get_scheme_by_id') {
				if (!schemeAvailable) return null;
				return {
					path: `/usr/share/vasak/schemes/${vasakDefault.id}.json`,
					scheme: structuredClone(vasakDefault),
				};
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

describe('el store aplica el borde de afuera', () => {
	test('grueso y de acento escribe las dos variables en :root', async () => {
		border = { width: 'thick', color: 'accent' };
		await useConfigStore().loadConfig();

		expect(written.get('--window-border-width')).toBe('2px');
		expect(written.get('--ui-window-border')).toBe('var(--use-primary)');
	});

	test('un archivo sin la clave deja el borde fino y quita el color propio', async () => {
		await useConfigStore().loadConfig();

		expect(written.get('--window-border-width')).toBe('1px');
		expect(written.has('--ui-window-border')).toBe(false);
		expect(removed).toContain('--ui-window-border');
	});

	test('volver al color del esquema devuelve el de siempre en la ventana abierta', async () => {
		// Sin quitar la variable, la ventana se quedaría con el acento hasta
		// cerrarse: escribir `null` o nada no basta.
		const store = useConfigStore();
		border = { width: 'thick', color: 'accent' };
		await store.loadConfig();
		expect(written.get('--ui-window-border')).toBe('var(--use-primary)');

		border = { width: 'normal', color: 'scheme' };
		await store.loadConfig();

		expect(written.get('--window-border-width')).toBe('1px');
		expect(written.has('--ui-window-border')).toBe(false);
	});

	test('se aplica aunque el esquema no se pueda leer', async () => {
		border = { width: 'thick', color: 'accent' };
		schemeAvailable = false;
		await useConfigStore().loadConfig();

		expect(written.has('--primary')).toBe(false);
		expect(written.get('--window-border-width')).toBe('2px');
		expect(written.get('--ui-window-border')).toBe('var(--use-primary)');
	});
});
