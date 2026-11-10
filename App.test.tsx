// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
import { expect, open, test } from "nsh/test";

test("shows the sidebar and its entry points", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  expect(app.texts()).toContain("Add project");
  expect(app.texts()).toContain("New workspace");
});

test("opens the new workspace screen", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("New workspace >> nth=0");
  expect(app.texts()).toContain("Import session");
});

test("attaches a pasted screenshot as a chip in the text", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("New workspace >> nth=0");
  // The placeholder, after the empty line's zero-width filler.
  expect(app.texts().some((t) => t.endsWith("What should the agent do?"))).toBe(true);
  // The composer is a rich editor: its native input sits inside it.
  await app.paste("id:composer", { files: ["assets/loader/moon@2x.png"] });
  // The chip, named as the file, inline in the composer's text.
  expect(app.texts()).toContain("moon@2x.png");
  expect(app.count("label:moon@2x.png")).toBe(1);
});

test("Backspace deletes a chip as one unit", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("New workspace >> nth=0");
  await app.paste("id:composer", { files: ["assets/loader/moon@2x.png"] });
  expect(app.count("label:moon@2x.png")).toBe(1);
  await app.keyboard.press("backspace");
  expect(app.count("label:moon@2x.png")).toBe(0);
});

test("settings lists the agents with their add buttons", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("label:Settings >> nth=0");
  expect(app.texts()).toContain("Coding agents");
  expect(app.texts()).toContain("Add account…");
  expect(app.texts()).toContain("Add API key…");
});

test("opens the usage page from the sidebar", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("label:Usage");
  expect(app.texts()).toContain("This computer's sessions");
  expect(app.texts()).toContain("Plan limits");
  expect(app.texts()).toContain("7 days");
});

test("the model picker shows the agents, the models and their reasoning", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("New workspace >> nth=0");
  await app.press("label:Model");
  expect(app.texts()).toContain("Reasoning");
  expect(app.count("label:Favorites")).toBe(1);
  await app.keyboard.press("escape");
  expect(app.texts()).not.toContain("Reasoning");
});

test("settings' TSX section reads the preferences from Rust and changes them there", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("label:Settings >> nth=0");
  // The section is TSX (`tsx/settings/General.tsx`), placed by the Rust page.
  expect(app.texts()).toContain("While an agent is running");
  const before = app.spanStyle("text:Queue").color;
  await app.press("text:Queue");
  // The TSX screen sets nothing itself: Queue lights up only once Rust has
  // the new value and tells it.
  expect(app.spanStyle("text:Queue").color).not.toBe(before);
  expect(app.spanStyle("text:Queue").color).toBe(app.spanStyle("text:While an agent is running").color);
  await app.press("text:Steer");
  expect(app.spanStyle("text:Queue").color).toBe(before);
});
