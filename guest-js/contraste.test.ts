import { describe, expect, it } from 'bun:test';
import {
	bordeFuerteSobre,
	contraste,
	luminancia,
	mejorSobre,
	MINIMO_NO_TEXTO,
	MINIMO_TEXTO,
	textoSobre,
} from './index';

/** La paleta oscura del esquema por omisión de VasakOS. */
const oscura = {
	color: { primary: '#eba0ac', secondary: '#cba6f7' },
	text: { main: '#cdd6f4', muted: '#a6adc8', 'on-primary': '#cdd6f4' },
	background: '#1e1e2e',
	border: '#11111b',
	surface: '#313244',
};

/** Y la clara. */
const clara = {
	color: { primary: '#dd7878', secondary: '#8839ef' },
	text: { main: '#4c4f69', muted: '#555869', 'on-primary': '#5c5f77' },
	background: '#eff1f5',
	border: '#dce0e8',
	surface: '#ccd0da',
};

describe('contraste', () => {
	it('da los valores de la fórmula de WCAG', () => {
		// Los extremos, que son los que fijan que la fórmula esté bien.
		expect(contraste('#ffffff', '#000000')).toBeCloseTo(21, 1);
		expect(contraste('#ffffff', '#ffffff')).toBeCloseTo(1, 2);
	});

	it('es simétrico', () => {
		expect(contraste('#eba0ac', '#1e1e2e')).toBeCloseTo(contraste('#1e1e2e', '#eba0ac'), 4);
	});

	it('acepta la forma corta de tres dígitos', () => {
		expect(contraste('#fff', '#000')).toBeCloseTo(21, 1);
	});

	it('un color ilegible no puede pasar por buen contraste', () => {
		// Si devolviera un número alto, se elegiría justo el que no se puede pintar.
		expect(contraste('rojo', '#000000')).toBe(0);
		expect(contraste('', '#000000')).toBe(0);
		expect(contraste('#12345', '#000000')).toBe(0);
		expect(luminancia('#zzzzzz')).toBeNull();
	});
});

describe('textoSobre', () => {
	it('rechaza el on-primary del esquema por omisión, que era ilegible', () => {
		// El caso real: #cdd6f4 sobre #eba0ac da 1.43 contra un mínimo de 4.5, así
		// que el texto de cualquier botón de acento era casi invisible.
		expect(contraste(oscura.text['on-primary'], oscura.color.primary)).toBeLessThan(2);

		const elegido = textoSobre(oscura.color.primary, oscura.text['on-primary'], oscura);
		expect(elegido).not.toBe(oscura.text['on-primary']);
		expect(contraste(elegido, oscura.color.primary)).toBeGreaterThanOrEqual(MINIMO_TEXTO);
	});

	it('respeta el del esquema cuando sí cumple', () => {
		// Es una decisión estética de quien escribió el esquema: si se puede leer,
		// no se le discute.
		const bueno = '#1e1e2e';
		expect(textoSobre(oscura.color.primary, bueno, oscura)).toBe(bueno);
	});

	it('se queda en la familia de colores antes de caer a negro o blanco', () => {
		// Con la paleta a mano, elegir #000 sería salirse del esquema sin necesidad.
		const elegido = textoSobre(oscura.color.primary, undefined, oscura);
		expect([oscura.background, oscura.text.main, oscura.surface]).toContain(elegido);
	});

	it('funciona con cualquier acento, que es el punto', () => {
		// Acentos de todo el rango: claro, medio, oscuro y saturado. Con cada uno
		// el texto tiene que quedar legible sin tocar el esquema.
		for (const acento of ['#ffffff', '#eba0ac', '#dd7878', '#8839ef', '#1e66f5', '#000000', '#40a02b']) {
			for (const paleta of [oscura, clara]) {
				const elegido = textoSobre(acento, paleta.text['on-primary'], paleta);
				expect(contraste(elegido, acento)).toBeGreaterThanOrEqual(MINIMO_TEXTO);
			}
		}
	});

	it('nunca devuelve vacío, ni con una paleta rota', () => {
		// Sin color de texto el botón queda sin nada; el negro es el último recurso.
		const rota = { ...oscura, background: 'nope', surface: '', text: { main: '', muted: '', 'on-primary': '' } };
		expect(textoSobre('#888888', undefined, rota as never)).toBe('#000000');
	});

	it('elige el de más contraste, no el primero que pasa', () => {
		// Entre 4.6 y 9 la diferencia se nota, y no cuesta nada tomar el mejor.
		const candidatos = ['#767676', '#000000'];
		expect(mejorSobre('#ffffff', candidatos, MINIMO_TEXTO)).toBe('#000000');
	});
});

describe('bordeFuerteSobre', () => {
	it('encuentra un borde que se perciba', () => {
		// El borde del esquema da 1.14 contra el fondo: el contorno de un campo no
		// se ve. WCAG 1.4.11 pide 3:1 para lo que delimita un control.
		for (const paleta of [oscura, clara]) {
			expect(contraste(paleta.border, paleta.background)).toBeLessThan(MINIMO_NO_TEXTO);
			const borde = bordeFuerteSobre(paleta);
			expect(borde).not.toBeNull();
			expect(contraste(borde as string, paleta.background)).toBeGreaterThanOrEqual(MINIMO_NO_TEXTO);
		}
	});

	it('devuelve null si la paleta no tiene nada que llegue', () => {
		// Y quien llama deja el valor por omisión del CSS en lugar de escribir algo
		// que tampoco se ve.
		const plana = {
			color: { primary: '#ffffff', secondary: '#ffffff' },
			text: { main: '#fefefe', muted: '#fdfdfd', 'on-primary': '#ffffff' },
			background: '#ffffff',
			border: '#ffffff',
			surface: '#fefefe',
		};
		expect(bordeFuerteSobre(plana)).toBeNull();
	});
});
