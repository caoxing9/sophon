// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
import { expect, open, test } from "nsh/test";

// The new-workspace screen, once it is drawn: a page may come a frame
// after the press that opens it, and a slow machine (CI) shows that.
async function newWorkspace(app: Awaited<ReturnType<typeof open>>) {
  await app.press("New workspace >> nth=0");
  await app.waitFor("text:Import session");
}

test("shows the sidebar and its entry points", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  expect(app.texts()).toContain("Add project");
  expect(app.texts()).toContain("New workspace");
});

test("opens the new workspace screen", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await newWorkspace(app);
  expect(app.texts()).toContain("Import session");
});

test("attaches a pasted screenshot as a chip in the text", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await newWorkspace(app);
  // The placeholder, after the empty line's zero-width filler.
  expect(app.texts().some((t) => t.endsWith("What should the agent do?"))).toBe(true);
  // The composer is a rich editor: its native input sits inside it.
  await app.paste("id:composer", { files: ["assets/loader/moon@2x.png"] });
  // The chip, named as the file, inline in the composer's text: it may
  // come a frame after the paste, and a slow machine (CI) shows that.
  await app.waitFor("label:moon@2x.png");
  expect(app.texts()).toContain("moon@2x.png");
  expect(app.count("label:moon@2x.png")).toBe(1);
});

test("Backspace deletes a chip as one unit", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await newWorkspace(app);
  await app.paste("id:composer", { files: ["assets/loader/moon@2x.png"] });
  await app.waitFor("label:moon@2x.png");
  expect(app.count("label:moon@2x.png")).toBe(1);
  await app.keyboard.press("backspace");
  expect(app.count("label:moon@2x.png")).toBe(0);
});

test("settings lists the agents with their add buttons", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("label:Settings >> nth=0");
  await app.waitFor("text:Coding agents");
  expect(app.texts()).toContain("Coding agents");
  expect(app.texts()).toContain("Add account…");
  expect(app.texts()).toContain("Add API key…");
});

test("opens the usage page from the sidebar", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await app.press("label:Usage");
  await app.waitFor("text:This computer's sessions");
  expect(app.texts()).toContain("This computer's sessions");
  expect(app.texts()).toContain("Plan limits");
  expect(app.texts()).toContain("7 days");
});

test("the model picker shows the agents, the models and their reasoning", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await newWorkspace(app);
  await app.press("label:Model");
  await app.waitFor("text:Reasoning");
  expect(app.texts()).toContain("Reasoning");
  expect(app.count("label:Favorites")).toBe(1);
  await app.keyboard.press("escape");
  expect(app.texts()).not.toContain("Reasoning");
});

test("a Mac that runs no agents of its own connects to others", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  const elsewhere = "text:Connect to the computer your agents run on";
  await app.press("label:Settings >> nth=0");
  await app.waitFor("label:Run agents on this Mac");
  // From its default, whatever an earlier run left.
  if (app.shows(elsewhere)) {
    await app.press("label:Run agents on this Mac");
    await app.waitFor(elsewhere, { state: "gone" });
  }
  await app.press("label:Run agents on this Mac");
  await app.waitFor(elsewhere);
  await app.press("label:Run agents on this Mac");
  await app.waitFor(elsewhere, { state: "gone" });
});

test("settings manage the spaces on this Mac", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("label:Settings >> nth=0");
  // The page may come a frame after the press.
  await app.waitFor("text:Spaces");
  expect(app.texts()).toContain("Spaces");
  expect(app.texts()).toContain("My space");
  expect(app.texts()).toContain("New space…");
});

test("a new schedule is set without writing cron", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("Schedules >> nth=0");
  await app.waitFor("text:New schedule");
  await app.press("New schedule >> nth=0");
  await app.waitFor("text:Repeats");
  // Every day at 9:00 to begin with, said back in words.
  expect(app.texts()).toContain("Runs: Every day at 9:00");
  expect(app.texts()).toContain("Every week");
  expect(app.texts()).not.toContain("Cron expression, e.g. 0 9 * * 1-5");
});
