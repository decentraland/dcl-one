// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ChromeAuthContext } from "@dcl/one-ui";
import { createShopClient } from "@shop/api/shop";
import { CREDITS_BALANCE_QUERY_KEY } from "@shop/credits/balance-query";
import { sitesShopClient } from "@/data/lib/auth/shop-client";

import SitesLayout from "./SitesLayout";

vi.mock("@/data/lib/auth/shop-client", () => ({ sitesShopClient: vi.fn() }));
vi.mock("@/data/lib/shop/marketplace-access", () => ({ useMarketplaceHere: () => false }));

vi.hoisted(() => Object.assign(window, {
  __vite_plugin_react_preamble_installed__: true,
  $RefreshSig$: () => (type: unknown) => type,
  $RefreshReg$: () => {},
}));

afterEach(() => {
  cleanup();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

const account = "0x000000000000000000000000000000000000abcd";

describe.each([
  ["build configuration", "https://configured.example.org/", "https://public.example.org/", "configured.example.org"],
  ["host configuration", undefined, "https://public.example.org:8443/", "public.example.org:8443"],
  ["the document origin", undefined, undefined, "preview.example.org:5222"],
] as const)("when the newsletter source comes from %s", (_mode, buildBase, hostBase, source) => {
  const email = "newsletter@example.org";
  const fetchNewsletter = vi.fn<typeof fetch>();
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    user = userEvent.setup();
    fetchNewsletter.mockReset();
    fetchNewsletter.mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchNewsletter);
    vi.stubEnv("VITE_SITE_URL", buildBase);
    vi.stubGlobal("__SITE_BASE__", hostBase);
    vi.stubGlobal("location", {
      origin: "https://preview.example.org:5222",
      host: "preview.example.org:5222",
    });

    render(
      <MemoryRouter initialEntries={["/events"]}>
        <QueryClientProvider client={new QueryClient()}>
          <SitesLayout>Events</SitesLayout>
        </QueryClientProvider>
      </MemoryRouter>,
    );
  });

  it("should send the resolved public host in the POST body", async () => {
    const footer = within(screen.getByRole("contentinfo"));
    await user.type(footer.getByRole("textbox", { name: "Email" }), email);
    await user.click(footer.getByRole("button", { name: "Subscribe" }));

    expect(fetchNewsletter).toHaveBeenCalledExactlyOnceWith("/v1/newsletter", expect.objectContaining({
      method: "POST",
      body: JSON.stringify({ email, source }),
    }));
  });
});

describe("when Sites renders the credits chip", () => {
  let queryClient: QueryClient;
  const transport = vi.fn();

  beforeEach(() => {
    queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    transport.mockReset();
    vi.mocked(sitesShopClient).mockReturnValue(createShopClient({
      base: "https://sites.test",
      mode: "canonical",
      signer: { address: account, sign: async () => ({}) },
      transport,
    }));
  });

  function show(signedIn: boolean) {
    render(
      <MemoryRouter initialEntries={["/events"]}>
        <QueryClientProvider client={queryClient}>
          <ChromeAuthContext value={{ signedIn, account: signedIn ? account : "", mana: "", name: "", committee: false }}>
            <SitesLayout>Events</SitesLayout>
          </ChromeAuthContext>
        </QueryClientProvider>
      </MemoryRouter>,
    );
  }

  it("should skip balance requests for a visitor", () => {
    show(false);

    expect(transport).not.toHaveBeenCalled();
  });

  it("should show the signed-in balance and share the Shop account key", async () => {
    transport.mockResolvedValueOnce({ status: 200, body: JSON.stringify({ usd: { balanceCents: 12340 } }) });
    show(true);

    expect((await screen.findByRole("link", { name: "1,234 credits" })).getAttribute("href")).toBe("/shop/credits");
    expect(queryClient.getQueryData([CREDITS_BALANCE_QUERY_KEY, account])).toMatchObject({ credits: 1234 });
  });

  it("should hide the balance when the upstream request fails", async () => {
    transport.mockResolvedValueOnce({ status: 503, body: "{}" });
    show(true);

    await waitFor(() => expect(queryClient.getQueryState([CREDITS_BALANCE_QUERY_KEY, account])?.status).toBe("error"));
    expect(screen.queryByRole("link", { name: /credits/i })).toBeNull();
  });
});
