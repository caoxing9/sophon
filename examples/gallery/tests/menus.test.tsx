// The menus family: behavior, and screenshots of each page in light and
// dark, drawn by macOS (`nsh test --target macos`), with its popup open, to
// compare with shadcn/ui's own (`reference/<id>-{light,dark}.png`,
// `reference/compare.html?c=<id>`). Paths are relative to examples/gallery.
import { expect, open, test } from "nsh/test";

const page = (id: string, colorScheme: "light" | "dark") =>
  open("..", { width: 880, height: 640, colorScheme, storage: { page: id, bare: "1" } });

test("dropdown-menu: an item chosen closes the menu", async () => {
  const app = await page("dropdown-menu", "light");
  await app.press("Open");
  await app.waitFor("Billing");
  await app.press("Billing");
  await app.waitFor("Chosen: Billing");
  expect(app.texts()).not.toContain("Profile");
});

test("dropdown-menu: a submenu opens beside its trigger", async () => {
  const app = await page("dropdown-menu", "light");
  await app.press("Open");
  await app.waitFor("Invite users");
  await app.hover("Invite users");
  await app.waitFor("Email");
  await app.press("Message");
  await app.waitFor("Chosen: Message");
});

for (const scheme of ["light", "dark"] as const) {
  test(`dropdown-menu: screenshot (${scheme})`, async () => {
    const app = await page("dropdown-menu", scheme);
    await app.press("Open");
    await app.waitFor("Log out");
    await app.wait(300);
    await app.screenshot(`../../screenshots/dropdown-menu-${scheme}.png`);
  });
}

test("context-menu: a right click opens it; a checkbox item toggles", async () => {
  const app = await page("context-menu", "light");
  await app.rightClick("Right click here");
  await app.waitFor("Show Full URLs");
  await app.press("Show Full URLs");
  await app.waitFor("Chosen: none, bookmarks on, URLs on, pedro");
  await app.rightClick("Right click here");
  await app.waitFor("Colm Tuite");
  await app.press("Colm Tuite");
  await app.waitFor("Chosen: none, bookmarks on, URLs on, colm");
});

for (const scheme of ["light", "dark"] as const) {
  test(`context-menu: screenshot (${scheme})`, async () => {
    const app = await page("context-menu", scheme);
    await app.rightClick("Right click here");
    await app.waitFor("Pedro Duarte");
    await app.wait(300);
    await app.screenshot(`../../screenshots/context-menu-${scheme}.png`);
  });
}

test("menubar: a menu opens, the pointer moves to the next", async () => {
  const app = await page("menubar", "light");
  await app.press("File");
  await app.waitFor("New Window");
  await app.hover("Edit");
  await app.waitFor("Undo");
  await app.press("Redo");
  await app.waitFor("Chosen: Redo");
});

for (const scheme of ["light", "dark"] as const) {
  test(`menubar: screenshot (${scheme})`, async () => {
    const app = await page("menubar", scheme);
    await app.press("File");
    await app.waitFor("Print...");
    await app.wait(300);
    await app.screenshot(`../../screenshots/menubar-${scheme}.png`);
  });
}

test("select: an item chosen shows on the trigger", async () => {
  const app = await page("select", "light");
  await app.press("Select a fruit");
  await app.waitFor("Blueberry");
  await app.press("Banana");
  await app.waitFor("Value: banana");
  expect(app.texts()).not.toContain("Blueberry");
});

for (const scheme of ["light", "dark"] as const) {
  test(`select: screenshot (${scheme})`, async () => {
    const app = await page("select", scheme);
    await app.press("Select a fruit");
    await app.waitFor("Blueberry");
    await app.press("Banana");
    await app.waitFor("Value: banana");
    await app.wait(300);
    await app.press("label:Fruit");
    await app.waitFor("Blueberry");
    await app.wait(300);
    await app.screenshot(`../../screenshots/select-${scheme}.png`);
  });
}

test("combobox: typing filters the list; a choice fills the input", async () => {
  const app = await page("combobox", "light");
  await app.type("Select a framework", "re");
  await app.waitFor("Remix");
  expect(app.texts()).not.toContain("Astro");
  await app.type("role:combobox", "zz");
  await app.waitFor("No items found.");
  await app.clear("role:combobox");
  await app.waitFor("Astro");
  await app.press("Astro");
  await app.waitFor("Value: Astro");
});

for (const scheme of ["light", "dark"] as const) {
  test(`combobox: screenshot (${scheme})`, async () => {
    const app = await page("combobox", scheme);
    // The trigger button at the input group's end (macOS reports no press
    // on the input itself yet: nsh-base's combobox opens on it elsewhere).
    await app.press("id:framework", { position: { x: 240, y: 18 } });
    await app.waitFor("Astro");
    await app.wait(300);
    await app.screenshot(`../../screenshots/combobox-${scheme}.png`);
  });
}

test("command: typing filters; Return chooses the highlighted item", async () => {
  const app = await page("command", "light");
  await app.type("Type a command or search...", "bil");
  await app.waitFor("Billing");
  expect(app.texts()).not.toContain("Calendar");
  await app.keyboard.press("enter");
  await app.waitFor("Chosen: billing");
});

test("command: a query that matches nothing shows the empty text", async () => {
  const app = await page("command", "light");
  await app.type("Type a command or search...", "zzz");
  await app.waitFor("No results found.");
});

for (const scheme of ["light", "dark"] as const) {
  test(`command: screenshot (${scheme})`, async () => {
    const app = await page("command", scheme);
    await app.waitFor("Calendar");
    await app.screenshot(`../../screenshots/command-${scheme}.png`);
  });
}

test("navigation-menu: a trigger opens its content; a link is chosen", async () => {
  const app = await page("navigation-menu", "light");
  await app.press("Components");
  await app.waitFor("Alert Dialog");
  await app.press("Tooltip");
  await app.waitFor("Chosen: Tooltip");
  await app.keyboard.press("escape");
  await app.press("Docs");
  await app.waitFor("Chosen: Docs");
});

for (const scheme of ["light", "dark"] as const) {
  test(`navigation-menu: screenshot (${scheme})`, async () => {
    const app = await page("navigation-menu", scheme);
    await app.press("Home");
    await app.waitFor("Introduction");
    await app.wait(300);
    await app.screenshot(`../../screenshots/navigation-menu-${scheme}.png`);
  });
}
