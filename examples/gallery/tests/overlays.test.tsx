// Overlays: each page's trigger opens its overlay (pressed, or hovered for
// a tooltip and a hover card), which the test waits for, then screenshots of
// the open state in light and dark, drawn by macOS (`nsh test --target
// macos`), to compare with shadcn/ui's own (`reference/<id>-{light,dark}.png`,
// `reference/compare.html?c=<id>`; paths are relative to examples/gallery,
// where `nsh test` runs).
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const page = (id: string, colorScheme: "light" | "dark") =>
  open("..", { width: 880, height: 640, colorScheme, storage: { page: id, bare: "1" } });

/// Saves screenshots of `id` in light and dark once `show` opened its
/// overlay and `shows` is on screen, its entrance over.
const shoot = (id: string, show: (app: App) => Promise<void>, shows: string, away?: string) => {
  for (const scheme of ["light", "dark"] as const) {
    test(`${id}: screenshot (${scheme})`, async () => {
      const app = await page(id, scheme);
      await show(app);
      await app.waitFor(shows);
      // The pointer leaves the trigger it pressed, as a still screenshot of
      // shadcn's has no pointer on it.
      if (away) await app.hover(away);
      await app.wait(700);
      await app.screenshot(`../../screenshots/${id}-${scheme}.png`);
    });
  }
};

const press = (trigger: string) => (app: App) => app.press(trigger);
const hover = (trigger: string) => (app: App) => app.hover(trigger);

test("dialog: the trigger opens it, Cancel and the close button close it", async () => {
  const app = await page("dialog", "light");
  expect(app.texts()).not.toContain("Edit profile");
  await app.press("Open Dialog");
  await app.waitFor("Edit profile");
  await app.press("Cancel");
  await app.waitFor("Edit profile", { state: "gone" });
  await app.press("Open Dialog");
  await app.waitFor("Edit profile");
  await app.press("Close");
  await app.waitFor("Edit profile", { state: "gone" });
});

test("alert-dialog: Cancel and Continue close it, Escape too", async () => {
  const app = await page("alert-dialog", "light");
  await app.press("Show Dialog");
  await app.waitFor("Are you absolutely sure?");
  await app.press("Cancel");
  await app.waitFor("Are you absolutely sure?", { state: "gone" });
  await app.press("Show Dialog");
  await app.waitFor("Are you absolutely sure?");
  await app.press("Continue");
  await app.waitFor("Are you absolutely sure?", { state: "gone" });
  await app.press("Show Dialog");
  await app.waitFor("Are you absolutely sure?");
  await app.keyboard.press("escape");
  await app.waitFor("Are you absolutely sure?", { state: "gone" });
});

test("sheet: the trigger opens it, Close closes it; each side opens its own", async () => {
  const app = await page("sheet", "light");
  await app.press("Open");
  await app.waitFor("Edit profile");
  // The outline Close button (the corner's is "Close" too).
  await app.press("Close >> nth=1");
  await app.waitFor("Edit profile", { state: "gone" });
  await app.press("bottom");
  await app.waitFor("Edit profile");
  await app.press("Save changes");
  await app.waitFor("Edit profile", { state: "gone" });
});

test("drawer: the goal moves by ten within 200–400, Cancel closes it", async () => {
  const app = await page("drawer", "light");
  await app.press("Open Drawer");
  await app.waitFor("Move Goal");
  expect(app.texts()).toContain("350");
  await app.press("Increase");
  expect(app.texts()).toContain("360");
  await app.press("Decrease");
  await app.press("Decrease");
  expect(app.texts()).toContain("340");
  await app.press("Cancel");
  await app.waitFor("Move Goal", { state: "gone" });
});

test("popover: the trigger toggles it", async () => {
  const app = await page("popover", "light");
  await app.press("Open popover");
  await app.waitFor("Dimensions");
  await app.press("Open popover");
  await app.waitFor("Dimensions", { state: "gone" });
});

test("hover-card: resting on the trigger opens it", async () => {
  const app = await page("hover-card", "light");
  expect(app.texts()).not.toContain("Joined December 2021");
  await app.hover("@nextjs");
  await app.waitFor("Joined December 2021");
});

test("tooltip: resting on the trigger shows it", async () => {
  const app = await page("tooltip", "light");
  expect(app.texts()).not.toContain("Add to library");
  await app.hover("Hover");
  await app.waitFor("Add to library");
});

test("sonner: each button shows its toast; Undo dismisses it", async () => {
  const app = await page("sonner", "light");
  await app.press("Show Toast");
  await app.waitFor("Sunday, December 03, 2023 at 9:00 AM");
  await app.press("Undo");
  await app.waitFor("Sunday, December 03, 2023 at 9:00 AM", { state: "gone" });
  await app.press("Success");
  await app.waitFor("Event has been created");
  await app.press("Error");
  await app.waitFor("Event has not been created");
  await app.press("Promise");
  await app.waitFor("Loading...");
  await app.waitFor("Loading...", { state: "gone", timeout: 5000 });
});

shoot("dialog", press("Open Dialog"), "Edit profile", "Edit profile");
shoot("alert-dialog", press("Show Dialog"), "Are you absolutely sure?", "Are you absolutely sure?");
shoot("sheet", press("Open"), "Edit profile", "Edit profile");
shoot("drawer", press("Open Drawer"), "Move Goal", "Move Goal");
shoot("popover", press("Open popover"), "Dimensions", "Dimensions");
shoot("hover-card", hover("@nextjs"), "Joined December 2021");
shoot("tooltip", hover("Hover"), "Add to library");
shoot("sonner", press("Show Toast"), "Sunday, December 03, 2023 at 9:00 AM", "Sonner");
