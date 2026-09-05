import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { ref } from "vue";

export async function writeConfig(value: VSKConfig): Promise<void> {
  await invoke("plugin:config-manager|write_config", {
    payload: JSON.stringify(value),
  });
}

export async function setDarkMode(darkmode: boolean): Promise<void> {
  await invoke("plugin:config-manager|set_darkmode", { darkmode });
}

export async function readConfig(): Promise<VSKConfig | null> {
  const jsonString = await invoke<string>("plugin:config-manager|read_config");
  if (jsonString) {
    try {
      return JSON.parse(jsonString) as VSKConfig;
    } catch (error) {
      console.error("Failed to parse config JSON:", error);
      console.debug("Config JSON length:", jsonString.length);
      return null;
    }
  }
  return null;
}

export async function getSchemes(): Promise<Scheme[]> {
  return await invoke<Scheme[]>("plugin:config-manager|get_schemes");
}

export async function getSchemeById(schemeId: string): Promise<Scheme | null> {
  return await invoke<Scheme | null>("plugin:config-manager|get_scheme_by_id", { schemeId });
}

/**
 * Contraste, para que el texto sobre los colores de marca se pueda leer.
 *
 * El esquema trae `on-primary` como un valor fijo, y eso funciona sólo mientras el
 * acento no cambie. El esquema por omisión de VasakOS lo tenía en `#cdd6f4` sobre
 * un primario `#eba0ac`: **1.43:1**, contra el mínimo de 4.5 que pide WCAG 1.4.3.
 * O sea que el texto de cualquier botón de acento era casi invisible, y con
 * cualquier esquema nuevo el problema vuelve, porque nada obliga a quien lo escribe
 * a verificarlo.
 *
 * Así que no se confía: se calcula. Si el valor del esquema cumple, se respeta —es
 * una decisión estética de quien lo hizo—; si no llega, se reemplaza por el color
 * de su propia paleta que mejor contraste dé. Cambiar el acento a lo que sea deja
 * el texto legible sin tocar nada más.
 */
export const MINIMO_TEXTO = 4.5;
/** WCAG 1.4.11: lo que delimita un control necesita 3:1, no 4.5. */
export const MINIMO_NO_TEXTO = 3;

