// @vitest-environment jsdom
import "@/features/locale/test-setup";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { useState, type ReactNode } from "react";
import { renderToString } from "react-dom/server";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createMemoryRouter,
  createRoutesStub,
  RouterProvider,
  createStaticHandler,
  createStaticRouter,
  StaticRouterProvider,
  useLoaderData,
} from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ChromeAuthContext, type ChromeAuth } from "@ui/web/frames/chrome-auth";

import { resolveLegalKey } from "@/data/legal";
import { loadLocaleMessages } from "@/features/locale/catalogs";
import {
  canonicalLegalTree,
  withoutCurrentMarks,
  type CanonicalNode,
} from "@/features/test-utils/legal-canonical";

const ACCOUNT = "0x1111111111111111111111111111111111112222";

vi.mock("@/data/lib/auth/index", () => ({
  useAuth: () => ({
    isConnected: false,
    address: undefined,
    fetch: async () => new Response("[]"),
    disconnect: vi.fn(),
  }),
}));
vi.mock("@ui/shop/credits/hooks", () => ({
  useBalance: () => ({ data: undefined, isError: false }),
}));
vi.mock("../shop/host", () => ({ installSitesShopClient: () => undefined }));
const openSignIn = vi.fn();
vi.mock("@/features/components/auth/signin-store", () => ({
  openSignIn: (...args: unknown[]) => openSignIn(...args),
}));
const track = vi.fn();
vi.mock("@/core/lib/telemetry/track", () => ({
  track: (...args: unknown[]) => track(...args),
}));

import * as legalRoute from "./$slug";
import type { Route } from "./+types/$slug";

const FIXTURES = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../data/legal/fixtures",
);
const fixture = (slug: string) =>
  (
    JSON.parse(
      fs.readFileSync(path.join(FIXTURES, `${slug}.json`), "utf8"),
    ) as { canonical: CanonicalNode[] }
  ).canonical;

// Each document: its key, the path production serves it at, and production's page title.
const DOCS = [
  ["brand", "brand", "Brand Guidelines | Decentraland"],
  ["content", "content", "Content Policy | Decentraland"],
  ["ethics", "ethics", "Code of Ethics | Decentraland"],
  ["rewards", "rewards-terms", "Rewards Terms | Decentraland"],
  ["credits", "credits-terms", "Decentraland"],
  ["security", "security", "Security | Decentraland"],
  ["privacy", "privacy", "Privacy Policy | Decentraland"],
  ["referral", "referral-terms", "Referral Program Terms | Decentraland"],
  ["terms", "terms", "Terms of Use | Decentraland"],
] as const;

/** The shared chrome auth as ChromeAuthBridge provides it, with a sign out that ends the session. */
function Session({
  initial,
  children,
}: {
  initial: boolean;
  children: ReactNode;
}) {
  const [signedIn, setSignedIn] = useState(initial);
  const value: ChromeAuth = {
    signedIn,
    account: signedIn ? ACCOUNT : "",
    mana: "",
    name: "Alice",
    committee: false,
    onSignIn: vi.fn(),
    onSignOut: () => setSignedIn(false),
  };
  return (
    <ChromeAuthContext.Provider value={value}>
      {children}
    </ChromeAuthContext.Provider>
  );
}

function Providers({
  signedIn = false,
  children,
}: {
  signedIn?: boolean;
  children: ReactNode;
}) {
  const [queryClient] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  );
  return (
    <QueryClientProvider client={queryClient}>
      <Session initial={signedIn}>{children}</Session>
    </QueryClientProvider>
  );
}

// The route's loader data for a path. The route module's loader is server code the React Router
// plugin leaves out of this browser test; $slug.loader.test.ts covers it.
async function testLoader({ params }: { params: { slug?: string } }) {
  const doc = resolveLegalKey(params.slug ?? "");
  if (!doc) throw new Response(null, { status: 404 });
  return { doc, sid: "sid-1", messages: await loadLocaleMessages("en") };
}

