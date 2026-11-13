// The /jump pages: the deep-link card a shared place or event link opens (sites: pages/jump, components/jump).
// The reference renders them in its Layout (LandingNavbar, <main>, LandingFooter) over the AnimatedBackground;
// dcl-one renders them in SitesChrome over the same background (library/ui/apps/sites/jump), with the card's
// title as its h1 and the invalid notice's message as a p, where the reference has an h2 and an h6.
// /jump and /jump/places?position=1,95 show a place card, /jump/event?position=0,5 (the legacy path, which
// redirects to /jump/events on both sides) and /jump/events?id= an event card, and the invalid pages the
// "Oops!" box a broken link lands on.
import { signedIn, type KeyElement, type PageSpec, type PerSide } from "../../lib/spec.mts";
import { mainText, SITES_UNRECORDED, sitesFrame, sitesFrameRules } from "./frame.mts";

const UP = "translate(normalize-space(),'abcdefghijklmnopqrstuvwxyz','ABCDEFGHIJKLMNOPQRSTUVWXYZ')";
const control = (text: string) => `xpath=//main//*[self::a or self::button][${UP}='${text}']`;

/** An upcoming event of the recording (The Absolute Basics of Building in Decentraland, 2026-11-10 16:00 UTC). */
export const JUMP_EVENT_ID = "7e27a9d4-f006-4bae-be59-1db06706f28c";

/** The same xpath on each side, around the reference's h2 and dcl-one's h1. */
const titled = (before: string, after: string): PerSide<string> => ({
  reference: `xpath=${before}h2${after}`,
  "dcl-one": `xpath=${before}h1${after}`,
});
const TITLE = titled("//main//", "");
const CARD_KEYS: KeyElement[] = [
  { name: "card.image", selector: titled("(//main//", "/ancestor::*[.//img][2]//img)[1]") },
  { name: "card.title", selector: TITLE },
  { name: "card.creator", selector: titled("//main//", "/following-sibling::*[1]") },
  { name: "card.meta", selector: titled("//main//", "/following-sibling::*[2]") },
  { name: "card.description", selector: titled("//main//", "/following-sibling::*[3]") },
];

/** Keyframed motion both sides run from mount: the AnimatedBackground's WebGL canvas and the badge's live pulse. */
async function holdMotion(page: import("playwright-core").Page): Promise<void> {
  await page.addStyleTag({ content: "main canvas { visibility: hidden !important; }" });
}

function card(id: string, title: string, path: PageSpec["path"], extra: KeyElement[]): PageSpec[] {
  const spec: PageSpec = {
    id,
    title,
    reference: "sites",
    path,
    ready: TITLE,
    keys: [...sitesFrame("dtb"), ...CARD_KEYS, ...extra],
    setup: holdMotion,
    ...sitesFrameRules(),
    unrecorded: SITES_UNRECORDED,
  };
  return [spec, signedIn(spec, `${title}, signed in as the test wallet`)];
}

function invalid(id: string, title: string, path: string, extra: KeyElement[]): PageSpec[] {
  const spec: PageSpec = {
    id,
    title,
    reference: "sites",
    path,
    ready: mainText("Oops!"),
    keys: [
      ...sitesFrame("dtb"),
      { name: "invalid.title", selector: mainText("Oops!") },
      { name: "invalid.message", selector: { reference: "xpath=//main//h6", "dcl-one": "xpath=//main//h1/following-sibling::p[1]" } },
      ...extra,
    ],
    setup: holdMotion,
    ...sitesFrameRules(),
    unrecorded: SITES_UNRECORDED,
  };
  return [spec, signedIn(spec, `${title}, signed in as the test wallet`)];
}

const JUMP_IN: KeyElement = { name: "cta.jump-in", selector: control("JUMP IN") };

export default [
  ...card("jump", "Jump: the default place card (/jump, Genesis Plaza)", "/jump", [JUMP_IN]),
  ...card("jump-places", "Jump: a place card (/jump/places?position=1,95)", "/jump/places?position=1,95", [JUMP_IN]),
  ...card("jump-event", "Jump: the legacy event path (/jump/event?position=0,5)", "/jump/event?position=0,5", []),
  ...card("jump-events", "Jump: an event card by id (/jump/events?id=)", `/jump/events?id=${JUMP_EVENT_ID}`, [
    { name: "cta.calendar", selector: control("ADD TO CALENDAR") },
    { name: "cta.remind", selector: control("REMIND ME") },
  ]),
  ...invalid("jump-events-invalid", "Jump: an invalid event link (/jump/events/invalid)", "/jump/events/invalid", [
    // A phone gets no Jump in button on an invalid link (sites: pages/jump/InvalidPage.tsx, !isMobile).
    { name: "cta.jump-into", selector: control("JUMP INTO DECENTRALAND"), absentOn: "mobile" },
    { name: "cta.explore-events", selector: control("EXPLORE EVENTS") },
  ]),
  ...invalid("jump-places-invalid", "Jump: an invalid place link (/jump/places/invalid)", "/jump/places/invalid", [
    { name: "cta.jump-into", selector: control("JUMP INTO DECENTRALAND"), absentOn: "mobile" },
  ]),
] satisfies PageSpec[];
