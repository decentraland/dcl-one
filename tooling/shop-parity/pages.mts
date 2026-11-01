// The pages a parity capture compares. Each runs on both sides with the same recording, path,
// viewport and interactions. Key elements name what matters on the page; most pair by testid
// (ui3 kept upstream's data-testid values), a few need a selector per side. Masks hide only what
// cannot be equal between two renders (3D previews, live timers) and say why; expected entries
// are deliberate differences, each with the rule (the repository's AGENTS.md) or code behind it.
import type { Page } from "playwright-core";
import type { Expectation } from "./lib/structure.mts";


export type Side = "reference" | "dcl-one";
export type PerSide<T> = { reference: T; "dcl-one": T };

export type KeyElement = { name: string; selector: string | PerSide<string>; max?: number };
/** What cannot be equal between two renders, in a box both sides have at the same place: a pixel only one side masks still counts. */
export type Mask = { selector: PerSide<string[]>; why: string };
/** A deliberate difference's area: excluded from the pixel count, listed with its rule. */
export type Region = { selector: PerSide<string[]>; why: string };

export type PageSpec = {
  id: string;
  title: string;
  /** The same path on both sides; a function reads targets discovered while recording. */
  path: string | ((targets: Record<string, string>) => string);
  /** Visible on each side once the page has rendered its content. */
  ready: PerSide<string>;
  /** Interactions after the first render (both sides do the same thing). */
  setup?: (page: Page, side: Side, targets: Record<string, string>) => Promise<void>;
  /** While recording: read targets later pages need (from the reference). */
  discover?: (page: Page) => Promise<Record<string, string>>;
  keys: KeyElement[];
  masks?: Mask[];
  regions?: Region[];
  /**
   * Compare only the top of the page, in CSS px: an infinitely scrolling list is captured to a depth
   * that never reaches its next-page trigger, so both sides show the same first page.
   */
  maxHeight?: number;
  expected?: Expectation[];
  /** Requests a page makes that the recording cannot hold, matched on `METHOD service path body`, with why. */
  unrecorded?: { match: RegExp; why: string }[];
};

const both = (s: string): PerSide<string> => ({ reference: s, "dcl-one": s });
const both1 = (s: string): PerSide<string[]> => ({ reference: [s], "dcl-one": [s] });

const UP = "translate(normalize-space(),'abcdefghijklmnopqrstuvwxyz','ABCDEFGHIJKLMNOPQRSTUVWXYZ')";
/** An element by its visible words, case-insensitively (the same words on both sides). */
const byText = (tag: string, text: string, scope = "//main") => `xpath=${scope}//${tag}[${UP}='${text.toUpperCase()}']`;
/** The nearest ancestor box of a text that is at least `minChildren` wide, e.g. a panel by its heading. */
const panelOf = (text: string) => `xpath=//main//*[${UP}='${text.toUpperCase()}' and not(*)]/ancestor::*[count(*)>=2][2]`;

/** The frame every Shop page shares: site nav, Shop subnav, footer. */
const FRAME_KEYS: KeyElement[] = [
  { name: "site-nav", selector: { reference: "xpath=//nav[.//*[normalize-space(text())='Discover']]", "dcl-one": "header.dtb" } },
  { name: "site-nav.signin", selector: { reference: "xpath=//nav[.//*[normalize-space(text())='Discover']]//*[self::a or self::button][normalize-space()='SIGN IN' or normalize-space()='Sign In' or normalize-space()='Sign in']", "dcl-one": "header.dtb .dtb__signin" } },
  { name: "subnav", selector: both("[data-testid=subnav]") },
  { name: "subnav.tabs", selector: both("[data-testid=subnav-tabs]") },
  { name: "subnav.tab", selector: both("[data-testid=subnav-tabs] a"), max: 4 },
  { name: "subnav.search", selector: { reference: "input[aria-label='Search the shop']", "dcl-one": "[data-testid=search-input]" } },
  { name: "subnav.credits", selector: both("[data-testid=subnav] a[href*=credits]") },
  { name: "subnav.favorites", selector: both("[data-testid=subnav] a[aria-label='My Favorites']") },
  { name: "subnav.cart", selector: both("[data-testid=subnav-cart]") },
  { name: "footer", selector: both("footer") },
];

