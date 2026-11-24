// @vitest-environment node
// The server markup renders before the DOM exists; the DOM is installed for hydration only.
import fs from "node:fs";
import { builtinEnvironments } from "vitest/runtime";
import { act, fireEvent, waitFor, within } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { hydrateRoot, type Root } from "react-dom/client";
import { createMemoryRouter, RouterContextProvider, RouterProvider, useLoaderData } from "react-router";
import type { ComponentProps } from "react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { sessionMiddleware } from "@/core/lib/telemetry/session";
import { SESSION_STORAGE_KEY } from "@/data/lib/auth/session";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { writeDocsFixture } from "@/data/lib/docs/fixture.test-helper";
import App, { loader as rootLoader } from "../root";
import DocsRoute, { loader as docsLoader, middleware as docsMiddleware } from "./docs.$";

vi.mock("@/features/lib/dev/dev-kit", () => ({ installDevKit: () => {} }));
vi.mock("@/data/lib/auth/use-profile-name", () => ({
  useProfileIdentity: (address: string | null, connected: boolean) => ({
    name: connected && address ? "Ada" : "",
    avatarUrl: "",
    bodyUrl: "",
    avatarColor: "",
  }),
}));

const WALLET = "0x00000000000000000000000000000000000000aa";
const PATH = "/docs/player/faq/";
const identity: AuthIdentity = {
  signer: WALLET,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"1".repeat(64)}` },
  expiration: new Date(Date.now() + 86_400_000).toISOString(),
  authChain: [],
};

let dir = "";
beforeAll(() => {
  dir = writeDocsFixture();
  vi.stubEnv("DOCS_CONTENT_DIR", dir);
});
afterAll(() => {
  vi.unstubAllEnvs();
  fs.rmSync(dir, { recursive: true, force: true });
});

/** The root and docs loader data of a signed-in visitor's docs document, through the real middleware. */
async function documentData() {
  const request = new Request(`https://dcl.test${PATH}`, { headers: { cookie: `dcl_wallet=${WALLET}` } });
  const context = new RouterContextProvider();
  const args = { request, context, params: { "*": PATH.slice("/docs/".length) } };
  let root!: Awaited<ReturnType<typeof rootLoader>>;
  let page!: ReturnType<typeof docsLoader>;
  await sessionMiddleware(args as never, async () => {
    await docsMiddleware[0](args as never, async () => {
      [root, page] = await Promise.all([rootLoader(args as never), docsLoader(args as never)]);
      return new Response();
    });
    return new Response();
  });
  return { root, page };
}

function Page() {
  return <DocsRoute {...({ loaderData: useLoaderData() } as ComponentProps<typeof DocsRoute>)} />;
}

function tree(data: Awaited<ReturnType<typeof documentData>>) {
  const router = createMemoryRouter(
    [
      {
        id: "root",
        path: "/",
        Component: App,
        loader: () => data.root,
        children: [{ id: "routes/docs.$", path: "docs/*", Component: Page, loader: () => data.page }],
      },
    ],
    { initialEntries: [PATH], hydrationData: { loaderData: { root: data.root, "routes/docs.$": data.page } } },
  );
  return { router, element: <RouterProvider router={router} /> };
}

describe("when a signed-in visitor's docs document hydrates", () => {
  let dom: Awaited<ReturnType<typeof builtinEnvironments.jsdom.setup>>;
  let markup: string;
  let container: HTMLElement;
  let hydrated: Root | undefined;
  let router: ReturnType<typeof tree>["router"] | undefined;
  const recover = vi.fn();

  beforeEach(async () => {
    const data = await documentData();
    markup = renderToString(tree(data).element);
    dom = await builtinEnvironments.jsdom.setup(globalThis, { jsdom: { url: `https://dcl.test${PATH}` } });
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    vi.stubGlobal("fetch", vi.fn(async () => Response.json({})));
    vi.stubGlobal("matchMedia", (media: string) => ({ media, matches: false, addEventListener() {}, removeEventListener() {} }));
    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(identity));
    container = document.createElement("div");
    container.innerHTML = markup;
    document.body.append(container);
    const browser = tree(data);
    router = browser.router;
    await act(async () => {
      hydrated = hydrateRoot(container, browser.element, { onRecoverableError: recover });
    });
  });

  afterEach(async () => {
    if (hydrated) await act(async () => hydrated?.unmount());
    hydrated = undefined;
    router?.dispose();
    recover.mockClear();
    await dom.teardown(globalThis);
    vi.unstubAllGlobals();
  });

  it("should serve markup that names no visitor", () => {
    expect(markup).not.toContain(WALLET);
    expect(markup).not.toContain("Ada");
    expect(markup).toContain("Sign in");
  });

  it("should hydrate the page without a mismatch", () => {
    expect(recover).not.toHaveBeenCalled();
    expect(within(container).getByRole("heading", { name: "Why?" })).toBeTruthy();
  });

  it("should show the visitor's profile in the nav", async () => {
    const account = await waitFor(() => within(container).getByRole("button", { name: "Account: Ada" }));
    fireEvent.click(account);
    const profile = await waitFor(() => within(document.body).getByRole("link", { name: "View Profile" }));
    expect(profile.getAttribute("href")).toBe(`/profile/${WALLET}`);
  });
});
