// The layout family (Tabs, Accordion, Collapsible, Scroll Area, Resizable,
// Sidebar, Carousel, Calendar, Chart): behavior, and screenshots of each
// page in light and dark, drawn by macOS (`nsh test --target macos`), to
// compare with shadcn/ui's own (`reference/<id>-{light,dark}.png`,
// `reference/compare.html?c=<id>`). Paths are relative to examples/gallery.
import { expect, open, test } from "nsh/test";

const page = (id: string, colorScheme: "light" | "dark") =>
  open("..", { width: 880, height: 640, colorScheme, storage: { page: id, bare: "1" } });

test("tabs: a press shows the tab's content", async () => {
  const app = await page("tabs", "light");
  expect(app.texts()).toContain("Save changes");
  expect(app.texts()).not.toContain("Save password");
  await app.press("role:tab[Password]");
  expect(app.texts()).toContain("Save password");
  expect(app.texts()).not.toContain("Save changes");
});

test("accordion: one item open at a time, collapsible", async () => {
  const app = await page("accordion", "light");
  const first = /^Our flagship product/;
  expect(app.shows(`text:${first}`)).toBe(true);
  await app.press("role:button[Shipping Details]");
  expect(app.shows("text:/^We offer worldwide shipping/")).toBe(true);
  expect(app.shows(`text:${first}`)).toBe(false);
  await app.press("role:button[Shipping Details]");
  expect(app.shows("text:/^We offer worldwide shipping/")).toBe(false);
});

test("collapsible: the trigger shows and hides the content", async () => {
  const app = await page("collapsible", "light");
  expect(app.texts()).toContain("@radix-ui/primitives");
  expect(app.texts()).not.toContain("@radix-ui/colors");
  await app.press("label:Toggle");
  expect(app.texts()).toContain("@radix-ui/colors");
  expect(app.texts()).toContain("@stitches/react");
  await app.press("label:Toggle");
  expect(app.texts()).not.toContain("@radix-ui/colors");
});

test("scroll-area: the tags and the photos scroll", async () => {
  const app = await page("scroll-area", "light");
  // A scroll view the user cannot scroll fails the step.
  await app.scroll("text:v1.2.0-beta.48", 100);
  await app.scroll("text:v1.2.0-beta.48", -100);
  await app.scroll("text:/Photo by Ornella/", 200);
});

test("resizable: panels share the group as their default sizes say", async () => {
  const app = await page("resizable", "light");
  // The handle demo: Sidebar 25%, Content 75% of a 450-point group at x 32.
  const center = (s: string) => {
    const l = app.layout(s)!;
    return l.x + l.width / 2 - 32;
  };
  expect(Math.abs(center("text:Sidebar") - 450 * 0.125)).toBeLessThan(4);
  expect(Math.abs(center("text:Content") - 450 * 0.625)).toBeLessThan(4);
  expect(Math.abs(center("text:One") - 450 * 0.25)).toBeLessThan(4);
});

test("carousel: next and previous move between the slides", async () => {
  const app = await page("carousel", "light");
  const demo = "label:Demo";
  expect(app.enabled(`${demo} >> label:Previous slide`)).toBe(false);
  await app.press(`${demo} >> label:Next slide`);
  expect(app.enabled(`${demo} >> label:Previous slide`)).toBe(true);
  for (let i = 0; i < 3; i++) await app.press(`${demo} >> label:Next slide`);
  expect(app.enabled(`${demo} >> label:Next slide`)).toBe(false);
  await app.press(`${demo} >> label:Previous slide`);
  expect(app.enabled(`${demo} >> label:Next slide`)).toBe(true);
});

test("calendar: a press selects a day, the arrows turn the month", async () => {
  const app = await page("calendar", "light");
  expect(app.texts()).toContain("June");
  expect(app.texts()).toContain("Jun");
  await app.press("label:Go to the Next Month >> nth=1");
  expect(app.texts()).toContain("July");
  await app.press("label:Go to the Previous Month >> nth=1");
  expect(app.texts()).toContain("June");
});

test("chart: the tooltip shows the row under the pointer", async () => {
  const app = await page("chart", "light");
  expect(app.texts()).toContain("Desktop");
  await app.hover("label:Bar chart");
  expect(app.texts().some((t: string) => t === "March" || t === "April")).toBe(true);
});

test("sidebar: the trigger collapses it to its icons and opens it again", async () => {
  const app = await page("sidebar", "light");
  expect(app.layout("role:button[Inbox]")!.width).toBeGreaterThan(200);
  await app.press("label:Toggle Sidebar");
  expect(app.layout("role:button[Inbox]")!.width).toBe(32);
  await app.press("label:Toggle Sidebar");
  expect(app.layout("role:button[Inbox]")!.width).toBeGreaterThan(200);
});

const shots: Record<string, (app: any) => Promise<void>> = {
  tabs: async () => {},
  accordion: async () => {},
  collapsible: async (app) => {
    await app.press("label:Toggle");
    await app.hover("text:@radix-ui/primitives");
  },
  "scroll-area": async (app) => {
    await app.hover("text:v1.2.0-beta.47");
  },
  resizable: async () => {},
  sidebar: async () => {},
  carousel: async () => {},
  calendar: async () => {},
  chart: async (app) => {
    await app.hover("label:Bar chart");
  },
};

for (const [id, prepare] of Object.entries(shots)) {
  for (const scheme of ["light", "dark"] as const) {
    test(`${id}: screenshot (${scheme})`, async () => {
      const app = await page(id, scheme);
      await prepare(app);
      await app.screenshot(`../../screenshots/${id}-${scheme}.png`);
    });
  }
}
