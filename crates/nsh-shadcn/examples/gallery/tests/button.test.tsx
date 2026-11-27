// Button: behavior, and screenshots of its page in light and dark, drawn by
// macOS (`nsh test --target macos`), to compare with shadcn/ui's own
// (paths are relative to examples/gallery, where `nsh test` runs)
// (`reference/button-{light,dark}.png`, `reference/compare.html?c=button`).
import { expect, open, test } from "nsh/test";

const page = (id: string, colorScheme: "light" | "dark") =>
  open("..", { width: 880, height: 640, colorScheme, storage: { page: id, bare: "1" } });

test("button: presses count", async () => {
  const app = await page("button", "light");
  expect(app.texts()).toContain("Pressed: 0");
  await app.press("Secondary");
  await app.press("Outline");
  expect(app.texts()).toContain("Pressed: 2");
});

for (const scheme of ["light", "dark"] as const) {
  test(`button: screenshot (${scheme})`, async () => {
    const app = await page("button", scheme);
    await app.screenshot(`../../screenshots/button-${scheme}.png`);
  });
}
