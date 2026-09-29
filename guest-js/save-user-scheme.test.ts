import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { type Scheme, type SchemeData, saveUserScheme } from './index';

/**
 * `saveUserScheme` sólo traduce a `invoke`, pero el nombre del comando y la
 * forma del argumento son el contrato con el backend: un error acá no lo ve el
 * compilador, se ve como un «command not found» en tiempo de ejecución.
 *
 * Se reemplaza el `invoke` que usa `@tauri-apps/api` —el de
 * `__TAURI_INTERNALS__`— en lugar de simular el módulo: dos simulaciones del
 * mismo módulo en archivos distintos se pisan en el CI.
 */
type Call = { cmd: string; args: unknown };
let calls: Call[] = [];

const variant = {
	ui: {
		color: { primary: '#eba0ac', secondary: '#cba6f7' },
		text: {
			main: '#cdd6f4',
			muted: '#a6adc8',
			'on-primary': '#1e1e2e',
			'on-secondary': '#1e1e2e',
		},
		background: '#1e1e2e',
		border: '#11111b',
		surface: '#313244',
	},
	terminal: {
		foreground: '#cdd6f4',
		background: '#1e1e2e',
		cursor: '#eba0ac',
		ansi: {
			black: '#181825',
			red: '#f38ba8',
			green: '#a6e3a1',
			yellow: '#f9e2af',
			blue: '#89b4fa',
			magenta: '#eba0ac',
			cyan: '#89dceb',
			white: '#cdd6f4',
			brightBlack: '#181825',
			brightRed: '#f38ba8',
			brightGreen: '#a6e3a1',
			brightYellow: '#f9e2af',
			brightBlue: '#89b4fa',
			brightMagenta: '#eba0ac',
			brightCyan: '#89dceb',
			brightWhite: '#cdd6f4',
		},
	},
};

const custom: SchemeData = {
	id: 'custom',
	name: 'Personalizado',
	author: 'Alguien',
	description: 'Clonado de Vasak Default',
	version: '0.0.1',
	colors: { dark: variant, light: variant },
};

const g = globalThis as Record<string, unknown>;
let hadWindow = false;

beforeEach(() => {
	calls = [];
	hadWindow = 'window' in g;
	if (!hadWindow) g.window = g;
	g.__TAURI_INTERNALS__ = {
		invoke: async (cmd: string, args: unknown): Promise<Scheme> => {
			calls.push({ cmd, args });
			return {
				path: '/home/alguien/.config/vasak/schemes/custom.json',
				scheme: (args as { scheme: SchemeData }).scheme,
			};
		},
		transformCallback: () => 0,
	};
});

afterEach(() => {
	delete g.__TAURI_INTERNALS__;
	if (!hadWindow) delete g.window;
});

describe('saveUserScheme', () => {
	test('llama a save_user_scheme con el esquema en `scheme`', async () => {
		await saveUserScheme(custom);

		expect(calls).toHaveLength(1);
		expect(calls[0].cmd).toBe('plugin:config-manager|save_user_scheme');
		expect(calls[0].args).toEqual({ scheme: custom });
	});

	test('devuelve el esquema guardado con su ruta', async () => {
		const saved = await saveUserScheme(custom);

		expect(saved.path).toBe('/home/alguien/.config/vasak/schemes/custom.json');
		expect(saved.scheme.id).toBe('custom');
		expect(saved.scheme.colors.dark.ui.text['on-secondary']).toBe('#1e1e2e');
	});

	test('on-secondary es opcional en el tipo', () => {
		// Si dejara de serlo, esto no compila: los esquemas anteriores no lo traen.
		const { 'on-secondary': _omitted, ...rest } = variant.ui.text;
		const older: SchemeData['colors']['dark']['ui']['text'] = rest;
		expect(older['on-secondary']).toBeUndefined();
	});
});
