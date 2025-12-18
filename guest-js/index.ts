import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { ref } from "vue";

export async function writeConfig(value: VSKConfig): Promise<void> {
  await invoke("plugin:config-manager|write_config", {
    payload: JSON.stringify(value), // Serializar VSKConfig y enviarlo como 'payload'
  });
}

export async function setDarkMode(darkmode: boolean): Promise<void> {
  await invoke("plugin:config-manager|set_darkmode", { darkmode });
}

export async function readConfig(): Promise<VSKConfig | null> {
  const jsonString = await invoke<string>( // Esperar que invoke resuelva directamente con la cadena JSON
    "plugin:config-manager|read_config"
  );
  if (jsonString) {
    try {
      return JSON.parse(jsonString) as VSKConfig; // Parsear la cadena JSON
    } catch (error) {
      console.error("Failed to parse config JSON:", error, "Raw string:", jsonString);
      return null;
    }
  }
  return null;
}

export type VSKConfig = {
  style: {
    darkmode: boolean;
    primarycolor: string;
    radius: number;
  };
  info: {
    logo: string;
  };
};

let configStore: ReturnType<typeof defineStore<"config", () => {
  config: any;
  loadConfig: () => Promise<void>;
}>> | null = null;

export const useConfigStore = () => {
  if (!configStore) {
    configStore = defineStore("config", () => {
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
          const { primarycolor, radius } = config.value.style;

          // Manejar primarycolor
          if (primarycolor && primarycolor.trim() !== "") {
            document.documentElement.style.setProperty(
              "--primary-color",
              primarycolor
            );
          }

          document.documentElement.style.setProperty(
            "--border-radius",
            `${radius}px`
          );
        }
      };

      return {
        config,
        loadConfig
      };
    });
  }
  
  return configStore();
};
