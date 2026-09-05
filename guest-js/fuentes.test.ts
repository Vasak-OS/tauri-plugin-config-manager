import { describe, expect, test } from 'bun:test';
import { pilaDeFuente } from './index';

/**
 * Las tres fuentes que ofrece Configuración —título, aplicaciones y terminal—
 * se eligen ahí y las tiene que usar todo el escritorio. Lo que arma la pila es
 * esto, así que acá se fija lo que no puede fallar.
 */
describe('la pila de fuentes', () => {
	test('termina siempre en una familia genérica', () => {
		// Sin genérica, una fuente desinstalada deja a la aplicación con la del
		// motor, que no se parece en nada al resto del escritorio.
		expect(pilaDeFuente('Noto Sans', 'apps')).toBe("'Noto Sans', sans-serif");
		expect(pilaDeFuente('Noto Sans', 'title')).toBe("'Noto Sans', sans-serif");
	});

	test('la de la terminal cae en monospace, no en sans-serif', () => {
		// La diferencia que más se nota: con ancho variable, una terminal dibuja
		// mal las tablas, las barras de progreso y todo lo que se alinee por
		// columnas.
		expect(pilaDeFuente('MesloLGL Nerd Font Mono', 'terminal')).toBe(
			"'MesloLGL Nerd Font Mono', monospace"
		);
	});

	test('sin fuente elegida devuelve la genérica sola', () => {
		for (const vacio of ['', '   ', null, undefined]) {
			expect(pilaDeFuente(vacio, 'apps')).toBe('sans-serif');
			expect(pilaDeFuente(vacio, 'terminal')).toBe('monospace');
		}
	});

	test('nunca devuelve una cadena vacía', () => {
		// `setProperty` con un valor vacío **borra** la propiedad, y entonces el
		// `var(--font-apps)` de las hojas de estilo se queda sin valor y la regla
		// entera se descarta.
		for (const raro of ['', ';;;', '{}', ' ', '()']) {
			expect(pilaDeFuente(raro, 'apps').length).toBeGreaterThan(0);
		}
	});

	test('el nombre va entre comillas', () => {
		// «3270 Nerd Font» empieza con un dígito: sin comillas invalida la
		// declaración entera y se pierde hasta la genérica.
		expect(pilaDeFuente('3270 Nerd Font', 'terminal')).toBe("'3270 Nerd Font', monospace");
	});

	test('no deja escapar de la cadena CSS', () => {
		// El nombre sale de vasak.conf, que se edita a mano. Una comilla sin
		// tratar cierra la cadena y lo que sigue pasa a ser CSS.
		const pila = pilaDeFuente("Fuente', color: red; x: '", 'apps');
		expect(pila.split("'").length - 1).toBe(2);
		expect(pila).not.toContain(';');
	});

	test('saca los saltos de línea y los caracteres de control', () => {
		const conSalto = ['Noto', 'Sans'].join('\n');
		expect(pilaDeFuente(conSalto, 'apps')).toBe("'NotoSans', sans-serif");
		// Y el espacio normal se conserva, que es lo que distingue una cosa de
		// la otra.
		expect(pilaDeFuente('Noto Sans', 'apps')).toBe("'Noto Sans', sans-serif");
	});

	test('no repite la genérica cuando es lo que se eligió', () => {
		expect(pilaDeFuente('monospace', 'terminal')).toBe('monospace');
		expect(pilaDeFuente('sans-serif', 'apps')).toBe('sans-serif');
	});
});
