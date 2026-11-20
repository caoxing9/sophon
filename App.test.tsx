// The Rust root tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
import { expect, open, test } from "nsh/test";

// A `.sophon` file made by hand (a gzipped tar): the plugin `hello`, one
// setting of it, and an automation set up with it.
const HELLO_SOPHON =
  "H4sIAAAAAAAC/+2WzW7cIBDHfe5TIOdWNV7IflV5geTeYxMp1CZrGgwIsNKV5XcvjO1s4laKlCirKJmfD6BhgAH0n7E3tja6+O2Nzt4KGtmsVtBG5i1dbw99sDPKGM0IzY5A6wN3hGSflC6/Na7hIT8n7BvJnTGpm9dCKZNHg+XlHd8JH40/u1xWTwbvpAaDVe1O6mTRvBHJcjm5SF2qthLJLbhW9NfR5kUIUu/Smt24WOrtnBDJDlvIvO+jK2+DidFJo8FlWv4H3xOYSCou1R4CdaaxEPvJCblIS13pK31wDIbcdN19bfr+poAJQ9CPzxNEYxUPsAVEk4zGpu2HYJVp4cD7eCEpvJJXQpcCxsLewsTSGbgK8cc64f0Qer6k5Dv5Gj92uo5H69/L+0/vu4BLWBQe8sHpcDmLoXltdnhO/2eUzfS/3ixR/0fS/0zT/yi4Er500o4aTNLzo6QEL2vSGKeTaCFXON6MmWJa5kHUcVzxX0Il48Vk7K/7DHlH+j/kW7+Ap3tkeXEaeE7/jJ3N9L9d0hXq/zj6/59qoc6lgjjWNH8vQ1k/0XCqrDKQ5BkLMAr5o+p/+K8qmurt6j9ly5n+N3SL+j8KDz/IxRcUA4IgCIIgCIIgCIIgCIIgCIIgyAfgL62iWQwAKAAA";

function fromBase64(text: string): Uint8Array {
  const letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const bytes: number[] = [];
  let held = 0;
  let bits = 0;
  for (const letter of text.replace(/=+$/, "")) {
    held = (held << 6) | letters.indexOf(letter);
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((held >> bits) & 0xff);
    }
  }
  return new Uint8Array(bytes);
}

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

test("a new automation is set without writing cron", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("Automations >> nth=0");
  // With none yet, the page offers a few to begin from.
  await app.waitFor("text:Start from scratch");
  expect(app.texts()).toContain("Find bugs in recent commits");
  await app.press("Start from scratch");
  await app.waitFor("text:Repeats");
  // Every day at 9:00 to begin with, said back in words.
  expect(app.texts()).toContain("Runs: Every day at 9:00");
  expect(app.texts()).toContain("Every week");
  expect(app.texts()).not.toContain("Cron expression, e.g. 0 9 * * 1-5");
});

test("an automation can begin from a template and run a command", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("Automations >> nth=0");
  await app.waitFor("text:Weekly changelog");
  await app.press("Weekly changelog");
  // Filled in from the template: when it runs, and what it is told.
  await app.waitFor("text:Runs: Fri at 16:00");
  expect(app.texts()).toContain("Instructions");
  expect(app.texts()).toContain("Start a conversation");
  expect(app.texts()).toContain("Runs a day");
  // What starts it has a name: the one picked on a line, the others a press
  // away. A script is a plugin's to bring, never typed in here.
  await app.press("Change >> nth=0");
  await app.waitFor("text:At the times you set below");
  expect(app.texts()).not.toContain("A script of your own");
  await app.press("On time");
  await app.waitFor("text:Repeats");
  // And what it does can be a command no agent has a part in.
  await app.press("Run a command");
  await app.waitFor("text:No conversation and no model: it does the same each time, and what it prints is kept with the run");
});