/** Every page: WebGL canvases are 3D renders (product previews, the promos' live preview). */
const WEBGL: Mask = {
  selector: { reference: ["canvas"], "dcl-one": ["canvas"] },
  why: "WebGL canvas (3D preview): its pixels differ between frames and renderers; the box it sits in is still compared structurally",
};

const NAV_SECTIONS = "the site nav links dcl-one's own sections: the landing page, What's On, /play, /shop, /create and /docs are one site (AGENTS.md, Scope)";

const CONFIRMATION_GROUPS =
  "a cart shows explicit confirmation groups, one per payment method, chain and settlement contract (checkout/components/CheckoutGroups.tsx), because Credits, MANA and the MANA-only sales settle separately (AGENTS.md: Credits are the main payment option; LAND and every secondary sale are MANA-only)";

const NO_MODELS =
  "3D models are not recorded (pictures are placeholders), so ui3's renderer shows its fallback notice inside the masked preview; the reference's preview iframe renders nothing there";

const PAYMENT_PANEL =
  "the price block is ui3's payment panel: the server's offer decides the methods shown (AGENTS.md: Credits are the main payment option, logged-in web3 wallets also see MANA, secondary sales are MANA-only with guidance for web2 users)";

/** The base avatar ui3's 3D preview dresses: its wearables' entities are not recorded and the preview is masked. */
const PREVIEW_AVATAR = { match: /^POST peer \/content\/entities\/active .*urn:decentraland:off-chain:base-avatars:/, why: NO_MODELS };

/** The overview's only entity lookups: the promos' live preview, which starts once a promo is on screen (or not at all). */
const PROMO_PREVIEW = {
  match: /^POST peer \/content\/entities\/active /,
  why: "the promos' live 3D preview (discovery/components/LivePromo.tsx, masked) looks up its avatar and wearables only when it starts, which depends on timing; their entities are not recorded",
};

const OUTFIT_ORDER =
  "the reference shuffles Buy the Look on every visit within recency buckets (components/OutfitsRow/OutfitsRow.tsx, lib/outfits.ts orderOutfitsByRecency), so its order is not a property to match; the row's box and cards are still compared";

const CREATOR_TAG =
  "every mint line says it is bought from its creator: one Shop sells mints and resales side by side (AGENTS.md: the Shop absorbs every Marketplace feature), where the reference shows the chip only while its secondary-sales flag is on, and that flag is off in the recording";

const NEWSLETTER = "the third-party beehiiv newsletter iframe is replaced by a first-party form of the same box (discovery/frames/ShopFooter.tsx, posting to /v1/newsletter)";

