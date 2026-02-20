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

let configStore: ReturnType<
  typeof defineStore<
    "config",
    () => {
      config: any;
      loadConfig: () => Promise<void>;
    }
  >
> | null = null;

export const useConfigStore = () => {
  configStore ??= defineStore("config", () => {
    const config = ref<VSKConfig | null>(null);

    const loadConfig = async () => {
      config.value = await readConfig();
      setMode();
      setProperties();
    };

    const setMode = () => {
      if (config.value?.style?.darkmode) {
        document.documentElement.classList.add("dark");
      } else {
        document.documentElement.classList.remove("dark");
      }
    };

    const setProperties = () => {
      if (config.value?.style) {
        const { "color-scheme": colorScheme, radius } = config.value.style;
        const scheme: Scheme | unknown = getSchemeById(colorScheme) as Scheme | unknown;

        if (scheme !== null) {
          const darkScheme = (scheme as Scheme).scheme.colors.dark;
          const lightScheme = (scheme as Scheme).scheme.colors.light;

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
    };

    return {
      config,
      loadConfig,
    };
  });

  return configStore();
};
