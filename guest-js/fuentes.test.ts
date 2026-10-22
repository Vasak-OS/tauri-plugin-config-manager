import { describe, expect, test } from 'bun:test';
import {
	ESCALA_MAXIMA,
	ESCALA_MINIMA,
	escalaDeFuente,
	pilaDeFuente,
	primeraUtil,
	propiedadesDeEscala,
} from './index';

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

/**
 * La cascada del título: si no hay una propia, se usa la de las aplicaciones.
 * Lo que importa es que decida mirando el nombre **ya saneado**.
 */
describe('la cascada entre candidatos', () => {
	test('se queda con el primero que sirva', () => {
		expect(primeraUtil('Cantarell', 'Noto Sans')).toBe('Cantarell');
	});

	test('un título de sólo espacios no le gana a una fuente buena', () => {
		// Es el caso que fallaba: `'   ' || 'Noto Sans'` devuelve los espacios,
		// porque una cadena de espacios es verdadera. Se saneaba a nada después
		// de haber ganado la comparación, y el título terminaba en la genérica
		// con una fuente de aplicaciones perfectamente buena a mano.
		expect(primeraUtil('   ', 'Noto Sans')).toBe('Noto Sans');
	});

	test('ni un título que sea todo caracteres prohibidos', () => {
		expect(primeraUtil('{};', 'Noto Sans')).toBe('Noto Sans');
		expect(primeraUtil('()', 'Cantarell')).toBe('Cantarell');
	});

	test('salta los vacíos, los nulos y los indefinidos', () => {
		expect(primeraUtil(null, undefined, '', 'Noto Sans')).toBe('Noto Sans');
	});

	test('sin ningún candidato útil devuelve vacío, y la pila cae en la genérica', () => {
		expect(primeraUtil(null, '  ', '')).toBe('');
		expect(pilaDeFuente(primeraUtil(null, '  ', ''), 'title')).toBe('sans-serif');
	});

	test('devuelve el nombre saneado, no el crudo', () => {
		expect(primeraUtil('  Noto Sans  ')).toBe('Noto Sans');
	});
});

describe('la escala del texto', () => {
	test('sin valor, o con algo que no es un número, vale uno', () => {
		for (const raro of [undefined, null, '1.5', 'grande', NaN, Infinity, -Infinity, {}, []]) {
			expect(escalaDeFuente(raro)).toBe(1);
		}
	});

	test('se acota al mínimo y al máximo, los mismos que el lado de Rust', () => {
		expect(escalaDeFuente(0.1)).toBe(ESCALA_MINIMA);
		expect(escalaDeFuente(-2)).toBe(ESCALA_MINIMA);
		expect(escalaDeFuente(9)).toBe(ESCALA_MAXIMA);
		expect(escalaDeFuente(1.25)).toBe(1.25);
		expect([ESCALA_MINIMA, ESCALA_MAXIMA]).toEqual([0.75, 2]);
	});

	test('a uno no se escribe un tamaño base, para no pisar el de la aplicación', () => {
		expect(propiedadesDeEscala(1)).toEqual({ variable: '1', tamanoBase: null });
		expect(propiedadesDeEscala(undefined)).toEqual({ variable: '1', tamanoBase: null });
	});

	test('el tamaño base es el porcentaje de la escala', () => {
		expect(propiedadesDeEscala(1.25)).toEqual({ variable: '1.25', tamanoBase: '125%' });
		expect(propiedadesDeEscala(0.8)).toEqual({ variable: '0.8', tamanoBase: '80%' });
		expect(propiedadesDeEscala(9)).toEqual({ variable: '2', tamanoBase: '200%' });
	});
});
