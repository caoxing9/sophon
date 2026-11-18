// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
import { expect, open, test } from "nsh/test";

// The new-workspace screen, once it is drawn: a page may come a frame
// after the press that opens it, and a slow machine (CI) shows that.
async function newWorkspace(app: Awaited<ReturnType<typeof open>>) {
  await app.press("New workspace >> nth=0");
  await app.waitFor("text:Import sessions");
}

test("shows the sidebar and its entry points", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  // Projects' + says what it does to the screen reader alone.
  expect(app.count("label:Add project")).toBe(1);
  expect(app.texts()).toContain("New workspace");
  expect(app.texts()).toContain("More");
});

test("opens the new workspace screen", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await newWorkspace(app);
  expect(app.texts()).toContain("Import sessions");
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

test("attaches a file dropped on the composer", async () => {
  const app = await open(".", { width: 1100, height: 760, platform: "macos" });
  await newWorkspace(app);
  // As Finder drags it: the file's address, read where it is let go.
  const here = import.meta.url.replace(/[^/]*$/, "").replace(/^[a-z]+:(\/\/)?/, "file://");
  await app.dropFiles("id:composer", [`${here}assets/loader/moon@2x.png`]);
  await app.waitFor("label:moon@2x.png");
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

test("a new line in the composer is a line of its own to the caret's keys", async () => {
  const app = await open(".", { width: 1100, height: 760, platform: "macos" });
  await newWorkspace(app);
  await app.type("id:composer", "one");
  await app.keyboard.press("shift+enter");
  await app.type("id:composer", "two");
  // Control+A: the start of this line, not of the whole message.
  await app.keyboard.press("ctrl+a");
  await app.type("id:composer", ">");
  expect(app.texts()).toContain("one");
  expect(app.texts()).toContain(">two");
  // Control+K: the rest of the line; once more, the break after it.
  await app.keyboard.press("ctrl+k");
  expect(app.texts()).toContain(">");
  expect(app.texts()).not.toContain(">two");
  await app.keyboard.press("cmd+up");
  await app.keyboard.press("cmd+right");
  await app.keyboard.press("ctrl+k");
  expect(app.texts()).toContain("one>");
});

test("typing on an empty line above another stays on that line", async () => {
  const app = await open(".", { width: 1100, height: 760, platform: "macos" });
  await newWorkspace(app);
  // An empty first line, a second with text, and the caret back up: it
  // showed on the first line while what was typed went to the second.
  await app.type("id:composer", "q");
  await app.keyboard.press("backspace");
  await app.keyboard.press("shift+enter");
  await app.type("id:composer", "second");
  await app.keyboard.press("up");
  await app.type("id:composer", "first");
  expect(app.texts()).toContain("first");
  expect(app.texts()).toContain("second");
  // And on an empty line between two.
  await app.keyboard.press("cmd+down");
  await app.keyboard.press("shift+enter");
  await app.keyboard.press("shift+enter");
  await app.type("id:composer", "fourth");
  await app.keyboard.press("up");
  await app.type("id:composer", "third");
  for (const line of ["first", "second", "third", "fourth"]) expect(app.texts()).toContain(line);
});

test("Option+Enter and Control+J break the line as Shift+Enter does", async () => {
  const app = await open(".", { width: 1100, height: 760, platform: "macos" });
  await newWorkspace(app);
  await app.type("id:composer", "first");
  await app.keyboard.press("alt+enter");
  await app.type("id:composer", "second");
  await app.keyboard.press("ctrl+j");
  await app.type("id:composer", "third");
  for (const line of ["first", "second", "third"]) expect(app.texts()).toContain(line);
});

test("a press on the composer's card is a press on its text", async () => {
  const app = await open(".", { width: 1100, height: 760 });
  await newWorkspace(app);
  // The card's middle: the bar between the controls and Send.
  await app.press("id:composer-card");
  await app.keyboard.press("h");
  await app.keyboard.press("i");
  expect(app.texts()).toContain("hi");
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
  // Behind More, with Pull requests and History.
  await app.press("More >> nth=0");
  await app.waitFor("text:Usage");
  await app.press("Usage >> nth=0");
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