// The route module's component as the framework mounts it, with the loader's data as a prop.
function Page() {
  return (
    <legalRoute.default
      {...({ loaderData: useLoaderData() } as unknown as Route.ComponentProps)}
    />
  );
}

/** The server render of a legal path, parsed back into a DOM. */
async function serverRender(
  pathname: string,
  headers: Record<string, string> = {},
) {
  const handler = createStaticHandler([
    { path: "/:slug", loader: testLoader, Component: Page },
  ]);
  const context = await handler.query(
    new Request(`http://localhost${pathname}`, { headers }),
  );
  if (context instanceof Response)
    throw new Error(`unexpected response ${context.status}`);
  const router = createStaticRouter(handler.dataRoutes, context);
  const html = renderToString(
    <Providers>
      <StaticRouterProvider router={router} context={context} hydrate={false} />
    </Providers>,
  );
  const container = document.createElement("div");
  container.innerHTML = html;
  return container;
}

/** The client render of a legal path in a data router, as after hydration. */
function clientRender(pathname: string, { signedIn = false } = {}) {
  const Stub = createRoutesStub([
    { path: "/:slug", Component: legalRoute.default, loader: testLoader },
  ]);
  return render(
    <Providers signedIn={signedIn}>
      <Stub initialEntries={[pathname]} />
    </Providers>,
  );
}

/** Whether a rendered DOM equals a document's frozen render, besides our aria-current mark. */
function matchesFixture(root: ParentNode, slug: string) {
  return isDeepStrictEqual(
    withoutCurrentMarks(canonicalLegalTree(root), `/${slug}`),
    fixture(slug),
  );
}

beforeEach(() => {
  Object.defineProperty(window, "innerWidth", {
    value: 1440,
    configurable: true,
  });
  track.mockClear();
  openSignIn.mockClear();
});

describe("the legal documents", () => {
  describe.each(DOCS)("when %s is server rendered", (_key, slug) => {
    it("should equal the frozen render of the source document, marking only its own sidebar entry", async () => {
      const container = await serverRender(`/${slug}`);
      // Exactly the source render, plus our one aria-current="page" on this document's entry.
      expect(
        withoutCurrentMarks(canonicalLegalTree(container), `/${slug}`),
      ).toEqual(fixture(slug));
      expect(container.querySelectorAll("h1")).toHaveLength(1);
    });
  });

  describe.each(DOCS)("when %s is rendered in the browser", (_key, slug) => {
    it("should equal the frozen render of the source document", async () => {
      const { container } = clientRender(`/${slug}`);
      await screen.findByRole("heading", { level: 1 });
      // Exactly the source render, plus our one aria-current="page" on this document's entry.
      expect(
        withoutCurrentMarks(canonicalLegalTree(container), `/${slug}`),
      ).toEqual(fixture(slug));
    });
  });

  describe("when the page is /security", () => {
    it("should offer the report as a mailto link, not a button", async () => {
      const container = await serverRender("/security");
      const cta = [...container.querySelectorAll("a")].find(
        (a) => a.textContent === "Report a security issue",
      );

      expect(cta?.getAttribute("href")).toBe(
        "mailto:security@decentraland.org",
      );
      expect(container.querySelector("article button")).toBeNull();
    });
  });
});

