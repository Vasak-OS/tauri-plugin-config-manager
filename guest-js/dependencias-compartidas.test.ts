/**
 * Que todo lo que este complemento importa sea una dependencia **par**.
 *
 * Un complemento no usa su propia copia de `vue` ni de `pinia`: usa la de la
 * aplicación que lo monta. Declararlas como dependencias normales —que es como
 * estuvieron hasta la 3.0.0— funciona sólo mientras los rangos se crucen, y deja
 * de funcionar en silencio el día que no.
 *
 * Así se rompió. Al probar `pinia` 4 en vasak-file-manager, el instalador dejó
 * la 4 en la raíz y anidó una 3 acá abajo, porque este `package.json` la pedía
 * en `^3.0.4`. Dos copias de `pinia` son dos tiendas activas y dos símbolos de
 * inyección distintos, así que `app.use(createPinia())` registraba la de la
 * aplicación y la tienda de este complemento no la encontraba nunca:
 *
 *     [🍍]: "getActivePinia()" was called but there was no active Pinia.
 *
 * Con `vue` habría sido peor: dos copias son dos sistemas de reactividad, y ahí
 * lo que falla no da un mensaje tan claro.
 *
 * La prueba mira los **imports del fuente** y no una lista escrita a mano: lo
 * que hay que compartir es lo que se importa, así que una importación nueva
 * aparece sola. `rollup.config.js` arma los externos leyendo `peerDependencies`,
 * o sea que declararlo par también es lo que lo deja afuera del paquete.
 *
 * Lo que esta prueba **no** puede ver es el fallo de verdad, que sólo existe con
 * una aplicación alrededor. Ese lo vigila `una-sola-copia.test.ts` en
 * vasak-file-manager, montando esta tienda con la `pinia` de la aplicación.
 */

import { describe, expect, test } from 'bun:test';
import { fileURLToPath } from 'node:url';

const raiz = fileURLToPath(new URL('..', import.meta.url));

interface Manifiesto {
	dependencies?: Record<string, string>;
	devDependencies?: Record<string, string>;
	peerDependencies?: Record<string, string>;
}

/**
 * Los paquetes que importa un fuente.
 *
 * Se queda con el nombre del paquete: de `@tauri-apps/api/core` sale
 * `@tauri-apps/api`, que es lo que se declara. Deja afuera lo relativo y los
 * módulos de node, que no se declaran en ningún lado.
 */
function paquetesImportados(fuente: string): string[] {
	const encontrados = new Set<string>();

	for (const [, especificador] of fuente.matchAll(
		/(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g
	)) {
		if (especificador.startsWith('.') || especificador.startsWith('node:')) continue;

		const partes = especificador.split('/');
		encontrados.add(especificador.startsWith('@') ? partes.slice(0, 2).join('/') : partes[0]);
	}

	return [...encontrados].sort();
}

const manifiesto = (await Bun.file(`${raiz}package.json`).json()) as Manifiesto;
const fuente = await Bun.file(`${raiz}guest-js/index.ts`).text();
const importados = paquetesImportados(fuente);

describe('lo que el complemento importa', () => {
	test('y la lectura del fuente encuentra imports de verdad', () => {
		// Sin esto, un fuente movido de lugar o un patrón que dejó de matchear
		// dejan las tres pruebas de abajo recorriendo una lista vacía.
		//
		// Los tres nombrados y no la lista entera: un import nuevo **declarado
		// como corresponde** no tiene por qué hacer fallar esto, y las pruebas
		// que siguen ya lo miran. Lo que esta vigila es que el lector siga
		// viendo lo que hay. Lo marcó la revisión.
		expect(importados).toEqual(expect.arrayContaining(['@tauri-apps/api', 'pinia', 'vue']));
	});

	test('lo pide como par y no se lo trae', () => {
		const pares = manifiesto.peerDependencies ?? {};
		const propias = manifiesto.dependencies ?? {};

		expect(importados.filter((paquete) => !(paquete in pares))).toEqual([]);
		expect(importados.filter((paquete) => paquete in propias)).toEqual([]);
	});

	test('y lo tiene igual para compilar y para probar', () => {
		// Una par no se instala sola: sin la misma en `devDependencies`, acá no
		// hay con qué compilar ni con qué correr estas pruebas.
		const desarrollo = manifiesto.devDependencies ?? {};

		expect(importados.filter((paquete) => !(paquete in desarrollo))).toEqual([]);
	});

	test('y `pinia` se pide en la 3 y en la 4, para que alcance a toda aplicación', () => {
		// La 3 y la 4 no comparten nada: con la aplicación en una y el rango del
		// complemento en la otra, el instalador anida una segunda copia, que es
		// todo el problema. Mientras el taller tenga aplicaciones en las dos, el
		// rango tiene que admitir las dos. Con sólo `^4.0.0` —la 2.7 y la 2.8—
		// las que están en la 3 quedaron fijas en `~2.6.1`.
		const rango = manifiesto.peerDependencies?.pinia ?? '';

		expect(Bun.semver.satisfies('3.0.4', rango)).toBe(true);
		expect(Bun.semver.satisfies('4.0.3', rango)).toBe(true);
		// Y no de más: la 2 es otra API, y una 5 no está probada.
		expect(Bun.semver.satisfies('2.3.1', rango)).toBe(false);
		expect(Bun.semver.satisfies('5.0.0', rango)).toBe(false);
	});

	test('y se comprueba: el lector distingue lo que se declara de lo que no', () => {
		// El control positivo. Sin esto, un `paquetesImportados` que devuelva
		// siempre `[]` deja las tres de arriba en verde para siempre.
		const inventado = [
			"import { invoke } from '@tauri-apps/api/core';",
			"import { defineStore } from 'pinia';",
			"import { ref } from 'vue';",
			"import { algo } from './vecino';",
			"import { readFileSync } from 'node:fs';",
			"const tarde = await import('@ambito/cargado-tarde/sub/ruta');",
		].join('\n');

		expect(paquetesImportados(inventado)).toEqual([
			'@ambito/cargado-tarde',
			'@tauri-apps/api',
			'pinia',
			'vue',
		]);
	});
});
