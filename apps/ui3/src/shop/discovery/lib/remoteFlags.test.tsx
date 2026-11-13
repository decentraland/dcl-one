// Upstream's rollout flags, read from the same feature-flags file: what each answer means, how long it is
// held, and that "Suggested for you" asks the server for anything only while its flag is on.

import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import SuggestedForYouRow from "../components/SuggestedForYouRow";
import { catalogClient, ShopHarness } from "../fixtures";
import { invalidateShopQueries } from "../query";
import { fetchRemoteFlags, parseRemoteFlags, primeRemoteFlags, readRemoteFlags, refreshRemoteFlags, REMOTE_FLAG, remoteFlagVariant, useRemoteFlag } from "./remoteFlags";

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

describe("an HTTP error from the flags service", () => {
  it("refuses with its status, not with a body that fails", async () => {
    const broken = new ReadableStream({
      start(controller) {
        controller.error(new Error("broken error body"));
      },
    });
    answers.push(() => new Response(broken, { status: 503 }));
    await expect(readRemoteFlags()).rejects.toThrow("feature flags: HTTP 503");
  });

  it("refuses at once, without waiting on a body that never ends", async () => {
    answers.push(() => new Response(new ReadableStream({ start() {} }), { status: 503 }));
    const waiting = new Promise((_, reject) => setTimeout(() => reject(new Error("still waiting on the body")), 200));
    await expect(Promise.race([readRemoteFlags(), waiting])).rejects.toThrow("feature flags: HTTP 503");
  });
});

describe("variants", () => {
  const LIST = "0x00000000000000000000000000000000000000aa, 0x00000000000000000000000000000000000000bb";

  it("keeps an enabled variant's string payload, as upstream, and nothing of a disabled or non-string one", () => {
    expect(
      parseRemoteFlags({
        flags: { "dapps-shop-my-store": true },
        variants: {
          "dapps-shop-my-store": { name: "testers", enabled: true, payload: { type: "string", value: LIST } },
          "dapps-off": { name: "x", enabled: false, payload: { type: "string", value: "kept-nowhere" } },
          "dapps-json": { name: "x", enabled: true, payload: { type: "json", value: { a: 1 } } },
          "dapps-bare": { name: "disabled", enabled: true },
        },
      }),
    ).toEqual({ flags: { "dapps-shop-my-store": true }, variants: { "dapps-shop-my-store": LIST } });
    expect(parseRemoteFlags(null)).toEqual({ flags: {}, variants: {} });
    expect(parseRemoteFlags({ flags: null, variants: null })).toEqual({ flags: {}, variants: {} });
  });

  it("a read publishes the variants with the flags, and a prime holds the ones it is given", async () => {
    answers.push(json({ flags: { "dapps-shop-my-store": true }, variants: { "dapps-shop-my-store": { enabled: true, payload: { value: LIST } } } }));
    await refreshRemoteFlags();
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBe(LIST);
    primeRemoteFlags({ "dapps-shop-my-store": true });
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBeNull();
    primeRemoteFlags({ "dapps-shop-my-store": true }, { "dapps-shop-my-store": LIST });
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBe(LIST);
    primeRemoteFlags(null);
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBeNull();
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
