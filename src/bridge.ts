import { invoke, isTauri } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
export const desktop = isTauri();
export async function api<T>(
  action: string,
  payload: unknown = {},
): Promise<T> {
  if (desktop) return invoke<T>("api", { action, payload });
  if (action === "state") {
    const response = await fetch("/preview.json");
    if (!response.ok)
      throw Error("Open the desktop app to create or load a spreadsheet.");
    return response.json() as Promise<T>;
  }
  throw Error(
    "This is a read-only browser preview. Open Cycles desktop to change your spreadsheet.",
  );
}
export async function chooseFile(
  kind: "xlsx" | "csv" | "json",
  destination = false,
): Promise<string | null> {
  if (!desktop) throw Error("File access is available in the desktop app.");
  const options = {
    filters: [{ name: kind.toUpperCase(), extensions: [kind] }],
  };
  if (destination)
    return save({
      ...options,
      defaultPath: `portfolio-${new Date().toLocaleDateString("en-CA")}.${kind}`,
    });
  const result = await open({ ...options, multiple: false, directory: false });
  return typeof result === "string" ? result : null;
}
export function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
export const uid = () => crypto.randomUUID();
