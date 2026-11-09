const { spawnSync } = require("node:child_process");
const { join, resolve } = require("node:path");
const { readFileSync, existsSync } = require("node:fs");

// Only updater signing values are loaded here; SSH credentials never enter the child process environment.
const envPath = resolve(__dirname, "..", ".env");
if (existsSync(envPath)) {
  for (const line of readFileSync(envPath, "utf8").split(/\r?\n/)) {
    const match = line.match(/^\s*(TAURI_SIGNING_PRIVATE_KEY(?:_PASSWORD)?)\s*=\s*(.*)\s*$/);
    if (!match || process.env[match[1]]) continue;
    process.env[match[1]] = match[2].replace(/^(["'])(.*)\1$/, "$2");
  }
}

if (process.platform === "win32" && !process.env.CARGO_TARGET_DIR && process.env.LOCALAPPDATA) {
  process.env.CARGO_TARGET_DIR = join(process.env.LOCALAPPDATA, "pr2po", "cargo-target");
}

const cliPath = require.resolve("@tauri-apps/cli/tauri.js");
const result = spawnSync(process.execPath, [cliPath, ...process.argv.slice(2)], {
  env: process.env,
  stdio: "inherit",
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
