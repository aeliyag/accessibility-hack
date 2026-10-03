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


test("main box and control preferences survive with OCR settings", () => {
  const settings = mergeSettings({ centerX: 310, centerY: 420, boxWidth: 650,
    boxHeight: 85, scrollSpeed: 24, controlsPanelX: 90, controlsPanelY: 120,
    autoSnap: true, snapMode: "sentence" });
  assert.equal(settings.centerX, 310);
  assert.equal(settings.centerY, 420);
  assert.equal(settings.boxWidth, 650);
  assert.equal(settings.boxHeight, 85);
  assert.equal(settings.scrollSpeed, 24);
  assert.equal(settings.controlsPanelX, 90);
  assert.equal(settings.controlsPanelY, 120);
  assert.equal(settings.autoSnap, true);
  assert.equal(settings.snapMode, "sentence");
});

test("old slit settings migrate to a box while retaining OCR preferences", () => {
  const settings = mergeSettings({ slitHeight: 72, yOffset: 30, autoSnap: true, snapMode: "word" });
  assert.equal(settings.boxHeight, 72);
  assert.equal(settings.centerY, 480);
  assert.equal(settings.autoSnap, true);
  assert.equal(settings.snapMode, "word");
});
