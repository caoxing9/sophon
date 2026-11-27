// The chat family (Message, Message Scroller, Bubble, Attachment, Marker,
// Direction): behavior, and screenshots of each page in light and dark,
// drawn by macOS (`nsh test --target macos`), to compare with shadcn/ui's
// own (`reference/<page>-{light,dark}.png`, `reference/compare.html?c=<page>`).
// Paths are relative to examples/gallery, where `nsh test` runs.
import { expect, open, test } from "nsh/test";

const page = (id: string, colorScheme: "light" | "dark") =>
  open("..", { width: 880, height: 640, colorScheme, storage: { page: id, bare: "1" } });

const PAGES = ["message", "message-scroller", "bubble", "attachment", "marker", "direction"];

test("message: a conversation with a footer, reactions and a marker", async () => {
  const app = await page("message", "light");
  expect(app.texts()).toContain("Deploying to prod real quick.");
  expect(app.texts()).toContain("Delivered");
  expect(app.texts()).toContain("sales-dashboard.pdf");
  expect(app.shows("label:Reactions: thumbs up")).toBe(true);
  // The user's message is on the end side, its avatar after the bubble.
  const mine = app.layout("text:Deploying to prod real quick.")!;
  const yours = app.layout("text:It's 4:55 PM. On a Friday.")!;
  expect(mine.x + mine.width).toBeGreaterThan(yours.x + yours.width + 100);
  await app.press("label:Download");
});

test("message-scroller: opens at the end, anchors a new turn, scrolls back to the end", async () => {
  const app = await page("message-scroller", "light");
  await app.wait(300);
  const viewport = app.layout("label:Messages")!;
  const bottom = viewport.y + viewport.height;
  // Opened at its end: the last message shows, and nothing is below.
  const last = app.layout("text:MessageScrollerItem fixes that with turn anchoring. Set scrollAnchor on the turn that should settle near the top instead of blindly snapping to the document bottom.")!;
  expect(last.y + last.height).toBeLessThanOrEqual(bottom + 1);
  expect(app.enabled("label:Scroll to end")).toBe(false);

  // The user's next message anchors near the top: 64 points of the turn
  // before it peek above it.
  await app.press("label:Send");
  await app.wait(300);
  const question = "text:And if they've scrolled up to re-read an older answer? I don't want to yank them back down.";
  const asked = app.layout(question)!;
  expect(asked.y - viewport.y).toBeGreaterThan(64);
  expect(asked.y - viewport.y).toBeLessThan(64 + 40);
  expect(app.enabled("label:Scroll to end")).toBe(false);

  // The reply streams in below it; the question stays where it is.
  await app.press("label:Send");
  await app.wait(1500);
  expect(Math.abs(app.layout(question)!.y - asked.y)).toBeLessThan(1);
  expect(app.texts().some((t) => t.startsWith("You won't."))).toBe(true);

  // Scrolled up, the button shows; it brings the reader back to the end.
  await app.scroll("label:Messages", -400);
  await app.wait(300);
  expect(app.enabled("label:Scroll to end")).toBe(true);
  await app.press("label:Scroll to end");
  await app.wait(800);
  expect(app.enabled("label:Scroll to end")).toBe(false);
  const reply = app.layout("text:You won't. Auto-scroll only runs when the viewport is already pinned to the bottom, so scrolling up is a deliberate opt-out. When there is content they haven't seen yet, MessageScrollerButton appears at the bottom of the viewport.")!;
  expect(reply.y + reply.height).toBeLessThanOrEqual(bottom + 1);

  // A long reply: it fills the room under its question, then auto-scroll
  // follows its last line.
  await app.press("label:Send");
  await app.wait(300);
  await app.press("label:Send");
  await app.wait(3000);
  const long = app.layout("text:" + app.texts().find((t) => t.startsWith("MessageScrollerContent is a live region"))!)!;
  const now = app.layout("label:Messages")!;
  expect(long.y + long.height).toBeLessThanOrEqual(now.y + now.height + 1);
  expect(long.y + long.height).toBeGreaterThan(now.y + now.height - 40);
  expect(app.enabled("label:Scroll to end")).toBe(false);
});

test("bubble: a pressable content takes presses", async () => {
  const app = await page("bubble", "light");
  await app.press("I forgot my password");
  expect(app.texts()).toContain("Quick replies: 1");
});

test("marker: link and button markers take presses", async () => {
  const app = await page("marker", "light");
  await app.press("View the pull request");
  await app.press("Revert this change");
  expect(app.texts()).toContain("Pressed: 2");
});

test("direction: the provider lays its content out right to left", async () => {
  const app = await page("direction", "light");
  const ltr = app.layout("text:Can you summarize the release notes?")!;
  const rtl = app.layout("text:هل يمكنك تلخيص ملاحظات الإصدار؟")!;
  // The user's bubble is at the end: the right in one, the left in the other.
  const ltrAvatar = app.layout("text:Sure. Three fixes and one new feature.")!;
  expect(ltr.x).toBeGreaterThan(ltrAvatar.x);
  const rtlReply = app.layout("text:بالتأكيد. ثلاثة إصلاحات وميزة جديدة.")!;
  expect(rtl.x + rtl.width).toBeLessThan(rtlReply.x + rtlReply.width);
});

for (const id of PAGES) {
  for (const scheme of ["light", "dark"] as const) {
    test(`${id}: screenshot (${scheme})`, async () => {
      const app = await page(id, scheme);
      await app.wait(300);
      await app.screenshot(`../../screenshots/${id}-${scheme}.png`);
    });
  }
}
