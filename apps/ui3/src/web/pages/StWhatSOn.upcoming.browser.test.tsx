// @vitest-environment node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import buttonStyles from "../../../../../library/ui/components/Button/Button.module.css";
import buttonCss from "../../../../../library/ui/components/Button/Button.module.css?inline";
import cardActionStyles from "../../../../../library/ui/components/CardActionButton/CardActionButton.module.css";
import cardActionCss from "../../../../../library/ui/components/CardActionButton/CardActionButton.module.css?inline";
import { findBrowser, openSession } from "../../../../../library/ui/figma/review/scripts/capture-browser.mjs";
import StWhatSOn, { type WoUpcomingCard } from "./StWhatSOn";

const root = new URL("../../../../../", import.meta.url);
/** The ?inline copy of a CSS Module hashes its classes by its own id; map them back to the names the markup uses. */
const asRendered = (inline: string, styles: Record<string, string>) =>
  inline.replace(/\._([A-Za-z][\w-]*)_[0-9a-z]+\b/g, (match, name: string) => (styles[name] ? `.${styles[name]}` : match));
const css = [
  ...[
    "library/ui/foundations/colors/colors.css",
    "library/ui/foundations/spacing/spacing.css",
    "library/ui/foundations/layout/layout.css",
  ].map((file) => readFileSync(fileURLToPath(new URL(file, root)), "utf8")),
  asRendered(buttonCss, buttonStyles),
  asRendered(cardActionCss, cardActionStyles),
  readFileSync(fileURLToPath(new URL("apps/ui3/src/web/pages/stwhatson.css", root)), "utf8"),
].join("\n");

const upcoming: WoUpcomingCard[] = [
  { id: "a", name: "Introduction to Yoga (Extended 90min session)", creator: "ClydeRouge", time: "Today 9:00 AM", hue: 30 },
  { id: "b", name: "Poker Night", creator: "OtoLam", time: "Today 5:00 PM", hue: 200, attending: true },
];

type Box = { width: number; height: number; left: number; right: number; radius: string; background: string; color: string };
type Actions = {
  visible: boolean;
  remind: Box & { label: string; fontSize: string; fontWeight: string; transform: string; tracking: string; icon: { width: number; height: number } };
  share: Box & { icon: { width: number; height: number } };
};

const measure = `(() => {
  const card = document.querySelector(".wo-up__grid .wo-sm");
  const actions = card.querySelector(".wo-sm__actions");
  const box = (el) => { const r = el.getBoundingClientRect(), s = getComputedStyle(el); return { width: r.width, height: r.height, left: r.left, right: r.right, radius: s.borderRadius, background: s.backgroundColor, color: s.color }; };
  const size = (el) => { const r = el.getBoundingClientRect(); return { width: r.width, height: r.height }; };
  const remind = actions.querySelector("a"), share = actions.querySelector("button");
  const text = getComputedStyle(remind);
  return {
    visible: getComputedStyle(actions).visibility === "visible" && getComputedStyle(actions).opacity === "1",
    remind: { ...box(remind), label: remind.textContent.trim(), fontSize: text.fontSize, fontWeight: text.fontWeight, transform: text.textTransform, tracking: text.letterSpacing, icon: size(remind.querySelector("svg")) },
    share: { ...box(share), icon: size(share.querySelector("svg")) },
  };
})()`;

let session: Awaited<ReturnType<typeof openSession>>;
let startup: ReturnType<typeof openSession> | undefined;
const startupController = new AbortController();

async function evaluate<T>(expression: string): Promise<T> {
  const response = (await session.send("Runtime.evaluate", { expression, returnByValue: true })) as { result: { value: T } };
  return response.result.value;
}

async function load(width: number) {
  await session.send("Emulation.setDeviceMetricsOverride", { width, height: 1000, deviceScaleFactor: 1, mobile: false });
  const markup = renderToStaticMarkup(<StWhatSOn chrome={false} upcoming={upcoming} />);
  const html = `<!doctype html><style>body{margin:0}${css}*,*::before,*::after{transition:none!important}</style>${markup}`;
  await session.navigate(`data:text/html,${encodeURIComponent(html)}`);
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
  startupController.abort(new Error("Upcoming fixture disposed"));
  const opened = await startup?.catch(() => undefined);
  await opened?.close();
}, 30_000);

