import { Buffer } from "node:buffer";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { developmentDiscoveryGetTarget } from "./bases";
import { getJSON, postJSON, signedGetJSON } from "./client";
import { fetchEvents } from "./places/events";
import { fetchMostActivePlaces } from "./places/index";
import { loadHome } from "./landings/home.server";
import type { AuthIdentity } from "../auth/types";
import { signedFetch } from "../auth/signer";

vi.mock("../auth/signer", () => ({ signedFetch: vi.fn() }));

describe("when the explicit development public discovery bridge is enabled", () => {
  let fetchImpl: ReturnType<typeof vi.fn<typeof fetch>>;
  let body: unknown;
  let status: number;
  let requested: URL;
  let init: RequestInit | undefined;
  let identity: AuthIdentity;

  beforeEach(() => {
    vi.stubEnv("DEV", true);
    vi.stubEnv("MODE", "development");
    vi.stubEnv("PROD", false);
    vi.stubEnv("VITE_SITES_DEV_PUBLIC_DISCOVERY", "1");
    vi.stubEnv("CATALYST_URL", "https://catalyst.example");
    body = { ok: true, data: [], total: 0 };
    status = 200;
    init = undefined;
    identity = {
      signer: "0xfixture",
      ephemeral: { address: "0xfixture", privateKey: "0xfixture" },
      expiration: "2999-01-01T00:00:00.000Z",
      authChain: [],
    };
    fetchImpl = vi.fn<typeof fetch>(async (input, options) => {
      requested = new URL(String(input));
      init = options;
      return new Response(JSON.stringify(body), {
        status,
        statusText: status === 503 ? "Unavailable" : "OK",
      });
    });
  });
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
    vi.clearAllMocks();
  });

  it("should read the exact public events endpoint with every effective list query intact", async () => {
    await fetchEvents(
      {
        list: "live",
        limit: 2,
        offset: 4,
        recurrent: false,
        search: "a b",
        from: "2099-01-01",
        to: "2099-01-02",
        with_connected_users: true,
      },
      { fetchImpl },
    );
    expect(requested.origin + requested.pathname).toBe(
      "https://events.decentraland.org/api/events",
    );
    expect(Object.fromEntries(requested.searchParams)).toEqual({
      list: "live",
      search: "a b",
      from: "2099-01-01",
      to: "2099-01-02",
      recurrent: "false",
      with_connected_users: "true",
      limit: "2",
      offset: "4",
    });
    expect(init).toMatchObject({
      method: "GET",
      credentials: "omit",
      headers: { accept: "application/json" },
    });
  });
  it("should reuse the existing places parser against the public most-active list without fabricated rows", async () => {
    await expect(
      fetchMostActivePlaces({ limit: 4 }, { fetchImpl }),
    ).resolves.toEqual([]);
    expect(requested.href).toBe(
      "https://places.decentraland.org/api/places?limit=4&order_by=most_active&with_realtime_active_users=true",
    );
    expect(init?.credentials).toBe("omit");
  });
  describe.each([
    [
      "missing flag",
      () => vi.stubEnv("VITE_SITES_DEV_PUBLIC_DISCOVERY", undefined),
    ],
    [
      "non-exact flag",
      () => vi.stubEnv("VITE_SITES_DEV_PUBLIC_DISCOVERY", "true"),
    ],
    ["production build", () => vi.stubEnv("DEV", false)],
    ["production environment", () => vi.stubEnv("PROD", true)],
    ["production mode", () => vi.stubEnv("MODE", "production")],
  ])("and there is a %s", (_label, configure) => {
    beforeEach(() => configure());
    it("should keep the same Catalyst base and path", async () => {
      await getJSON("/events/v1/events", {
        fetchImpl,
        query: { list: "live" },
      });
      expect(requested.href).toBe(
        "https://catalyst.example/events/v1/events?list=live",
      );
      expect(init?.credentials).toBeUndefined();
    });
  });
  describe.each([
    "/events/v1/events/extra",
    "/events/v1/events/",
    "/events/api/events/id",
    "/places/api/places/id",
    "/places/api/places-extra",
    "/places/api/destinations",
  ])("and the path is outside the exact bridge: %s", (path) => {
    it("should retain the original Catalyst request", async () => {
      await getJSON(path, { fetchImpl });
      expect(requested.href).toBe("https://catalyst.example" + path);
    });
  });
  it("should honor an explicit base override instead of contacting public production", async () => {
    await getJSON("/events/v1/events", {
      base: "https://isolated.example",
      fetchImpl,
    });
    expect(requested.href).toBe("https://isolated.example/events/v1/events");
  });
  describe.each([
    "Authorization",
    "Cookie",
    "X-Identity-Auth-Chain",
    "x-custom-signature",
  ])("and the request includes %s", (header) => {
    it("should keep authenticated or caller-header reads on Catalyst", async () => {
      await getJSON("/events/v1/events", {
        fetchImpl,
        headers: { [header]: "fixture-only" },
      });
      expect(requested.origin).toBe("https://catalyst.example");
      expect(init?.headers).toEqual({
        accept: "application/json",
        [header]: "fixture-only",
      });
    });
  });
  it("should retain unsupported query semantics on Catalyst instead of dropping a filter", async () => {
    await getJSON("/places/api/places", {
      fetchImpl,
      query: { creator_address: "public-address", limit: 4 },
    });
    expect(requested.href).toBe(
      "https://catalyst.example/places/api/places?creator_address=public-address&limit=4",
    );
  });
  it("should allow omitted query values without inventing effective filters", async () => {
    await getJSON("/events/v1/events", {
      fetchImpl,
      query: { list: "live", unused: undefined },
    });
    expect(requested.href).toBe(
      "https://events.decentraland.org/api/events?list=live",
    );
  });
  it("should keep signed reads and writes on Catalyst", async () => {
    vi.mocked(signedFetch).mockImplementation(
      async () => new Response(JSON.stringify({ ok: true })),
    );
    await signedGetJSON("/events/v1/events", { identity });
    expect(signedFetch).toHaveBeenLastCalledWith(
      identity,
      "https://catalyst.example/events/v1/events",
      expect.objectContaining({ method: "GET" }),
    );
    await postJSON("/places/api/places", {}, { identity });
    expect(signedFetch).toHaveBeenLastCalledWith(
      identity,
      "https://catalyst.example/places/api/places",
      expect.objectContaining({ method: "POST" }),
    );
  });
  describe("and the actual homepage loader reads these configured clients", () => {
    beforeEach(() => {
      vi.stubEnv("DOWNLOAD_COUNTS_URL", undefined);
    });
    it("should keep successful empty lists empty without a fabricated Genesis card or creator lookup", async () => {
      const result = await loadHome({ fetchImpl });
      expect(result.whatsOn).toEqual([]);
      expect(result.unreadable).toEqual([]);
      expect(fetchImpl).toHaveBeenCalledTimes(6);
      expect(
        fetchImpl.mock.calls.every(([input]) =>
          [
            "https://events.decentraland.org",
            "https://places.decentraland.org",
          ].includes(new URL(String(input)).origin),
        ),
      ).toBe(true);
    });
    it("should report failed public reads without retrying Catalyst or fabricating cards", async () => {
      status = 503;
      const result = await loadHome({ fetchImpl });
      expect(result.whatsOn).toEqual([]);
      expect(result.unreadable).toEqual(
        expect.arrayContaining([
          "live-events",
          "events",
          "hotspots",
          "rituals",
        ]),
      );
      expect(fetchImpl).toHaveBeenCalledTimes(6);
      expect(
        fetchImpl.mock.calls.every(([input]) =>
          [
            "https://events.decentraland.org",
            "https://places.decentraland.org",
          ].includes(new URL(String(input)).origin),
        ),
      ).toBe(true);
    });
  });
  describe("and public production returns an HTTP error", () => {
    beforeEach(() => {
      status = 503;
    });
    it("should report the failure without falling back to another provider", async () => {
      await expect(
        fetchEvents({ list: "live" }, { fetchImpl }),
      ).rejects.toMatchObject({
        name: "CatalystError",
        status: 503,
        url: "https://events.decentraland.org/api/events?list=live",
      });
      expect(fetchImpl).toHaveBeenCalledTimes(1);
    });
  });
  describe("and public production is unreachable", () => {
    beforeEach(() => {
      fetchImpl.mockRejectedValueOnce(new Error("offline"));
    });
    it("should preserve the network failure and make no fallback request", async () => {
      await expect(fetchMostActivePlaces({}, { fetchImpl })).rejects.toThrow(
        "Catalyst request failed: offline",
      );
      expect(fetchImpl).toHaveBeenCalledTimes(1);
    });
  });
  describe("and the public event envelope is malformed", () => {
    beforeEach(() => {
      body = { unexpected: true };
      vi.spyOn(console, "warn").mockImplementation(() => {});
    });
    it("should preserve existing validation failure rather than fabricate an empty successful list", async () => {
      await expect(fetchEvents({}, { fetchImpl })).rejects.toThrow(
        "events envelope failed validation",
      );
      expect(fetchImpl).toHaveBeenCalledTimes(1);
    });
  });

  describe.each(["production", "development"])(
    "and the helper is compiled for production with the opt-in flag and NODE_ENV=%s",
    (nodeEnv) => {
      let compiled: typeof import("./bases");
      beforeEach(async () => {
        vi.stubEnv("NODE_ENV", nodeEnv);
        const result = await build({
          configFile: false,
          mode: "production",
          logLevel: "silent",
          define: {
            "import.meta.env.VITE_SITES_DEV_PUBLIC_DISCOVERY":
              JSON.stringify("1"),
          },
          build: {
            write: false,
            ssr: fileURLToPath(new URL("./bases.ts", import.meta.url)),
          },
        });
        const outputs = (Array.isArray(result) ? result : [result]).flatMap(
          (item) => ("output" in item ? item.output : []),
        );
        const chunk = outputs.find((item) => item.type === "chunk");
        if (!chunk || chunk.type !== "chunk")
          throw new Error("Missing compiled production helper");
        compiled = await import(
          /* @vite-ignore */ "data:text/javascript;base64," +
            Buffer.from(chunk.code).toString("base64")
        );
        vi.stubEnv("NODE_ENV", "development");
      });
      it("should refuse the bridge even after runtime environment flags change", () => {
        expect(
          compiled.developmentDiscoveryGetTarget("/events/v1/events"),
        ).toBeUndefined();
        expect(developmentDiscoveryGetTarget("/events/v1/events")).toEqual({
          base: "https://events.decentraland.org",
          path: "/api/events",
        });
      });
    },
  );
});
