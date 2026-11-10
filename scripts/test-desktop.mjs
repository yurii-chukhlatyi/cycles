import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const state = mkdtempSync(resolve(tmpdir(), "cycles-native-"));
const env = { ...process.env, CYCLES_TEST_DATA_DIR: state };
mkdirSync(resolve(root, "test-results/native"), { recursive: true });
function run(args) {
  const result = spawnSync("pnpm", args, {
    cwd: root,
    env,
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run([
  "exec",
  "tauri",
  "build",
  "--debug",
  "--features",
  "e2e",
  "--config",
  "src-tauri/tauri.e2e.conf.json",
  "--no-bundle",
]);
run(["exec", "wdio", "run", "wdio.conf.mjs"]);
