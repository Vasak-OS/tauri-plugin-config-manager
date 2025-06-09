import { invoke } from "@tauri-apps/api/core";

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
