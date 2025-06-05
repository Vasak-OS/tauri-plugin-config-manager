import { invoke } from "@tauri-apps/api/core";

export async function writeConfig(value: VSKConfig): Promise<void> {
  await invoke<{ value?: string }>("plugin:config-manager|write_config", {
    payload: {
      value,
    },
  });
}

export async function readConfig(): Promise<string | null> {
  return await invoke<{ value?: string }>(
    "plugin:config-manager|read_config"
  ).then((r) => (r.value ? r.value : null));
}

export type VSKConfig = {
  style: {
    darkMode: boolean;
    primaryColor: string;
    borderRadius: number;
  };
};