test("instructions are written as they read, and kept as Markdown", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("Automations >> nth=0");
  await app.waitFor("text:Start from scratch");
  await app.press("Start from scratch");
  await app.waitFor("text:Repeats");
  // The marks of Markdown become what they mean as they are typed: there
  // is no other view of the instructions to switch to.
  await app.type("label:Instructions", "## Steps");
  await app.keyboard.press("enter");
  await app.type("label:Instructions", "- run `gh pr view {{url}}`");
  await app.keyboard.press("enter");
  await app.type("label:Instructions", "say **only** what matters");
  expect(app.texts()).toContain("Steps");
  expect(app.texts()).not.toContain("## Steps");
  expect(app.texts()).not.toContain("Preview");
  expect(app.spanStyle("text:Steps")).toMatchObject({ fontSize: 18, fontWeight: 600 });
  expect(app.spanStyle("text:only")).toMatchObject({ fontWeight: 600 });
  expect(app.spanStyle("text:gh pr view").backgroundColor).toBeTruthy();
  // What is kept is the Markdown: the editor made again reads it back as
  // the same heading, list and code.
  await app.press("Run a command");
  await app.waitFor("text:No conversation and no model: it does the same each time, and what it prints is kept with the run");
  await app.press("Start a conversation");
  await app.waitFor("text:Steps");
  expect(app.texts()).toContain("run gh pr view {{url}}");
  expect(app.spanStyle("text:Steps")).toMatchObject({ fontSize: 18, fontWeight: 600 });
  expect(app.spanStyle("text:gh pr view").backgroundColor).toBeTruthy();
});

test("integrations have a page of their own to set up and add", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  // Behind More, with the pages less often wanted.
  await app.press("More >> nth=0");
  await app.waitFor("text:Integrations");
  await app.press("Integrations >> nth=0");
  await app.waitFor("text:Add an integration");
  expect(app.texts()).toContain("Add");
  expect(app.texts()).toContain("One someone shared with you, its settings filled in. It runs its scripts on that computer: add only what you trust");
});

test("plugins have a page of their own, apart from the integrations they count on", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  await app.press("More >> nth=0");
  await app.waitFor("text:Plugins");
  await app.press("Plugins >> nth=0");
  await app.waitFor("text:Add a plugin");
  expect(app.texts()).toContain("A plugin runs its scripts on that computer, and so do the integrations it brings: add only what you trust");
  // What connects a service is not listed here.
  expect(app.texts()).not.toContain("Add an integration");
});

test("a plugin is saved as a file, and a file is shown before it is added", async () => {
  const app = await open(".", { width: 1100, height: 900 });
  // By way of the automations the plugins bring: the headless runner does
  // not put a dialog away on Escape once a menu was the way to its page.
  await app.press("Automations >> nth=0");
  await app.waitFor("text:From plugins");
  await app.press("From plugins");
  await app.waitFor("text:Open Plugins");
  await app.press("Open Plugins");
  await app.waitFor("text:Add a plugin");
  // What would go in the file is said before it is made. One that comes
  // with Sophon, nothing set here: there is nothing to hand on yet.
  await app.press("label:Save as a file to share >> nth=0");
  await app.waitFor("text:Save “GitHub PR 值守” as a file");
  expect(app.texts()).toContain("A sign-in is never in the file: whoever adds it signs in themselves");
  expect(app.texts().some((text) => text.startsWith("Named, not packed, since it comes with Sophon: GitHub PR 值守"))).toBe(true);
  expect(app.enabled("Save file…")).toBe(false);
  expect(app.texts()).toContain("Nothing to put in the file yet: it comes with Sophon, and nothing is set or made with it here");
  await app.keyboard.press("escape");
  expect(app.texts()).not.toContain("Save file…");
  // A file someone handed on is read and shown: what it installs, what
  // it fills in, and the automation that comes with it. Nothing of it
  // is here until that is answered.
  app.onPick(() => [{ name: "hello.sophon", data: fromBase64(HELLO_SOPHON) }]);
  await app.press("Choose a file…");
  await app.waitFor("text:Add “Hello”");
  expect(app.texts()).toContain("Says hello each morning");
  expect(app.texts()).toContain("Fills in these settings");
  expect(app.texts()).toContain("Hello · Greeting");
  expect(app.texts()).toContain("Opens the automation “Say hello daily”");
  expect(app.texts()).toContain("What it installs runs its scripts on that computer: add only what you trust");
  // Put away unanswered, none of it is installed.
  await app.keyboard.press("escape");
  expect(app.texts()).not.toContain("Says hello each morning");
  expect(app.texts()).not.toContain("Hello");
  // One that is no file of Sophon's is refused, and says so.
  app.onPick(() => [{ name: "notes.sophon", data: new Uint8Array([1, 2, 3]) }]);
  await app.press("Choose a file…");
  await app.waitFor("text:Could not read the file: This is not a file Sophon made");
});
