// @vitest-environment node
// A press that reaches the browser as input runs microtasks between the event listeners. React commits the close
// of a select in that gap, before the dialog's own listener runs; a test in jsdom does not have that gap.
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { browserPage, type Point } from "../test/browser/harness";

const page = browserPage("/src/test/browser/dialogOverlay.tsx");
const { evaluate, until, center, press } = page;
beforeAll(page.open, 60_000);
afterAll(page.close);

/** A point on the backdrop, away from the dialog and from anything open over it. */
const onBackdrop: Point = { x: 8, y: 8 };

async function openPage() {
  await page.load();
  await until(`document.querySelector('[role="dialog"]')`);
}

describe("Dialog in a browser", () => {
  test("a press on the backdrop while a select is open closes the select and keeps the dialog open", async () => {
    await openPage();
    await press(await center('[role="combobox"]'));
    await until(`document.querySelector('[role="listbox"]')`);

    await press(onBackdrop);
    await until(`!document.querySelector('[role="listbox"]')`);

    expect(await evaluate("window.dialogCloses")).toEqual([]);
    expect(await evaluate(`!!document.querySelector('[role="dialog"]')`)).toBe(true);
  }, 30_000);

  test("a press on the backdrop while a menu is open closes the menu and keeps the dialog open", async () => {
    await openPage();
    await press(await center('[data-trigger="menu"]'));
    await until(`document.querySelector('[role="menu"]')`);

    await press(onBackdrop);
    await until(`!document.querySelector('[role="menu"]')`);

    expect(await evaluate("window.dialogCloses")).toEqual([]);
    expect(await evaluate(`!!document.querySelector('[role="dialog"]')`)).toBe(true);
  }, 30_000);

  test("a press on the backdrop with nothing open over the dialog closes it", async () => {
    await openPage();
    await press(onBackdrop);
    await until(`!document.querySelector('[role="dialog"]')`);

    expect(await evaluate("window.dialogCloses")).toEqual(["backdrop"]);
  }, 30_000);
});