// The comparison must fail on each kind of drift it exists to catch.
describe("the frozen-render comparison", () => {
  const firstText = (root: ParentNode, pattern: RegExp) => {
    const walker = document.createTreeWalker(
      root.querySelector("article")!,
      NodeFilter.SHOW_TEXT,
    );
    for (let node = walker.nextNode(); node; node = walker.nextNode())
      if (pattern.test(node.nodeValue ?? "")) return node;
    throw new Error(`no text matching ${pattern}`);
  };

  it("should accept an unchanged render", async () => {
    expect(matchesFixture(await serverRender("/ethics"), "ethics")).toBe(true);
  });

  it("should reject a non-breaking space turned into a space", async () => {
    const container = await serverRender("/terms");
    const node = firstText(container, / /);
    node.nodeValue = node.nodeValue!.replace(" ", " ");

    expect(matchesFixture(container, "terms")).toBe(false);
  });

  it("should reject a space removed before a link, joining two words", async () => {
    const container = await serverRender("/terms");
    const link = [...container.querySelectorAll("article p a")].find((a) =>
      a.previousSibling?.nodeValue?.endsWith(" "),
    )!;
    link.previousSibling!.nodeValue = link.previousSibling!.nodeValue!.replace(
      / $/,
      "",
    );

    expect(matchesFixture(container, "terms")).toBe(false);
  });

  it("should reject a space removed before a bold run, joining two words", async () => {
    const container = await serverRender("/terms");
    const bold = [...container.querySelectorAll("article strong")].find((b) =>
      b.previousSibling?.nodeValue?.endsWith(" "),
    )!;
    bold.previousSibling!.nodeValue = bold.previousSibling!.nodeValue!.replace(
      / $/,
      "",
    );

    expect(matchesFixture(container, "terms")).toBe(false);
  });

  it("should reject a curly apostrophe made straight", async () => {
    const container = await serverRender("/ethics");
    const node = firstText(container, /’/);
    node.nodeValue = node.nodeValue!.replace("’", "'");

    expect(matchesFixture(container, "ethics")).toBe(false);
  });

  it("should reject an en dash made a hyphen", async () => {
    const container = await serverRender("/ethics");
    const node = firstText(container, /–/);
    node.nodeValue = node.nodeValue!.replace("–", "-");

    expect(matchesFixture(container, "ethics")).toBe(false);
  });

  it("should reject a changed href", async () => {
    const container = await serverRender("/privacy");
    container
      .querySelector('article a[href^="mailto:"]')!
      .setAttribute("href", "mailto:jane.doe@example.com");

    expect(matchesFixture(container, "privacy")).toBe(false);
  });

  it("should reject a dropped target", async () => {
    const container = await serverRender("/terms");
    container
      .querySelector('article a[target="_blank"]')!
      .removeAttribute("target");

    expect(matchesFixture(container, "terms")).toBe(false);
  });

  it("should reject a dropped rel", async () => {
    const container = await serverRender("/terms");
    container
      .querySelector('article a[rel="noopener noreferrer"]')!
      .removeAttribute("rel");

    expect(matchesFixture(container, "terms")).toBe(false);
  });

  it("should reject an h3 turned into an h2", async () => {
    const container = await serverRender("/ethics");
    const h3 = container.querySelector("article h3")!;
    const h2 = document.createElement("h2");
    h2.id = h3.id;
    h2.append(...h3.childNodes);
    h3.replaceWith(h2);

    expect(matchesFixture(container, "ethics")).toBe(false);
  });

  it("should reject two list items merged", async () => {
    const container = await serverRender("/rewards-terms");
    const [first, second] = container.querySelectorAll(
      "article section ul > li",
    );
    first!.append(...second!.childNodes);
    second!.remove();

    expect(matchesFixture(container, "rewards-terms")).toBe(false);
  });

  it("should reject a changed anchor id", async () => {
    const container = await serverRender("/privacy");
    container.querySelector("article section[id]")!.id = "changed";

    expect(matchesFixture(container, "privacy")).toBe(false);
  });

  it("should reject a reordered sidebar", async () => {
    const container = await serverRender("/content");
    const [first, second] = container.querySelectorAll("nav a[href^='/']");
    first!.before(second!);

    expect(matchesFixture(container, "content")).toBe(false);
  });
});

