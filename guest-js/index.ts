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
  ligth: ThemeVariant;
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
  seccondary: string;
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

        // TODO: Remplazar esto por todo el sistema de schemes, lo actual no funciona porque el parametro colorScheme tiene el id del scheme
        if (colorScheme && colorScheme.trim() !== "") {
          document.documentElement.setAttribute(
            "data-color-scheme",
            colorScheme,
          );
        }

        document.documentElement.style.setProperty(
          "--border-radius",
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
