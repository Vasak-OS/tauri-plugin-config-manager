import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { ref } from "vue";

export async function writeConfig(value: VSKConfig): Promise<void> {
  await invoke("plugin:config-manager|write_config", {
    payload: JSON.stringify(value), // Serializar VSKConfig y enviarlo como 'payload'
  });
}

export async function readConfig(): Promise<VSKConfig | null> {
  const jsonString = await invoke<string>( // Esperar que invoke resuelva directamente con la cadena JSON
    "plugin:config-manager|read_config"
  );
  if (jsonString) {
    try {
      console.log("Config JSON:", jsonString); // Log para depuración
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
    color: string;
    radius: number;
  };
  info: {
    logo: string;
  };
};

export const useConfigStore = defineStore("config", () => {
  const config = ref<VSKConfig | null>(null);

  const loadConfig = async () => {
    config.value = await readConfig();
    if (config.value === null) {
      console.error("Failed to load configuration");
      return;
    }
    setMode();
    setProperties();
  };

  const setMode = () => {
    if (config.value?.style.darkmode) {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }
  };

  const setProperties = () => {
    if (config.value) {
      document.documentElement.style.setProperty(
        "--primary-color",
        config.value.style.color || "#4a90e2"
      );
      document.documentElement.style.setProperty(
        "--border-radius",
        `${config.value.style.radius}px` || "8px"
      );
    }
  };

  return {
    config,
    loadConfig,
    setMode,
    setProperties,
  };
});
