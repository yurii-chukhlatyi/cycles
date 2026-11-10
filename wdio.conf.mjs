import { resolve } from "node:path";
export const config = {
  runner: "local",
  specs: ["./tests/desktop/*.e2e.mjs"],
  maxInstances: 1,
  logLevel: "warn",
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { timeout: 120000 },
  connectionRetryTimeout: 60000,
  connectionRetryCount: 0,
  services: [
    [
      "tauri",
      {
        appBinaryPath: resolve(
          `target/debug/portfolio-cycle-app${process.platform === "win32" ? ".exe" : ""}`,
        ),
        driverProvider: "embedded",
        captureBackendLogs: true,
      },
    ],
  ],
  capabilities: [{ browserName: "tauri", "tauri:options": {} }],
};
