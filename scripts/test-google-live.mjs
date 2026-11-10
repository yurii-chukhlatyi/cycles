import { spawnSync } from "node:child_process";
if (process.env.CYCLES_RUN_GOOGLE_LIVE !== "1") {
  console.error(
    "Not run: connect your Google Desktop client in Cycles, then explicitly set CYCLES_RUN_GOOGLE_LIVE=1. This suite creates a disposable sample spreadsheet in that account.",
  );
  process.exit(2);
}
const result = spawnSync(
  "cargo",
  [
    "test",
    "-p",
    "portfolio-cycle-app",
    "google_live",
    "--",
    "--ignored",
    "--nocapture",
  ],
  { stdio: "inherit" },
);
if (result.error) throw result.error;
process.exit(result.status ?? 1);
