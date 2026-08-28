import { describe, expect, test } from 'bun:test';
import { useConfigStore } from './index';

/**
 * Que el tipo publicado del store deje ver sus acciones.
 *
 * `configStore` llevaba una anotación explícita del tipo, y eso publicaba los
 * parámetros del `Store` como `Pick<…, never>`: quien lo usara no veía
 * `loadConfig` y tenía que escribir una aserción para poder llamarlo. Había 28
 * en el escritorio, y una de ellas metía `loadConfig` en el parámetro del
 * estado, con lo cual `vue-tsc` dejaba de comprobar la llamada.
 *
 * Esta comprobación es de tipos, no de ejecución: si la anotación vuelve, el
 * `tsc` del repositorio falla acá. No se llama a nada —hacerlo necesitaría el
 * backend de Tauri—, sólo se nombra.
 */
type Store = ReturnType<typeof useConfigStore>;
type CargarConfig = Store['loadConfig'];

// Si `loadConfig` dejara de estar en el tipo, esto no compila.
const _cargar: CargarConfig = async () => {};

describe('el tipo publicado del store', () => {
	test('deja ver loadConfig sin aserciones', () => {
		// La comprobación real la hace el compilador; acá sólo se deja constancia
		// de que la firma es la que se espera.
		expect(typeof _cargar).toBe('function');
	});
});