// Figma Events > card-hover, state=hover, live=false (16295:37451): CTAs 16295:37459.
describe.each([1280, 1440])("when an Upcoming card is hovered at %ipx", (width) => {
  it("should hide the actions until the card is hovered", async () => {
    await load(width);
    expect((await evaluate<Actions>(measure)).visible).toBe(false);
  });

  it("should show Remind me and Share at the sizes, radius, type and colours of the Figma CTAs", async () => {
    await load(width);
    const { x, y } = await evaluate<{ x: number; y: number }>(
      `(() => { const r = document.querySelector(".wo-up__grid .wo-sm").getBoundingClientRect(); return { x: r.x + r.width * 0.8, y: r.y + 10 }; })()`,
    );
    await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    const { visible, remind, share } = await evaluate<Actions>(measure);

    expect(visible).toBe(true);
    // Remind me (16326:4330): button sm, 32px tall, radius 8, Inter Bold 13 uppercase with 0.5px tracking, #fcfcfc on white 10%.
    expect(remind).toMatchObject({
      label: "Remind me",
      width: 134,
      height: 32,
      radius: "8px",
      fontSize: "13px",
      fontWeight: "700",
      transform: "uppercase",
      tracking: "0.5px",
      color: "rgb(252, 252, 252)",
      background: "rgba(255, 255, 255, 0.1)",
    });
    // The library Button sizes its icon at 1.25em: 16.25px at sm against Figma's 16.
    expect(remind.icon.width).toBeCloseTo(16, 0);
    expect(remind.icon.height).toBeCloseTo(16, 0);
    // Share (16338:1066): 32px square, radius 8, a 24px glyph, white 10%.
    expect(share).toMatchObject({ width: 32, height: 32, radius: "8px", background: "rgba(255, 255, 255, 0.1)", icon: { width: 24, height: 24 } });
    expect(share.left - remind.right).toBe(12);
  });

  it("should darken Remind me and Share to the public controls' white 20% on their own hover", async () => {
    await load(width);
    const hover = async (selector: string) => {
      const { x, y } = await evaluate<{ x: number; y: number }>(
        `(() => { const card = document.querySelector(".wo-up__grid .wo-sm"); const r = card.getBoundingClientRect(); return { x: r.x + r.width * 0.8, y: r.y + 10 }; })()`,
      );
      await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      const target = await evaluate<{ x: number; y: number }>(
        `(() => { const r = document.querySelector(".wo-up__grid .wo-sm ${selector}").getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
      );
      await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: target.x, y: target.y });
      return evaluate<string>(`getComputedStyle(document.querySelector(".wo-up__grid .wo-sm ${selector}")).backgroundColor`);
    };
    expect(await hover(".wo-sm__actions a")).toBe("rgba(255, 255, 255, 0.2)");
    expect(await hover(".wo-sm__actions button")).toBe("rgba(255, 255, 255, 0.2)");
  });
});

describe.each([390, 768])("when an Upcoming card is focused or hovered at %ipx, where its actions are hidden", (width) => {
  it.each(["focus", "hover"])("should keep the event time visible on %s", async (mode) => {
    await load(width);
    const card = ".wo-up__carousel .wo-sm";
    if (mode === "focus") await evaluate(`document.querySelector("${card} .wo-card__overlay").focus()`);
    else {
      const { x, y } = await evaluate<{ x: number; y: number }>(
        `(() => { const r = document.querySelector("${card}").getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + 10 }; })()`,
      );
      await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    }
    const state = await evaluate<{ time: string; actions: string; within: boolean }>(`(() => {
      const c = document.querySelector("${card}");
      return { time: getComputedStyle(c.querySelector('[data-role="time-pill"]')).opacity, actions: getComputedStyle(c.querySelector(".wo-sm__actions")).display, within: c.matches(":hover, :focus-within") };
    })()`);
    expect(state).toEqual({ time: "1", actions: "none", within: true });
  });
});
