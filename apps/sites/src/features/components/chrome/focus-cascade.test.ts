import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";
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

type Result = Awaited<ReturnType<typeof tabTo>>;

describe("when a keyboard user focuses an element inside the Sites surface", () => {
  let result: Result;

  describe("and the element belongs to a web InputField, which rings its field", () => {
    beforeEach(async () => {
      result = await tabTo(
        `<div class="root" data-surface="web"><div class="field webField"><input class="input" aria-label="Email"></div></div>`,
        ".input",
        ".field",
      );
    });

    it("should give the input visible keyboard focus", () => {
      expect(result.focused).toBe(true);
    });

    it("should not paint the surface outline on the input", () => {
      expect(result.target.outline).toBe("none");
    });

    it("should paint the field's ring", () => {
      expect(result.painter.boxShadow).toMatch(/ 0px 0px 0px 2px inset$/);
    });
  });

  describe("and the element is a web select with a disabled placeholder", () => {
    beforeEach(async () => {
      result = await tabTo(
        `<div class="field webField nativeField"><select class="input nativeSelect" aria-label="Reason"><option value="" disabled selected>Choose a reason</option><option>Harassment</option></select></div>`,
        ".input",
        ".field",
      );
    });

    it("should remain visibly enabled", () => {
      expect(result.opacity).toBe("1");
    });

    it("should give the select visible keyboard focus", () => {
      expect(result.focused).toBe(true);
    });

    it("should not paint the surface outline on the select", () => {
      expect(result.target.outline).toBe("none");
    });

    it("should paint the shared field ring", () => {
      expect(result.painter.boxShadow).toMatch(/ 0px 0px 0px 2px inset$/);
    });
  });

  describe("and the element is a disabled web select", () => {
    beforeEach(async () => {
      result = await tabTo(
        `<div class="field webField nativeField"><select class="input nativeSelect" aria-label="Reason" disabled><option>Harassment</option></select></div>`,
        ".input",
        ".field",
      );
    });

    it("should dim the frame", () => {
      expect(result.opacity).toBe("0.4");
    });

    it("should skip focus", () => {
      expect(result.focused).toBe(false);
    });
  });

  describe("and the element is a PlaceCard's title link, whose card draws the ring", () => {
    beforeEach(async () => {
      result = await tabTo(
        `<div class="card"><a class="link" href="#place">Place</a></div>`,
        ".link",
        ".card",
      );
    });

    it("should give the link visible keyboard focus", () => {
      expect(result.focused).toBe(true);
    });

    it("should not paint the surface outline on the link", () => {
      expect(result.target.outline).toBe("none");
    });

    it("should paint the card's outline", () => {
      expect(result.painter.outline).toBe("solid 2px 2px");
    });
  });

  describe.each([
    ["a link", `<a id="plain" href="#plain">Plain</a>`],
    ["a button", `<button id="plain">Plain</button>`],
  ])("and the element is %s with no focus indicator of its own", (_, markup) => {
    beforeEach(async () => {
      result = await tabTo(markup, "#plain", "#plain");
    });

    it("should give it visible keyboard focus", () => {
      expect(result.focused).toBe(true);
    });

    it("should keep the surface's focus ring", () => {
      expect(result.target.outline).toBe("solid 2px 3px");
    });
  });
});
