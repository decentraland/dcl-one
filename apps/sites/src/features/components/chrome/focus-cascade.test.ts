import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import {
  type CaptureSession,
  START_BUDGET_MS,
  startCaptureBrowser,
} from "./capture-browser.helpers";

const root = new URL("../../../../../../", import.meta.url);
const read = (file: string) =>
  readFileSync(fileURLToPath(new URL(file, root)), "utf8");
// The Sites surface (loaded by SitesChrome through siteschrome.css) and two public components that
// draw their own focus indicator. Module class names stay unhashed in a plain stylesheet.
const css = [
  read("library/ui/foundations/colors/colors.css"),
  read("apps/ui3/src/atoms/surface-tokens.css"),
  read("apps/ui3/src/components/surface.css").replace(/^@import .*$/m, ""),
  read("library/ui/components/InputField/InputField.module.css"),
  read("library/ui/components/PlaceCard/PlaceCard.module.css"),
].join("\n");

let session: CaptureSession;
let startup: Promise<CaptureSession> | undefined;
const startupController = new AbortController();

// Teardown also owns a startup that outlives this fixture's budget.
beforeAll(async () => {
  startup = startCaptureBrowser("Focus cascade", startupController.signal);
  session = await startup;
}, START_BUDGET_MS + 10_000);

afterAll(async () => {
  startupController.abort(new Error("Focus cascade fixture disposed"));
  const opened = await startup?.catch(() => undefined);
  await opened?.close();
}, 30_000);

type Ring = { outline: string; boxShadow: string };

/** Tabs from a button placed just before the target, so the target gets keyboard focus. */
async function tabTo(markup: string, target: string, painter: string) {
  const html = `<!doctype html><style>body{margin:0}${css}</style><div class="st"><button id="start">start</button>${markup}</div>`;
  await session.navigate(`data:text/html,${encodeURIComponent(html)}`);
  await session.send("Runtime.evaluate", {
    expression: `document.getElementById("start").focus()`,
  });
  await session.send("Input.dispatchKeyEvent", {
    type: "rawKeyDown",
    key: "Tab",
    code: "Tab",
    windowsVirtualKeyCode: 9,
  });
  await session.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "Tab",
    code: "Tab",
    windowsVirtualKeyCode: 9,
  });
  const response = (await session.send("Runtime.evaluate", {
    expression: `(() => {
      const ring = (el) => { const s = getComputedStyle(el); return { outline: s.outlineStyle === "none" ? "none" : s.outlineStyle + " " + s.outlineWidth + " " + s.outlineOffset, boxShadow: s.boxShadow }; };
      const target = document.querySelector(${JSON.stringify(target)});
      const field = document.querySelector(${JSON.stringify(painter)});
      return { focused: document.activeElement === target && target.matches(":focus-visible"), target: ring(target), painter: ring(field), opacity: getComputedStyle(field).opacity };
    })()`,
    returnByValue: true,
  })) as {
    result: {
      value: { focused: boolean; target: Ring; painter: Ring; opacity: string };
    };
  };
  return response.result.value;
}

describe("when a keyboard user focuses an element inside the Sites surface", () => {
  describe("and the element belongs to a web InputField, which rings its field", () => {
    it("should paint only the field's ring, not the surface outline on the input", async () => {
      const result = await tabTo(
        `<div class="root" data-surface="web"><div class="field webField"><input class="input" aria-label="Email"></div></div>`,
        ".input",
        ".field",
      );
      expect(result.focused).toBe(true);
      expect(result.target.outline).toBe("none");
      expect(result.painter.boxShadow).toMatch(/ 0px 0px 0px 2px inset$/);
    });
  });

  describe("and the element is a web select with a disabled placeholder", () => {
    it("should remain visibly enabled and paint the shared field ring", async () => {
      const result = await tabTo(
        `<div class="field webField nativeField"><select class="input nativeSelect" aria-label="Reason"><option value="" disabled selected>Choose a reason</option><option>Harassment</option></select></div>`,
        ".input",
        ".field",
      );
      expect(result.opacity).toBe("1");
      expect(result.focused).toBe(true);
      expect(result.target.outline).toBe("none");
      expect(result.painter.boxShadow).toMatch(/ 0px 0px 0px 2px inset$/);
    });

    it("should dim the frame and skip focus for a disabled control", async () => {
      const result = await tabTo(
        `<div class="field webField nativeField"><select class="input nativeSelect" aria-label="Reason" disabled><option>Harassment</option></select></div>`,
        ".input",
        ".field",
      );
      expect(result.opacity).toBe("0.4");
      expect(result.focused).toBe(false);
    });
  });

  describe("and the element is a PlaceCard's title link, whose card draws the ring", () => {
    it("should paint only the card's outline, not the surface outline on the link", async () => {
      const result = await tabTo(
        `<div class="card"><a class="link" href="#place">Place</a></div>`,
        ".link",
        ".card",
      );
      expect(result.focused).toBe(true);
      expect(result.target.outline).toBe("none");
      expect(result.painter.outline).toBe("solid 2px 2px");
    });
  });

  describe("and the element has no focus indicator of its own", () => {
    it.each([
      ["a link", `<a id="plain" href="#plain">Plain</a>`],
      ["a button", `<button id="plain">Plain</button>`],
    ])("should keep the surface's focus ring on %s", async (_, markup) => {
      const result = await tabTo(markup, "#plain", "#plain");
      expect(result.focused).toBe(true);
      expect(result.target.outline).toBe("solid 2px 3px");
    });
  });
});
