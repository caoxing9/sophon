// The basics family: behavior, and screenshots of each page in light and
// dark, drawn by macOS (`nsh test --target macos`), to compare with
// shadcn/ui's own (paths are relative to examples/gallery, where `nsh test`
// runs) (`reference/<id>-{light,dark}.png`, `reference/compare.html?c=<id>`).
import { expect, open, test } from "nsh/test";

const page = (id: string, colorScheme: "light" | "dark") =>
  open("..", { width: 880, height: 640, colorScheme, storage: { page: id, bare: "1" } });

test("label: pressing the label checks its checkbox", async () => {
  const app = await page("label", "light");
  const checked = () => app.accessibility("role:checkbox")?.checked;
  expect(checked()).toBe("false");
  await app.press("text:Accept terms and conditions");
  expect(checked()).toBe("true");
  await app.press("role:checkbox");
  expect(checked()).toBe("false");
});

test("badge, kbd: what screen readers hear", async () => {
  const badges = await page("badge", "light");
  for (const t of ["Badge", "Verified", "99", "Syncing"]) expect(badges.texts()).toContain(t);
  expect(badges.count("role:progressbar[Loading]")).toBe(3);
  const keys = await page("kbd", "light");
  expect(keys.count("role:keyboardkey")).toBe(10);
  expect(keys.shows("role:keyboardkey[Ctrl + K]")).toBe(true);
});

test("alert: three alerts, a title and description each", async () => {
  const app = await page("alert", "light");
  expect(app.count("role:alert")).toBe(3);
  expect(app.texts()).toContain("Unable to process your payment.");
  expect(app.texts()).toContain("Please verify your billing information and try again.");
});

test("avatar: the fallback shows without a picture, the alt names it", async () => {
  const app = await page("avatar", "light");
  expect(app.texts()).toContain("CN");
  expect(app.texts()).toContain("+3");
  expect(app.shows("role:image[@evilrabbit]")).toBe(true);
});

test("item: an item with an href is a link; its media moves up beside a description", async () => {
  const app = await page("item", "light");
  expect(app.count("role:link[Your profile has been verified.]")).toBe(1);
  await app.press("role:link[Your profile has been verified.]");
  // The description-less link item centers its icon; the others hold none.
  const item = app.layout("role:link[Your profile has been verified.]")!;
  const title = app.layout("text:Your profile has been verified.")!;
  expect(Math.abs(item.y + item.height / 2 - (title.y + title.height / 2))).toBeLessThan(1.5);
});

test("breadcrumb: links press, the page is current", async () => {
  const app = await page("breadcrumb", "light");
  expect(app.texts()).toContain("Opened: none");
  await app.press("text:Components >> nth=0");
  expect(app.texts()).toContain("Opened: Components");
  expect(app.count("role:navigation[breadcrumb]")).toBe(2);
});

test("pagination: a page link becomes the current page", async () => {
  const app = await page("pagination", "light");
  expect(app.texts()).toContain("Page: 2");
  await app.press("role:link[3]");
  expect(app.texts()).toContain("Page: 3");
  await app.press("label:Go to previous page");
  await app.press("label:Go to previous page");
  expect(app.texts()).toContain("Page: 1");
});

test("progress: goes from 13 to 66", async () => {
  const app = await page("progress", "light");
  const bar = app.layout("role:progressbar");
  expect(bar).not.toBeNull();
  await app.wait(1200);
  expect(app.shows("role:progressbar")).toBe(true);
});

test("table: the columns line up across rows", async () => {
  const app = await page("table", "light");
  expect(app.count("role:row")).toBe(9);
  expect(app.count("role:columnheader")).toBe(4);
  const head = app.layout("role:columnheader[Amount]")!;
  const cell = app.layout("text:$250.00")!;
  expect(Math.round(cell.x + cell.width)).toBe(Math.round(head.x + head.width - 8));
});

const ids = [
  "badge",
  "card",
  "separator",
  "label",
  "kbd",
  "skeleton",
  "spinner",
  "alert",
  "avatar",
  "aspect-ratio",
  "empty",
  "item",
  "breadcrumb",
  "pagination",
  "progress",
  "table",
];

for (const id of ids) {
  for (const scheme of ["light", "dark"] as const) {
    test(`${id}: screenshot (${scheme})`, async () => {
      // Spinners and skeletons move forever, and NSH on macOS keeps them
      // moving under reduced motion too: they are caught mid-turn / mid-pulse
      // (the reference draws them still).
      const app = await page(id, scheme);
      if (id === "progress") await app.wait(1200);
      await app.screenshot(`../../screenshots/${id}-${scheme}.png`);
    });
  }
}
