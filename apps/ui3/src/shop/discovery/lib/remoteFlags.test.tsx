// Upstream's rollout flags, read from the same feature-flags file: what each answer means, how long it is
// held, and that "Suggested for you" asks the server for anything only while its flag is on.

import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import SuggestedForYouRow from "../components/SuggestedForYouRow";
import { catalogClient, ShopHarness } from "../fixtures";
import { invalidateShopQueries } from "../query";
import { fetchRemoteFlags, primeRemoteFlags, refreshRemoteFlags, REMOTE_FLAG, useRemoteFlag } from "./remoteFlags";

type FlagsWindow = Window & { __SHOP_FEATURE_FLAGS__?: string };

const ON = { "dapps-shop-suggested-for-you": true };
let answers: Array<() => Response>;
const requested: string[] = [];
beforeEach(() => {
  requested.length = 0;
  answers = [];
  primeRemoteFlags(null);
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input instanceof Request ? input.url : input);
      requested.push(url);
      if (url.endsWith("/dapps.json")) return (answers.shift() ?? json({ flags: {} }))();
      return new Response("[]", { status: 200 });
    }),
  );
});
afterEach(() => {
  vi.unstubAllGlobals();
  delete (window as FlagsWindow).__SHOP_FEATURE_FLAGS__;
  primeRemoteFlags(null);
});

const json = (body: unknown, status = 200) => () => new Response(JSON.stringify(body), { status });
const flagReads = () => requested.filter((u) => u.endsWith("/dapps.json")).length;

function Probe() {
  return <span data-testid="flag">{String(useRemoteFlag(REMOTE_FLAG.suggestedForYou))}</span>;
}

describe("fetchRemoteFlags", () => {
  it("reads the dapps file of Decentraland's feature-flags service, keeping only true as on", async () => {
    answers.push(json({ flags: { "dapps-shop-suggested-for-you": true, "dapps-other": "yes" } }));
    expect(await fetchRemoteFlags()).toEqual({ "dapps-shop-suggested-for-you": true, "dapps-other": false });
    expect(requested).toEqual(["https://feature-flags.decentraland.org/dapps.json"]);
  });

  it("follows a host's own service", async () => {
    (window as FlagsWindow).__SHOP_FEATURE_FLAGS__ = "https://flags.example/";
    await fetchRemoteFlags();
    expect(requested).toEqual(["https://flags.example/dapps.json"]);
  });
});

describe("useRemoteFlag", () => {
  it("is off until the service answers, then on when the flag is true", async () => {
    answers.push(json({ flags: ON }));
    const view = render(<Probe />);
    expect(view.getByTestId("flag").textContent).toBe("false");
    await act(async () => refreshRemoteFlags());
    expect(view.getByTestId("flag").textContent).toBe("true");
  });

  it("retries a failed read once, and stays off when both fail", async () => {
    answers.push(json({}, 502), json({ flags: ON }));
    await refreshRemoteFlags();
    expect(flagReads()).toBe(2);
    primeRemoteFlags(null);
    answers.push(json({}, 502), json({}, 502));
    const view = render(<Probe />);
    await act(async () => refreshRemoteFlags());
    expect(view.getByTestId("flag").textContent).toBe("false");
  });

  it("holds an answer for 60s, and keeps it when a later refresh fails", async () => {
    answers.push(json({ flags: ON }));
    await refreshRemoteFlags();
    await refreshRemoteFlags();
    expect(flagReads()).toBe(1);
    answers.push(json({}, 500), json({}, 500));
    await refreshRemoteFlags(Date.now() + 61_000);
    expect(flagReads()).toBe(3);
    const view = render(<Probe />);
    expect(view.getByTestId("flag").textContent).toBe("true");
  });

  it("waits 10s after a failed refresh before reading again", async () => {
    answers.push(json({}, 500), json({}, 500));
    const now = Date.now();
    await refreshRemoteFlags(now);
    expect(flagReads()).toBe(2);
    await refreshRemoteFlags(now + 5_000);
    expect(flagReads()).toBe(2);
    await refreshRemoteFlags(now + 11_000);
    expect(flagReads()).toBe(3);
  });

  it("keeps primed flags: no read replaces them, not even one already running", async () => {
    answers.push(json({ flags: {} }));
    const running = refreshRemoteFlags();
    primeRemoteFlags(ON);
    await running;
    const view = render(<Probe />);
    expect(view.getByTestId("flag").textContent).toBe("true");
    await refreshRemoteFlags(Date.now() + 61_000);
    expect(flagReads()).toBe(1);
  });

  it("reads again after forgetting primed flags, even while an older read is still running", async () => {
    let release!: () => void;
    const held = new Promise<void>((r) => (release = r));
    vi.mocked(fetch).mockImplementationOnce(async (input) => {
      requested.push(String(input));
      await held;
      return new Response(JSON.stringify({ flags: {} }));
    });
    const stale = refreshRemoteFlags();
    primeRemoteFlags(null);
    answers.push(json({ flags: ON }));
    const view = render(<Probe />);
    await act(async () => refreshRemoteFlags());
    expect(flagReads()).toBe(2);
    expect(view.getByTestId("flag").textContent).toBe("true");
    release();
    await act(async () => stale);
    expect(view.getByTestId("flag").textContent).toBe("true");
  });

  it("survives a page invalidating its queries", async () => {
    primeRemoteFlags(ON);
    const view = render(<Probe />);
    act(() => invalidateShopQueries());
    expect(view.getByTestId("flag").textContent).toBe("true");
    expect(flagReads()).toBe(0);
  });
});

describe("Suggested for you behind its flag", () => {
  function row(remoteFlags?: Record<string, boolean>) {
    const client = catalogClient();
    render(
      <ShopHarness session="web3" client={client} remoteFlags={remoteFlags}>
        <SuggestedForYouRow />
      </ShopHarness>,
    );
    return client;
  }
  const settle = () => act(async () => new Promise((r) => setTimeout(r, 50)));
  const askedForSuggestions = (client: ReturnType<typeof catalogClient>) => client.calls.some((c) => c.url.includes("suggested"));

  it("asks for nothing while the flag is absent, as in production today", async () => {
    const client = row();
    await settle();
    expect(askedForSuggestions(client)).toBe(false);
    expect(flagReads()).toBe(0);
  });

  it("asks once the flag is on", async () => {
    const client = row(ON);
    await settle();
    expect(askedForSuggestions(client)).toBe(true);
  });
});
