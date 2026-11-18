// @vitest-environment node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { findBrowser, openSession } from "../../../../../library/ui/figma/review/scripts/capture-browser.mjs";
import StWhatSOn, { type WoLiveCard } from "./StWhatSOn";

const root = new URL("../../../../../", import.meta.url);
const css = [
  "library/ui/foundations/colors/colors.css",
  "library/ui/foundations/spacing/spacing.css",
  "library/ui/foundations/layout/layout.css",
  "apps/ui3/src/web/pages/stwhatson.css",
]
  .map((file) => readFileSync(fileURLToPath(new URL(file, root)), "utf8"))
  .join("\n");

const cards: WoLiveCard[] = [
  { id: "a", title: "Stoney Eye's MO FYAH MONDAYS at casa Roustan!", users: 24, isEvent: true, creator: "Roustan", hue: 75 },
  { id: "b", title: "Meta GamiMall", users: 5, isEvent: false, creator: "MetaDoge", hue: 311 },
  { id: "c", title: "Genesis Plaza", users: 3, isEvent: false, creator: "Decentraland Foundation", hue: 200 },
];

type Geometry = {
  section: number;
  cards: { height: number; title: number; creator: number; creatorOpacity: string; jumpOpacity: string }[];
};

const measure = `(() => {
  const section = document.querySelector(".wo-ln");
  return {
    section: section.getBoundingClientRect().height,
    cards: [...section.querySelectorAll(".wo-ln__card")].map((card) => {
      const box = card.getBoundingClientRect();
      const row = card.querySelector(".wo-ln__creatorrow");
      return {
        height: box.height,
        title: card.querySelector(".wo-ln__title").getBoundingClientRect().top - box.top,
        creator: row.getBoundingClientRect().top - box.top,
        creatorOpacity: getComputedStyle(row).opacity,
        jumpOpacity: getComputedStyle(card.querySelector(".wo-ln__jumpwrap")).opacity,
      };
    }),
  };
})()`;

let session: Awaited<ReturnType<typeof openSession>>;
let startup: ReturnType<typeof openSession> | undefined;
const startupController = new AbortController();

async function evaluate<T>(expression: string): Promise<T> {
  const response = (await session.send("Runtime.evaluate", { expression, returnByValue: true })) as { result: { value: T } };
  return response.result.value;
}

async function load(width: number): Promise<Geometry> {
  await session.send("Emulation.setDeviceMetricsOverride", { width, height: 1000, deviceScaleFactor: 1, mobile: false });
  const markup = renderToStaticMarkup(<StWhatSOn chrome={false} liveNow={cards} />);
  // Transitions off, so each reading is the settled state rather than a frame of the animation.
  const html = `<!doctype html><style>body{margin:0}${css}*,*::before,*::after{transition:none!important}</style>${markup}`;
  await session.navigate(`data:text/html,${encodeURIComponent(html)}`);
  return evaluate<Geometry>(measure);
}

async function pointAt(selector: string) {
  const { x, y } = await evaluate<{ x: number; y: number }>(
    `(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
  );
  await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
}

async function pressTab() {
  await session.send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
  await session.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
}

function expectSameLayout(actual: Geometry, rest: Geometry) {
  expect(actual.section).toBe(rest.section);
  expect(actual.cards.map(({ height, title, creator }) => ({ height, title, creator }))).toEqual(
    rest.cards.map(({ height, title, creator }) => ({ height, title, creator })),
  );
}

beforeAll(async () => {
  startup = (async () =>
    openSession(
      await findBrowser([
        process.env.BROWSER_PATH,
        process.env.CHROME_PATH,
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
      ]),
      { signal: startupController.signal },
    ))();
  session = await startup;
}, 40_000);

afterAll(async () => {
  startupController.abort(new Error("Live Now fixture disposed"));
  const opened = await startup?.catch(() => undefined);
  await opened?.close();
}, 30_000);

describe.each([1280, 1440, 1920])("when the Live Now rail renders at %ipx", (width) => {
  it("should show the by-line and hide Jump In at rest", async () => {
    const rest = await load(width);
    expect(rest.cards.map((card) => [card.creatorOpacity, card.jumpOpacity])).toEqual(cards.map(() => ["1", "0"]));
  });

  it("should reveal Jump In on hover without resizing any card or moving its title and by-line", async () => {
    const rest = await load(width);
    await pointAt(".wo-ln__card:nth-child(2) .wo-ln__media");
    const hovered = await evaluate<Geometry>(measure);

    expectSameLayout(hovered, rest);
    expect(hovered.cards[1]).toMatchObject({ creatorOpacity: "0", jumpOpacity: "1" });
    expect(hovered.cards[0]).toMatchObject({ creatorOpacity: "1", jumpOpacity: "0" });
  });

  it("should reveal Jump In for a keyboard-focused card without resizing it", async () => {
    const rest = await load(width);
    await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 0, y: 0 });
    await pressTab();
    expect(await evaluate<string>(`document.activeElement.className`)).toBe("wo-ln__area");
    const focused = await evaluate<Geometry>(measure);

    expectSameLayout(focused, rest);
    expect(focused.cards[0]).toMatchObject({ creatorOpacity: "0", jumpOpacity: "1" });
    expect(focused.cards[1]).toMatchObject({ creatorOpacity: "1", jumpOpacity: "0" });
  });

  it("should keep Jump In clear of the title it overlays the by-line beneath", async () => {
    await load(width);
    await pointAt(".wo-ln__card:first-child .wo-ln__media");
    const gap = await evaluate<number>(
      `(() => { const card = document.querySelector(".wo-ln__card"); return card.querySelector(".wo-ln__jump").getBoundingClientRect().top - card.querySelector(".wo-ln__title").getBoundingClientRect().bottom; })()`,
    );
    expect(gap).toBeGreaterThanOrEqual(0);
  });
});
