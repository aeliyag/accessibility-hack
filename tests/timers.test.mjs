import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

const compile = (text) => ts.transpileModule(text, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const uri = (text) => `data:text/javascript;base64,${Buffer.from(text).toString("base64")}`;

test("main screen-break timer regression checks still pass", async () => {
  const model = uri(compile(await readFile(new URL("../src/lib/screenBreak.ts", import.meta.url), "utf8")));
  const checks = await readFile(new URL("../src/lib/screenBreak.check.ts", import.meta.url), "utf8");
  await import(uri(compile(checks).replace('"./screenBreak"', JSON.stringify(model))));
});