describe("the legal page header", () => {
  it("should show the generated logo and Sign In when signed out", async () => {
    clientRender("/privacy");

    expect(
      await screen.findByRole("button", { name: "Sign In" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Decentraland" }),
    ).toBeInTheDocument();
  });

  it("should sign in back to the same page and report the click", async () => {
    clientRender("/privacy");
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign In" }),
    );

    expect(openSignIn).toHaveBeenCalledWith({ redirectTo: "/privacy" });
    expect(track).toHaveBeenCalledWith(
      "Click",
      expect.objectContaining({ place: "Landing Navbar", action: "sign_in" }),
      { sid: "sid-1", story: "misc/legal" },
    );
  });

  it("should show the account when signed in, and Sign In again after signing out without a reload", async () => {
    clientRender("/privacy", { signedIn: true });
    const userMenu = await screen.findByRole("button", { name: "Profile" });
    expect(
      screen.queryByRole("button", { name: "Sign In" }),
    ).not.toBeInTheDocument();

    await userEvent.click(userMenu);
    await userEvent.click(screen.getByRole("button", { name: "Logout" }));

    expect(
      await screen.findByRole("button", { name: "Sign In" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Profile" }),
    ).not.toBeInTheDocument();
  });
});

describe("the legal telemetry", () => {
  const viewed = () =>
    track.mock.calls.filter(([event]) => event === "lp_legal_viewed");

  describe.each(DOCS)("when %s is opened", (key, slug) => {
    it("should report one view of it", async () => {
      clientRender(`/${slug}`);
      await screen.findByRole("heading", { level: 1 });

      await waitFor(() => expect(viewed()).toHaveLength(1));
      expect(viewed()[0]).toEqual([
        "lp_legal_viewed",
        { doc: key },
        { sid: "sid-1", story: "misc/legal" },
      ]);
    });
  });

  it("should report a view of each document entered through the sidebar", async () => {
    clientRender("/terms");
    await screen.findByRole("heading", { level: 1, name: "Terms of Use" });
    const sidebar = () =>
      within(screen.getByRole("navigation", { name: "Legal documents" }));
    await userEvent.click(
      sidebar().getByRole("link", { name: "Privacy Policy" }),
    );
    await screen.findByRole("heading", { level: 1, name: "Privacy Policy" });
    await userEvent.click(
      sidebar().getByRole("link", { name: "Terms of Use" }),
    );
    await screen.findByRole("heading", { level: 1, name: "Terms of Use" });

    const context = { sid: "sid-1", story: "misc/legal" };
    await waitFor(() =>
      expect(viewed()).toEqual([
        ["lp_legal_viewed", { doc: "terms" }, context],
        ["lp_legal_viewed", { doc: "privacy" }, context],
        ["lp_legal_viewed", { doc: "terms" }, context],
      ]),
    );
  });

  it("should report a view under the canonical key when an alias is opened", async () => {
    clientRender("/content-policy");
    await screen.findByRole("heading", { level: 1, name: "Content Policy" });

    await waitFor(() =>
      expect(viewed().map(([, props]) => props)).toEqual([{ doc: "content" }]),
    );
  });

  it("should not report another view when the same document's data is revalidated", async () => {
    const router = createMemoryRouter(
      [{ path: "/:slug", Component: Page, loader: testLoader }],
      { initialEntries: ["/privacy"] },
    );
    render(
      <Providers>
        <RouterProvider router={router} />
      </Providers>,
    );
    await screen.findByRole("heading", { level: 1, name: "Privacy Policy" });
    await waitFor(() => expect(viewed()).toHaveLength(1));

    await act(() => router.revalidate());
    await act(() => router.navigate("/privacy#cookies"));

    expect(viewed()).toHaveLength(1);
  });

  it("should report each contents link followed, repeats included", async () => {
    clientRender("/privacy");
    const link = await screen.findByRole("link", {
      name: "2. Information Collected",
    });
    await act(async () => link.click());
    await act(async () => link.click());

    const clicks = track.mock.calls.filter(
      ([event]) => event === "lp_legal_section_clicked",
    );
    expect(clicks).toEqual([
      [
        "lp_legal_section_clicked",
        { doc: "privacy", section: "information-collected" },
        { sid: "sid-1", story: "misc/legal" },
      ],
      [
        "lp_legal_section_clicked",
        { doc: "privacy", section: "information-collected" },
        { sid: "sid-1", story: "misc/legal" },
      ],
    ]);
  });
});
