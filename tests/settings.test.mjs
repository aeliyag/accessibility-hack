import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../src/lib/settings.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
});
const { mergeSettings, cycleSnapMode } = await import(
  `data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`
);

test("existing paragraph preferences migrate without losing other settings", () => {
  const settings = mergeSettings({ snapMode: "paragraph", autoSnap: true, maskOpacity: 0.4 });
  assert.equal(settings.snapMode, "word");
  assert.equal(settings.autoSnap, true);
  assert.equal(settings.maskOpacity, 0.4);
});

test("M cycles only the three enabled modes after preference migration", () => {
  let mode = mergeSettings({ snapMode: "paragraph" }).snapMode;
  const modes = [];
  for (let i = 0; i < 6; i++) { modes.push(mode); mode = cycleSnapMode(mode); }
  assert.deepEqual(modes, ["word", "sentence", "line", "word", "sentence", "line"]);
});
