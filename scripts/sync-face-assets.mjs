import { cpSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(root, "node_modules/@mediapipe/tasks-vision/wasm");
const target = join(root, "public/mediapipe");

if (!existsSync(source)) {
  console.error("MediaPipe wasm was not found. Run npm install first.");
  process.exit(1);
}

mkdirSync(target, { recursive: true });

for (const name of [
  "vision_wasm_internal.js",
  "vision_wasm_internal.wasm",
  "vision_wasm_module_internal.js",
  "vision_wasm_module_internal.wasm",
  "vision_wasm_nosimd_internal.js",
  "vision_wasm_nosimd_internal.wasm",
]) {
  cpSync(join(source, name), join(target, name));
}
