// @vitest-environment jsdom
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

// The real transport (no mocked `page`): what a cold 404 actually puts on the wire.
import { lookupSession, useNotFoundPageView } from "./page-view";

const BROWSER_UA =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

function NotFound() {
  useNotFoundPageView(true);
  return null;
}

// A URL no route matches: the root loader did not run, so the page has no root data.
function renderColdNotFound(path: string) {
  const router = createMemoryRouter(
    [{ id: "root", path: "*", loader: () => null, Component: NotFound, HydrateFallback: () => null }],
    { initialEntries: [path] },
  );
  render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  );
}

type Sent = { url: string; body: Record<string, unknown> };

describe("a cold 404's page view on the wire", () => {
  let sendBeacon: ReturnType<typeof vi.fn>;
  let fetchMock: ReturnType<typeof vi.fn>;
  let session: Record<string, unknown>;

  const sent = async (): Promise<Sent[]> =>
    Promise.all(
      sendBeacon.mock.calls.map(async ([url, blob]) => ({ url: url as string, body: JSON.parse(await (blob as Blob).text()) })),
    );

  beforeEach(() => {
    // Nothing baked into the bundle, and the document published the transport empty, as root.tsx's
    // Layout does without root data.
    vi.stubEnv("VITE_SEGMENT_WRITE_KEY", "");
    vi.stubEnv("VITE_SEGMENT_API_HOST", "");
    (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "", SEGMENT_API_HOST: "" };
    Object.defineProperty(navigator, "userAgent", { value: BROWSER_UA, configurable: true });
    sendBeacon = vi.fn(() => true);
    Object.defineProperty(navigator, "sendBeacon", { value: sendBeacon, configurable: true });
    session = { sid: "minted-sid", userId: null, segmentWriteKey: "public-key", segmentApiHost: "evs.example.test/v1" };
    fetchMock = vi.fn(async () => Response.json(session));
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
    // Own properties shadow jsdom's: deleting them restores the originals.
    Reflect.deleteProperty(navigator, "userAgent");
    Reflect.deleteProperty(navigator, "sendBeacon");
    Reflect.deleteProperty(window, "__DCL_PUBLIC__");
  });

  it("sends 'Not Found' to the proxy with the fetched write key and sid, from one lookup", async () => {
    renderColdNotFound("/missing/cold");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [call] = await sent();
    expect(call?.url).toBe("https://evs.example.test/v1/page");
    expect(call?.body).toMatchObject({
      type: "page",
      name: "Not Found",
      writeKey: "public-key",
      anonymousId: "minted-sid",
      properties: expect.objectContaining({ path: "/missing/cold" }),
    });
    expect(call?.body).not.toHaveProperty("userId");
  });

  it("sends to Segment's own endpoint when no proxy host is configured", async () => {
    session = { ...session, segmentApiHost: "" };
    renderColdNotFound("/missing/default-host");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    const [call] = await sent();
    expect(call?.url).toBe("https://api.segment.io/v1/page");
    expect(call?.body).toMatchObject({ writeKey: "public-key", anonymousId: "minted-sid" });
  });

  it("keeps a transport the document already has", async () => {
    (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "doc-key", SEGMENT_API_HOST: "" };
    renderColdNotFound("/missing/doc-key");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    const [call] = await sent();
    expect(call?.url).toBe("https://evs.example.test/v1/page");
    expect(call?.body).toMatchObject({ writeKey: "doc-key" });
  });

  it("sends nothing when the server has no write key either", async () => {
    let answer: (response: Response) => void = () => {};
    fetchMock.mockImplementation(() => new Promise<Response>((resolve) => (answer = resolve)));
    renderColdNotFound("/missing/no-key");
    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    // Settles the page's own in-flight lookup, then lets the page act on its answer.
    await act(async () => {
      answer(Response.json({ ...session, segmentWriteKey: "", segmentApiHost: "" }));
      await lookupSession();
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(sendBeacon).not.toHaveBeenCalled();
  });
});