/** The reference's footer loads that iframe from beehiiv, a host the harness does not record. */
const NEWSLETTER_EMBED = { match: /^GET embeds\.beehiiv\.com\//, why: NEWSLETTER };

/** Only the section links: the bar's height, brand, sign-in button and colours are still compared. */
const FRAME_REGIONS: Region[] = [
  { selector: { reference: ["xpath=//nav[.//*[normalize-space(text())='Discover']]//*[normalize-space(text())='Discover']/ancestor::div[count(*)>=4][1]"], "dcl-one": ["header.dtb nav.dtb__links"] }, why: NAV_SECTIONS },
  { selector: { reference: ["footer iframe"], "dcl-one": ["[data-testid=shop-footer-newsletter]"] }, why: NEWSLETTER },
];

const TEST_HOOKS = "ui3's unit tests find these elements by test ids the reference does not carry (ShopChrome, SearchDropdown, Items and CartPage tests); the elements themselves are compared by their key, testid-free path or text";

const FRAME_EXPECTED: Expectation[] = [
  { match: /^"(Discover|Play|Social|Create|Learn|Marketplace|Builder|Events|Places)"(#\d+)?$/, why: NAV_SECTIONS },
  { match: /^"Shop"$/, why: NAV_SECTIONS, only: ["left", "width"] },
  { match: /^key:site-nav$/, why: NAV_SECTIONS, only: ["text"] },
  { match: /(^|\/)(search|search-input|sort-select|total-credits|confirm-checkout)$/, status: "only-dcl-one", why: TEST_HOOKS },
  { match: /newsletter|^"Enter your email"$|^"SUBSCRIBE"$/i, why: NEWSLETTER },
  { match: /^key:footer$/, why: NEWSLETTER, only: ["text"] },
];

/** A rail's section, by testid where both have it, else by its heading's text. */
const rail = (testid: string, heading: string): PerSide<string> => ({
  reference: `[data-testid=${testid}] || xpath=//section[.//h2[normalize-space()='${heading}']]`,
  "dcl-one": `[data-testid=${testid}] || xpath=//section[.//h2[normalize-space()='${heading}']]`,
});
const within = (scope: PerSide<string>, inner: string): PerSide<string> => ({
  reference: scope.reference.split(" || ").map((s) => (s.startsWith("xpath=") ? `${s}//*[${inner}]` : `${s} [${inner.replace(/^@/, "").replace(/=/, "=")}]`)).join(" || "),
  "dcl-one": scope["dcl-one"].split(" || ").map((s) => (s.startsWith("xpath=") ? `${s}//*[${inner}]` : `${s} [${inner.replace(/^@/, "").replace(/=/, "=")}]`)).join(" || "),
});

const CARD_KEYS = (name: string, scope: PerSide<string>): KeyElement[] => [
  { name, selector: scope },
  { name: `${name}.title`, selector: { reference: scope.reference.split(" || ").map((s) => (s.startsWith("xpath=") ? `${s}//h2` : `${s} h2`)).join(" || "), "dcl-one": scope["dcl-one"].split(" || ").map((s) => (s.startsWith("xpath=") ? `${s}//h2` : `${s} h2`)).join(" || ") } },
  { name: `${name}.card`, selector: within(scope, "@data-testid='card'"), max: 5 },
  { name: `${name}.card.name`, selector: within(scope, "@data-testid='card-name'"), max: 5 },
  { name: `${name}.card.price`, selector: within(scope, "@data-testid='card-price'"), max: 5 },
  { name: `${name}.card.author`, selector: within(scope, "@data-testid='card-author'"), max: 5 },
];

export const PAGES: PageSpec[] = [
  {
    id: "overview",
    title: "Shop home",
    path: "/shop/overview",
    ready: both("[data-testid=hero-title]"),
    keys: [
      ...FRAME_KEYS,
      { name: "hero", selector: both("xpath=//section[.//*[@data-testid='hero-title']]") },
      { name: "hero.title", selector: both("[data-testid=hero-title]") },
      { name: "hero.cta", selector: both("[data-testid=hero-credits-cta]") },
      ...CARD_KEYS("trending", rail("trending-rail", "Trending Products")),
      { name: "outfits", selector: both("[data-testid=outfits-row]") },
      { name: "outfits.title", selector: both("[data-testid=outfits-row] h2") },
      { name: "outfits.card", selector: both("[data-testid=outfit-card]"), max: 6 },
      ...CARD_KEYS("new-creations", rail("new-creations-rail", "New Creations")),
      { name: "promos", selector: { reference: "xpath=//section[.//a[contains(@href,'items?category=emote')]][not(.//h2)]", "dcl-one": ".shop-overview__promos" } },
      { name: "top-creators", selector: both("[data-testid=top-creators]") },
      { name: "top-creators.card", selector: both("[data-testid=top-creator-card]"), max: 4 },
    ],
    masks: [WEBGL],
    regions: [...FRAME_REGIONS, { selector: both1("[data-testid=outfits-row-track]"), why: OUTFIT_ORDER }],
    expected: [...FRAME_EXPECTED, { match: /^key:outfits$/, why: OUTFIT_ORDER, only: ["text"] }],
    unrecorded: [PROMO_PREVIEW, NEWSLETTER_EMBED],
  },
  {
    id: "items",
    title: "Items list, filtered (wearables, epic)",
    path: "/shop/items?category=wearable&rarities=epic",
    ready: { reference: "[data-testid=grid] [data-testid=card]", "dcl-one": "[data-testid=grid] > *" },
    keys: [
      ...FRAME_KEYS,
      { name: "filters", selector: { reference: "aside, [data-testid=filters]", "dcl-one": "aside, [data-testid=filters]" } },
      { name: "filters.category", selector: { reference: "xpath=(//aside//button[@data-cat])[1]/../..", "dcl-one": "[data-testid=category-filter]" } },
      { name: "filters.rarity", selector: both("[data-testid=rarity-filter]") },
      { name: "toolbar", selector: { reference: "[data-testid=browse-toolbar], [data-testid=toolbar]", "dcl-one": "[data-testid=browse-toolbar]" } },
      { name: "grid", selector: both("[data-testid=grid]") },
      { name: "grid.card", selector: { reference: "[data-testid=grid] [data-testid=card]", "dcl-one": "[data-testid=grid] > *" }, max: 8 },
      { name: "grid.card.name", selector: both("[data-testid=grid] [data-testid=card-name]"), max: 8 },
      { name: "grid.card.price", selector: both("[data-testid=grid] [data-testid=card-price]"), max: 8 },
    ],
    discover: async (page) => {
      const href = await page.locator("[data-testid=grid] [data-testid=card] a[data-testid=card-link]").first().getAttribute("href");
      if (!href) throw new Error("no card in the reference items grid to take as the detail page");
      return { item: new URL(href, page.url()).pathname.replace(/^(?!\/shop)/, "/shop") };
    },
    maxHeight: 3000,
    masks: [WEBGL],
    regions: FRAME_REGIONS,
    expected: FRAME_EXPECTED,
    unrecorded: [NEWSLETTER_EMBED],
  },
  {
    id: "item",
    title: "Item detail (first card of the filtered list)",
    path: (t) => t.item ?? "/shop/items",
    ready: both("[data-testid=item-price]"),
    keys: [
      ...FRAME_KEYS,
      { name: "breadcrumb", selector: both("[aria-label=Breadcrumb]") },
      { name: "preview", selector: both("[data-testid=item-preview]") },
      { name: "info", selector: both("[data-testid=item-info]") },
      { name: "title", selector: both("[data-testid=item-info] h1") },
      { name: "favorite", selector: both("[data-testid=fav-count]") },
      { name: "tag.rarity", selector: both("[data-testid=detail-rarity-link]") },
      { name: "tag.category", selector: both("[data-testid=detail-category-link]") },
      { name: "tag.play-mode", selector: both("[data-testid=detail-play-mode]") },
      { name: "tag.export", selector: both("[data-testid=detail-export-blocked]") },
      { name: "creator", selector: both("[data-testid=creator]") },
      { name: "price", selector: both("[data-testid=item-price]") },
      { name: "stock", selector: { reference: "xpath=//main//*[" + UP + "='STOCK' and not(*)]/..", "dcl-one": "[data-testid=detail-stock]" } },
      { name: "buy-now", selector: both(byText("button", "Buy now")) },
      { name: "add-to-cart", selector: both(byText("button", "Add to cart")) },
      { name: "rail", selector: { reference: "[data-testid=carousel]", "dcl-one": "[data-testid=detail-rail]" } },
      { name: "rail.title", selector: { reference: "[data-testid=carousel] h2", "dcl-one": "[data-testid=detail-rail] h2" } },
      { name: "rail.card", selector: { reference: "[data-testid=carousel] [data-testid=card]", "dcl-one": "[data-testid=detail-rail] [data-testid=card]" }, max: 5 },
      { name: "rail.card.price", selector: { reference: "[data-testid=carousel] [data-testid=card-price]", "dcl-one": "[data-testid=detail-rail] [data-testid=card-price]" }, max: 5 },
    ],
    masks: [
      {
        selector: { reference: ["[data-testid=item-preview]"], "dcl-one": ["[data-testid=item-preview]"] },
        why: "3D preview: a WebGL render (the reference's wearable-preview iframe, ui3's own renderer) that differs between frames and renderers; its box is still compared structurally",
      },
      WEBGL,
    ],
    regions: FRAME_REGIONS,
    expected: [
      ...FRAME_EXPECTED,
      { match: /(^|\/)(payment-panel|cta-sign-in|cta-add-to-cart|price-credits|detail-stock)$/, status: "only-dcl-one", why: PAYMENT_PANEL },
      { match: /^"Preview unavailable for this item"$/, status: "only-dcl-one", why: NO_MODELS },
      { match: /^key:preview$/, why: NO_MODELS, only: ["text"] },
    ],
    unrecorded: [PREVIEW_AVATAR, NEWSLETTER_EMBED],
  },
  {
    id: "cart",
    title: "Cart with the detail page's item, signed out",
    path: "/shop/cart",
    ready: both(byText("*", "Purchase Summary")),
    setup: async (page, _side, targets) => {
      // Add the detail page's item through the page itself, then open the cart.
      await page.goto(new URL(targets.item ?? "/shop/items", page.url()).href, { waitUntil: "load" });
      const add = page.locator(byText("button", "Add to cart"));
      await add.first().waitFor({ state: "visible", timeout: 20_000 });
      await add.first().click();
      await page.waitForTimeout(500);
      await page.goto(new URL("/shop/cart", page.url()).href, { waitUntil: "load" });
    },
    keys: [
      ...FRAME_KEYS,
      { name: "lines", selector: both(panelOf("Fitting room")) },
      { name: "lines.count", selector: both("xpath=//main//*[starts-with(normalize-space(),'Cart:') and not(*)]") },
      { name: "lines.fitting-room", selector: both(byText("button", "Fitting room")) },
      { name: "line.name", selector: both("xpath=//main//*[normalize-space()='Semi Visible Man RGB' and not(*)]") },
      { name: "line.price", selector: { reference: "xpath=//main//*[normalize-space(text())='Semi Visible Man RGB']/ancestor::*[count(.//button)>=3][1]//span[normalize-space(text())='12']/..", "dcl-one": "[data-testid=cart-line-price]" } },
      { name: "lines.clear", selector: both(byText("*", "Clear cart")) },
      { name: "summary", selector: both(panelOf("Purchase Summary")) },
      { name: "summary.title", selector: both(byText("*", "Purchase Summary")) },
      { name: "summary.total", selector: both("xpath=//main//*[starts-with(normalize-space(),'Total:') and not(*)]/..") },
      { name: "summary.checkout", selector: both(byText("button", "Checkout")) },
      { name: "rail", selector: both("[data-testid=carousel]") },
      { name: "rail.card", selector: both("[data-testid=carousel] [data-testid=card]"), max: 5 },
    ],
    masks: [WEBGL],
    regions: [...FRAME_REGIONS, { selector: { reference: [], "dcl-one": ["[data-testid=checkout-group]"] }, why: CONFIRMATION_GROUPS }],
    expected: [
      ...FRAME_EXPECTED,
      { match: /checkout-group|^"(Pay with Credits|Pay with MANA|Ready to confirm)"/, why: CONFIRMATION_GROUPS },
      { match: /(^|\/)cart-creator-tag$|^"Creator"$/, status: "only-dcl-one", why: CREATOR_TAG },
    ],
    unrecorded: [PREVIEW_AVATAR, NEWSLETTER_EMBED],
  },
];

export function pagePath(spec: PageSpec, targets: Record<string, string>): string {
  return typeof spec.path === "function" ? spec.path(targets) : spec.path;
}
