// Forms: behavior, and screenshots of each page in light and dark, drawn by
// macOS (`nsh test --target macos`), to compare with shadcn/ui's own
// (paths are relative to examples/gallery, where `nsh test` runs)
// (`reference/<id>-{light,dark}.png`, `reference/compare.html?c=<id>`).
import { expect, open, test } from "nsh/test";

const page = (id: string, colorScheme: "light" | "dark") =>
  open("..", { width: 880, height: 640, colorScheme, storage: { page: id, bare: "1" } });

test("checkbox: a press checks it", async () => {
  const app = await page("checkbox", "light");
  expect(app.texts()).toContain("Accepted: no");
  await app.press("label:Terms");
  expect(app.texts()).toContain("Accepted: yes");
  await app.press("label:Terms");
  expect(app.texts()).toContain("Accepted: no");
});

test("radio-group: a press chooses", async () => {
  const app = await page("radio-group", "light");
  expect(app.texts()).toContain("Density: comfortable");
  await app.press("label:Compact");
  expect(app.texts()).toContain("Density: compact");
});

test("switch: a press turns it on", async () => {
  const app = await page("switch", "light");
  expect(app.texts()).toContain("Airplane mode: off");
  await app.press("label:Airplane Mode");
  expect(app.texts()).toContain("Airplane mode: on");
});

test("textarea: typing reaches onChange", async () => {
  const app = await page("textarea", "light");
  await app.type("text:Write a message", "Hello");
  expect(app.texts()).toContain("Message: Hello");
});

test("field: a press on a label activates its control", async () => {
  const app = await page("field", "light");
  expect(app.accessibility("label:MFA")?.checked).toBe("false");
  await app.press("text:Multi-factor authentication");
  expect(app.accessibility("label:MFA")?.checked).toBe("true");
  await app.press("text:Virtual Machine");
  expect(app.accessibility("label:Virtual Machine")?.checked).toBe("true");
  expect(app.accessibility("label:Kubernetes")?.checked).toBe("false");
});

test("slider: a drag moves the thumb", async () => {
  const app = await page("slider", "light");
  expect(app.texts()).toContain("Volume: 50");
  await app.drag("label:Volume", { x: 47, y: 0 });
  expect(app.texts()).not.toContain("Volume: 50");
  expect(app.texts().some((t) => /^Volume: (6\d|70)$/.test(t))).toBe(true);
});

test("toggle: a press turns it on", async () => {
  const app = await page("toggle", "light");
  expect(app.texts()).toContain("Bookmarked: no");
  await app.press("label:Toggle bookmark");
  expect(app.texts()).toContain("Bookmarked: yes");
  expect(app.accessibility("label:Toggle bookmark")?.pressed).toBe("true");
});

test("toggle-group: presses turn items on and off", async () => {
  const app = await page("toggle-group", "light");
  await app.press("label:Toggle bold >> nth=0");
  await app.press("label:Toggle italic >> nth=0");
  expect(app.texts()).toContain("On: bold, italic");
  await app.press("label:Toggle bold >> nth=0");
  expect(app.texts()).toContain("On: italic");
});

test("button-group: its buttons press", async () => {
  const app = await page("button-group", "light");
  await app.press("Archive");
  expect(app.texts()).toContain("Archived: 1");
});

test("native-select: an option is chosen from the list", async () => {
  const app = await page("native-select", "light");
  await app.press("label:Status");
  await app.press("text:Done");
  expect(app.texts()).toContain("Status: done");
});

test("input-otp: typing fills the code", async () => {
  const app = await page("input-otp", "light");
  expect(app.texts()).toContain("Enter your one-time password.");
  await app.type("label:One-time password", "123456");
  expect(app.texts()).toContain("You entered: 123456");
});

test("input-group: an addon's button presses", async () => {
  const app = await page("input-group", "light");
  await app.press("label:Copy");
  expect(app.texts()).toContain("Copied: 1");
});

const PAGES = [
  "input",
  "textarea",
  "checkbox",
  "radio-group",
  "switch",
  "slider",
  "toggle",
  "toggle-group",
  "button-group",
  "input-otp",
  "input-group",
  "field",
];

for (const id of PAGES) {
  for (const scheme of ["light", "dark"] as const) {
    test(`${id}: screenshot (${scheme})`, async () => {
      const app = await page(id, scheme);
      await app.screenshot(`../../screenshots/${id}-${scheme}.png`);
    });
  }
}

// The native select's list opens on its trigger before the screenshot.
for (const scheme of ["light", "dark"] as const) {
  test(`native-select: screenshot (${scheme})`, async () => {
    const app = await page("native-select", scheme);
    await app.press("label:Status");
    await app.screenshot(`../../screenshots/native-select-${scheme}.png`);
  });
}
