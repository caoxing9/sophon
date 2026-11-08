// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
import { expect, open, test } from "nsh/test";

test("shows the sidebar and its entry points", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  expect(app.texts()).toContain("Sophon");
  expect(app.texts()).toContain("Add project");
  expect(app.texts()).toContain("New workspace");
});

test("opens the new workspace screen", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("New workspace >> nth=0");
  expect(app.texts()).toContain("Import session");
});

test("attaches a pasted screenshot", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("New workspace >> nth=0");
  await app.paste("What should the agent do?", { files: ["assets/loader/moon@2x.png"] });
  expect(app.texts()).toContain("moon@2x.png");
});