export function luminancia(hex: string): number | null {
  const limpio = hex.trim().replace(/^#/, "");
  const completo =
    limpio.length === 3
      ? limpio
          .split("")
          .map((c) => c + c)
          .join("")
      : limpio;
  if (!/^[0-9a-fA-F]{6}$/.test(completo)) return null;

  const canal = (i: number) => {
    const v = parseInt(completo.slice(i, i + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * canal(0) + 0.7152 * canal(2) + 0.0722 * canal(4);
}

export function contraste(a: string, b: string): number {
  const la = luminancia(a);
  const lb = luminancia(b);
  // Un color que no se puede leer no puede compararse: se informa el peor caso
  // para que nunca se elija por «buen contraste».
  if (la === null || lb === null) return 0;
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/**
 * El mejor color de la lista sobre ese fondo, o `null` si ninguno llega al mínimo.
 *
 * Se recorre en orden y gana el de mayor contraste, no el primero que pase: entre
 * dos que cumplen conviene el más legible, y la diferencia entre 4.6 y 9 se nota.
 */
export function mejorSobre(
  fondo: string,
  candidatos: Array<string | undefined>,
  minimo: number,
): string | null {
  let elegido: string | null = null;
  let mejor = 0;
  for (const c of candidatos) {
    if (!c) continue;
    const r = contraste(c, fondo);
    if (r >= minimo && r > mejor) {
      mejor = r;
      elegido = c;
    }
  }
  return elegido;
}

/**
 * El color de texto para un fondo de marca.
 *
 * Se respeta el del esquema si cumple. Si no, se busca en su propia paleta —el
 * fondo, la superficie, el texto principal— para no salirse de la familia de
 * colores, y sólo como último recurso se cae a negro o blanco.
 */
export function textoSobre(fondo: string, preferido: string | undefined, paleta: UiColors): string {
  if (preferido && contraste(preferido, fondo) >= MINIMO_TEXTO) return preferido;

  // Dos etapas y no una lista sola: `mejorSobre` se queda con el de más contraste,
  // y el negro puro le gana a cualquier color de la paleta casi siempre. Con una
  // sola lista, un botón de acento terminaba con texto negro aunque el esquema
  // tuviera un color propio perfectamente legible — o sea, salirse de la familia
  // de colores sin necesidad. El negro y el blanco quedan como último recurso.
  const deLaPaleta = mejorSobre(
    fondo,
    [paleta.background, paleta.text.main, paleta.surface],
    MINIMO_TEXTO,
  );
  if (deLaPaleta) return deLaPaleta;

  return mejorSobre(fondo, ["#000000", "#ffffff"], MINIMO_TEXTO) ?? "#000000";
}

/**
 * Un borde que se perciba para lo que delimita un control.
 *
 * El borde del esquema es un separador decorativo —el de VasakOS da 1.14 contra el
 * fondo— y con eso el contorno de un campo no se ve. Se busca en la paleta uno que
 * llegue a 3:1 sin ser tan fuerte como el texto.
 */
export function bordeFuerteSobre(paleta: UiColors): string | null {
  return mejorSobre(
    paleta.background,
    [paleta.text.muted, paleta.surface, paleta.text.main],
    MINIMO_NO_TEXTO,
  );
}

/**
 * Para qué se usa una fuente. Determina en qué genérica termina la pila.
 */
export type RolDeFuente = "apps" | "title" | "terminal";

/**
 * La familia genérica con la que termina cada pila.
 *
 * La de la terminal es `monospace` y no `sans-serif`, que es la diferencia que
 * importa: si la fuente elegida no está, una terminal con ancho variable no es
 * una terminal fea, es una que dibuja mal las tablas, las barras de progreso y
 * todo lo que se alinee por columnas.
 */
const GENERICA: Record<RolDeFuente, string> = {
  apps: "sans-serif",
  title: "sans-serif",
  terminal: "monospace",
};

/**
 * Lo que no puede entrar en un nombre de familia.
 *
 * El nombre sale de `vasak.conf`, que es un archivo que se edita a mano, así
 * que puede traer cualquier cosa. Va dentro de una cadena CSS entre comillas:
 * una comilla o una barra invertida sin escapar la cierran antes de tiempo y
 * el resto pasa a ser CSS. Se sacan también los caracteres de control y los
 * saltos de línea, que hacen lo mismo.
 *
 * Se quitan en lugar de escaparse porque ninguno aparece en el nombre real de
 * una fuente: escapar dejaría pasar un nombre absurdo, quitarlos deja el más
 * parecido al que se quiso poner.
 */
// biome-ignore lint/suspicious/noControlCharactersInRegex: son justo los que hay que sacar.
const PROHIBIDOS = /['"\\;{}()<>\u0000-\u001f\u007f]/g;

/**
 * La pila de fuentes para un rol, lista para `font-family`.
 *
 * Siempre termina en una familia genérica. Sin eso, una fuente que se
 * desinstala —o un nombre mal escrito en `vasak.conf`— deja a las
 * aplicaciones sin ninguna fuente declarada, y el motor cae en la suya por
 * omisión, que no tiene por qué parecerse en nada al resto del escritorio.
 *
 * El nombre va entre comillas simples porque casi todos los del sistema tienen
 * espacios —«Noto Sans», «MesloLGL Nerd Font Mono»— y sin comillas hay que
 * confiar en que cada palabra sea un identificador CSS válido: uno que empiece
 * con un dígito, como «3270 Nerd Font», invalida la declaración entera y se
 * pierde también la genérica.
 *
 * Devolver una cadena válida siempre, y nunca vacía, es parte del contrato:
 * `setProperty` con un valor vacío **borra** la propiedad, y entonces el
 * `var(--font-apps)` de las hojas de estilo se queda sin valor y la regla se
 * descarta.
 */
export function pilaDeFuente(
  nombre: string | null | undefined,
  rol: RolDeFuente,
): string {
  const generica = GENERICA[rol];
  const limpio = (nombre ?? "").replace(PROHIBIDOS, "").trim();

  if (limpio === "") {
    return generica;
  }

  // Si ya es el nombre de la genérica, no se repite: `monospace, monospace` es
  // válido pero absurdo, y delata el error a quien lea el CSS.
  if (limpio === generica) {
    return generica;
  }

  return `'${limpio}', ${generica}`;
}

export type VSKConfig = {
  style: {
    darkmode: boolean;
    "color-scheme": string;
    radius: number;
  };
  desktop: {
    wallpaper: string[];
    iconsize: number;
    showfiles: boolean;
    showhiddenfiles: boolean;
  };
  fonts: {
    terminal: string;
    title: string;
    apps: string;
  };
  icons: {
    dark: string;
    light: string;
  };
};

export type Scheme = {
  path: string;
  scheme: SchemeData;
};

export type SchemeData = {
  id: string;
  name: string;
  author: string;
  description: string;
  version: string;
  colors: SchemeColors;
};

export type SchemeColors = {
  dark: ThemeVariant;
  light: ThemeVariant;
};

export type ThemeVariant = {
  ui: UiColors;
  terminal: TerminalColors;
};

export type UiColors = {
  color: ColorPalette;
  text: TextColors;
  background: string;
  border: string;
  surface: string;
};

export type ColorPalette = {
  primary: string;
  secondary: string;
};

export type TextColors = {
  main: string;
  muted: string;
  "on-primary": string;
};

export type TerminalColors = {
  foreground: string;
  background: string;
  cursor: string;
  ansi: AnsiColors;
};

export type AnsiColors = {
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
};

/**
 * El store, definido una sola vez y con su tipo **inferido**.
 *
 * Antes `configStore` llevaba una anotación explícita
 * —`ReturnType<typeof defineStore<"config", () => {...}>>`— y eso es lo que
 * publicaba los parámetros del `Store` como `Pick`: quien lo usara no veía
 * `loadConfig` y tenía que escribir una aserción para poder llamarlo. Había 28
 * en el escritorio, y una de ellas metía `loadConfig` en el parámetro del
 * estado, con lo cual `vue-tsc` dejaba de comprobar la llamada.
 *
 * Dejando que TypeScript infiera el tipo desde la propia definición, el store
 * sale tipado y las aserciones dejan de hacer falta.
 */
const definirConfigStore = () =>
  defineStore("config", () => {
    const config = ref<VSKConfig | null>(null);

    const loadConfig = async () => {
      config.value = await readConfig();
      setMode();
      await setProperties();
    };

    const setMode = () => {
      if (config.value?.style?.darkmode) {
        document.documentElement.classList.add("dark");
      } else {
        document.documentElement.classList.remove("dark");
      }
    };

    const setProperties = async () => {
      if (config.value?.style) {
        const { "color-scheme": colorScheme, radius } = config.value.style;
        const scheme = await getSchemeById(colorScheme);

        if (scheme !== null && scheme !== undefined) {
          const darkScheme = scheme.scheme.colors.dark;
          const lightScheme = scheme.scheme.colors.light;

          // Colores de Marca
          document.documentElement.style.setProperty(
            "--primary",
            lightScheme.ui.color.primary,
          );
          document.documentElement.style.setProperty(
            "--secondary",
            lightScheme.ui.color.secondary,
          );
          document.documentElement.style.setProperty(
            "--primary-dark",
            darkScheme.ui.color.primary,
          );
          document.documentElement.style.setProperty(
            "--secondary-dark",
            darkScheme.ui.color.secondary,
          );

          // Colores de Interfaz (UI)
          document.documentElement.style.setProperty(
            "--ui-background",
            lightScheme.ui.background,
          );
          document.documentElement.style.setProperty(
            "--ui-surface",
            lightScheme.ui.surface,
          );
          document.documentElement.style.setProperty(
            "--ui-border",
            lightScheme.ui.border,
          );
          document.documentElement.style.setProperty(
            "--ui-background-dark",
            darkScheme.ui.background,
          );
          document.documentElement.style.setProperty(
            "--ui-surface-dark",
            darkScheme.ui.surface,
          );
          document.documentElement.style.setProperty(
            "--ui-border-dark",
            darkScheme.ui.border,
          );

          // Colores de Texto
          document.documentElement.style.setProperty(
            "--text-main",
            lightScheme.ui.text.main,
          );
          document.documentElement.style.setProperty(
            "--text-muted",
            lightScheme.ui.text.muted,
          );
          document.documentElement.style.setProperty(
            "--text-on-primary",
            lightScheme.ui.text["on-primary"],
          );
          document.documentElement.style.setProperty(
            "--text-main-dark",
            darkScheme.ui.text.main,
          );
          document.documentElement.style.setProperty(
            "--text-muted-dark",
            darkScheme.ui.text.muted,
          );
          document.documentElement.style.setProperty(
            "--text-on-primary-dark",
            darkScheme.ui.text["on-primary"],
          );

          // El texto sobre los colores de marca, verificado.
          //
          // Lo de arriba escribe lo que dice el esquema; esto lo corrige si no se
          // puede leer. Va después a propósito: así un esquema con un `on-primary`
          // bien elegido lo conserva, y uno que no lo verificó igual queda legible.
          // Ver el módulo de contraste arriba.
          for (const [variante, sufijo] of [
            [lightScheme, ""],
            [darkScheme, "-dark"],
          ] as const) {
            const ui = variante.ui;
            document.documentElement.style.setProperty(
              `--text-on-primary${sufijo}`,
              textoSobre(ui.color.primary, ui.text["on-primary"], ui),
            );
            // El secundario no viene en el esquema y necesita el suyo: en un tema
            // puede ser un violeta claro y en el otro uno saturado, así que un solo
            // color de texto no sirve para los dos.
            document.documentElement.style.setProperty(
              `--text-on-secondary${sufijo}`,
              textoSobre(ui.color.secondary, undefined, ui),
            );

            const borde = bordeFuerteSobre(ui);
            if (borde) {
              document.documentElement.style.setProperty(
                `--ui-border-strong${sufijo}`,
                borde,
              );
            }
          }

          // Status Colors
          document.documentElement.style.setProperty(
            "--status-error",
            lightScheme.terminal.ansi.red,
          );
          document.documentElement.style.setProperty(
            "--status-success",
            lightScheme.terminal.ansi.green,
          );
          document.documentElement.style.setProperty(
            "--status-warning",
            lightScheme.terminal.ansi.yellow,
          );
          document.documentElement.style.setProperty(
            "--status-error-dark",
            darkScheme.terminal.ansi.red,
          );
          document.documentElement.style.setProperty(
            "--status-success-dark",
            darkScheme.terminal.ansi.green,
          );
          document.documentElement.style.setProperty(
            "--status-warning-dark",
            darkScheme.terminal.ansi.yellow,
          );

          // Terminal Colors
          document.documentElement.style.setProperty(
            "--terminal-foreground",
            lightScheme.terminal.foreground,
          );
          document.documentElement.style.setProperty(
            "--terminal-background",
            lightScheme.terminal.background,
          );
          document.documentElement.style.setProperty(
            "--terminal-cursor",
            lightScheme.terminal.cursor,
          );
          document.documentElement.style.setProperty(
            "--terminal-foreground-dark",
            darkScheme.terminal.foreground,
          );
          document.documentElement.style.setProperty(
            "--terminal-background-dark",
            darkScheme.terminal.background,
          );
          document.documentElement.style.setProperty(
            "--terminal-cursor-dark",
            darkScheme.terminal.cursor,
          );

          document.documentElement.style.setProperty(
            "--terminal-ansi-black",
            lightScheme.terminal.ansi.black,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-red",
            lightScheme.terminal.ansi.red,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-green",
            lightScheme.terminal.ansi.green,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-yellow",
            lightScheme.terminal.ansi.yellow,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-blue",
            lightScheme.terminal.ansi.blue,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-magenta",
            lightScheme.terminal.ansi.magenta,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-cyan",
            lightScheme.terminal.ansi.cyan,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-white",
            lightScheme.terminal.ansi.white,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-black",
            lightScheme.terminal.ansi.brightBlack,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-red",
            lightScheme.terminal.ansi.brightRed,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-green",
            lightScheme.terminal.ansi.brightGreen,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-yellow",
            lightScheme.terminal.ansi.brightYellow,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-blue",
            lightScheme.terminal.ansi.brightBlue,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-magenta",
            lightScheme.terminal.ansi.brightMagenta,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-cyan",
            lightScheme.terminal.ansi.brightCyan,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-white",
            lightScheme.terminal.ansi.brightWhite,
          );

          document.documentElement.style.setProperty(
            "--terminal-ansi-black-dark",
            darkScheme.terminal.ansi.black,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-red-dark",
            darkScheme.terminal.ansi.red,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-green-dark",
            darkScheme.terminal.ansi.green,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-yellow-dark",
            darkScheme.terminal.ansi.yellow,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-blue-dark",
            darkScheme.terminal.ansi.blue,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-magenta-dark",
            darkScheme.terminal.ansi.magenta,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-cyan-dark",
            darkScheme.terminal.ansi.cyan,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-white-dark",
            darkScheme.terminal.ansi.white,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-black-dark",
            darkScheme.terminal.ansi.brightBlack,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-red-dark",
            darkScheme.terminal.ansi.brightRed,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-green-dark",
            darkScheme.terminal.ansi.brightGreen,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-yellow-dark",
            darkScheme.terminal.ansi.brightYellow,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-blue-dark",
            darkScheme.terminal.ansi.brightBlue,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-magenta-dark",
            darkScheme.terminal.ansi.brightMagenta,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-cyan-dark",
            darkScheme.terminal.ansi.brightCyan,
          );
          document.documentElement.style.setProperty(
            "--terminal-ansi-bright-white-dark",
            darkScheme.terminal.ansi.brightWhite,
          );
        }

        document.documentElement.style.setProperty(
          "--corner-radius",
          `${radius}px`,
        );
      }

      // Las tres fuentes se aplican acá, al lado de los colores, porque es lo
      // que hace que toda aplicación siga la configuración sin que cada una
      // tenga que acordarse. Antes se leían y se tiraban: Configuración
      // escribía la elección en `vasak.conf` y nadie la miraba, así que elegir
      // una fuente no cambiaba nada en ninguna parte.
      const fuentes = config.value?.fonts;
      const root = document.documentElement.style;

      // Con prefijo `--vsk-` para no chocar con los tokens de Tailwind. Las
      // hojas de estilo definen `--font-sans`, `--font-mono` y `--font-title`
      // **en función** de estas tres, y una variable que se define a sí misma
      // no resuelve.
      const deApps = pilaDeFuente(fuentes?.apps, "apps");

      root.setProperty("--vsk-font-apps", deApps);
      // El título cae en la de las aplicaciones antes que en la genérica: son
      // la misma familia por omisión, y quien eligió una sola quiere esa.
      root.setProperty(
        "--vsk-font-title",
        pilaDeFuente(fuentes?.title || fuentes?.apps, "title"),
      );
      root.setProperty(
        "--vsk-font-terminal",
        pilaDeFuente(fuentes?.terminal, "terminal"),
      );

      // Y además en el elemento, que es el piso.
      //
      // Con esto una aplicación sigue la fuente configurada **aunque su hoja de
      // estilo no declare los tokens**: se hereda a todo el documento. Importa
      // porque el plugin y las aplicaciones se actualizan por separado, y sin
      // este piso una que quedara atrás volvería a la fuente del motor sin que
      // nada lo dijera. Las utilidades de Tailwind lo pisan donde se usen, que
      // es lo que se busca para el título y la terminal.
      root.fontFamily = deApps;
    };

    return {
      config,
      loadConfig,
    };
  });

let configStore: ReturnType<typeof definirConfigStore> | null = null;

export const useConfigStore = () => {
  configStore ??= definirConfigStore();
  return configStore();
};
