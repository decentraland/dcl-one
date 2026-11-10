// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";

import type { DeliveryResult } from "./delivery";
import { cdnInstallerUrl } from "./links";
import {
  gatewayAnonUserId,
  nextRevisit,
  resolveIdentityId,
  runDownload,
  type DownloadTracker,
} from "./success";

const ANON = "3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f";
const GATEWAY = "https://gateway.example";
const identity = { authChain: [] } as unknown as AuthIdentity;

type Event = {
  event: "started" | "success" | "failed";
  href: string;
  detail?: unknown;
};
let events: Event[];
const tracker = (href: string): DownloadTracker => ({
  started: () => events.push({ event: "started", href }),
  success: (detail) => events.push({ event: "success", href, detail }),
  failed: (reason) => events.push({ event: "failed", href, detail: reason }),
});
const delivered: DeliveryResult = {
  deliveryMode: "streamed",
  gatewayRequestId: "req-1",
  bytesTransferred: 5,
};

function run(overrides: Partial<Parameters<typeof runDownload>[0]> = {}) {
  return runDownload({
    os: "windows",
    arch: "amd64",
    gateway: GATEWAY,
    identity: null,
    anonUserId: ANON,
    query: {},
    signal: new AbortController().signal,
    tracker,
    resolveIdentity: vi.fn(async () => undefined),
    deliver: vi.fn(async () => delivered),
    ...overrides,
  });
}

beforeEach(() => {
  events = [];
  sessionStorage.clear();
  localStorage.clear();
});

describe("runDownload URL resolution", () => {
  it.each([
    ["windows", "amd64", "decentraland.exe"],
    ["macos", "arm64", "decentraland.dmg"],
  ] as const)(
    "gives a signed-in %s %s visitor the identity gateway URL",
    async (os, arch, file) => {
      const deliver = vi.fn(async () => delivered);
      const outcome = await run({
        os,
        arch,
        identity,
        resolveIdentity: vi.fn(async () => "id-7"),
        deliver,
      });

      const href = `${GATEWAY}/id-7/${file}?anon_user_id=${ANON}`;
      expect(outcome).toMatchObject({
        status: "success",
        installer: { source: "identity", url: href },
      });
      expect(deliver).toHaveBeenCalledWith(
        expect.objectContaining({ url: href, filename: file, os }),
      );
      // Telemetry never carries the identity id: the launcher authenticates with it.
      const redacted = `${GATEWAY}/%3Cidentity%3E/${file}?anon_user_id=${ANON}`;
      expect(events.map((e) => [e.event, e.href])).toEqual([
        ["started", redacted],
        ["success", redacted],
      ]);
    },
  );

  it("falls back to the anonymous gateway, never the CDN, when the identity lookup fails", async () => {
    const outcome = await run({
      identity,
      resolveIdentity: vi.fn(async () => undefined),
    });

    expect(outcome).toMatchObject({
      status: "success",
      installer: {
        source: "anonymous",
        url: `${GATEWAY}/anonymous/decentraland.exe?anon_user_id=${ANON}`,
      },
    });
  });

  it("keeps the deep link and referrer on the gateway URL without an identity", async () => {
    const outcome = await run({
      query: { position: "10,20", realm: "main", referrer: "partner" },
    });

    const url = new URL(
      (outcome as { installer: { url: string } }).installer.url,
    );
    expect(url.pathname).toBe("/anonymous/decentraland.exe");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      anon_user_id: ANON,
      position: "10,20",
      realm: "main",
      referrer: "partner",
    });
  });

  it("uses the bare static CDN without a gateway: no identity lookup, no attribution", async () => {
    const resolveIdentity = vi.fn(async () => "id-7");
    const outcome = await run({
      gateway: undefined,
      identity,
      resolveIdentity,
      query: { position: "10,20", referrer: "partner", utm_source: "x" },
    });

    expect(outcome).toMatchObject({
      status: "success",
      installer: { source: "cdn", url: cdnInstallerUrl("windows", "amd64") },
    });
    expect(resolveIdentity).not.toHaveBeenCalled();
  });

  it("serves macOS amd64 the legacy CDN DMG even when signed in", async () => {
    const outcome = await run({
      os: "macos",
      arch: "amd64",
      identity,
      resolveIdentity: vi.fn(async () => "id-7"),
    });
    expect(outcome).toMatchObject({
      installer: { source: "cdn", url: cdnInstallerUrl("macos", "amd64") },
    });
  });
});

describe("runDownload outcomes", () => {
  it("reports delivery details on download_success", async () => {
    await run();
    expect(events[1]!.detail).toEqual({
      filename: "decentraland.exe",
      deliveryMode: "streamed",
      gatewayRequestId: "req-1",
      bytes: 5,
    });
  });

  it("reports download_failed with the reason when delivery throws", async () => {
    const outcome = await run({
      deliver: vi.fn(async () => {
        throw new Error("disk full");
      }),
    });

    expect(outcome).toMatchObject({ status: "failed", reason: "disk full" });
    expect(events.map((e) => e.event)).toEqual(["started", "failed"]);
  });

  it("reports the failure against the CDN link when nothing resolved", async () => {
    const outcome = await run({
      resolveIdentity: vi.fn(async () => {
        throw new Error("boom");
      }),
    });

    expect(outcome).toMatchObject({ status: "failed" });
    expect(events).toEqual([
      {
        event: "failed",
        href: cdnInstallerUrl("windows", "amd64"),
        detail: "boom",
      },
    ]);
  });

  it("reports nothing once aborted", async () => {
    const controller = new AbortController();
    const outcome = await run({
      signal: controller.signal,
      deliver: vi.fn(async () => {
        controller.abort();
        throw new DOMException("aborted", "AbortError");
      }),
    });

    expect(outcome).toEqual({ status: "aborted" });
    expect(events.map((e) => e.event)).toEqual(["started"]);
  });
});

describe("resolveIdentityId", () => {
  const signal = () => new AbortController().signal;

  it("posts the identity to the same-origin auth-api", async () => {
    const post = vi.fn(async () => ({ identityId: "id-1", expiration: "" }));
    await expect(
      resolveIdentityId(identity, { signal: signal(), post }),
    ).resolves.toBe("id-1");
    expect(post).toHaveBeenCalledWith(
      identity,
      "/auth-api",
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it("yields undefined when signed out, on an auth error and on a timeout", async () => {
    await expect(
      resolveIdentityId(null, { signal: signal() }),
    ).resolves.toBeUndefined();
    await expect(
      resolveIdentityId(identity, {
        signal: signal(),
        post: vi.fn(async () => {
          throw new Error("401");
        }),
      }),
    ).resolves.toBeUndefined();
    const hang = vi.fn(
      (_: AuthIdentity, __: string, opts: { signal?: AbortSignal } = {}) =>
        new Promise<never>((_, reject) =>
          opts.signal?.addEventListener("abort", () =>
            reject(opts.signal?.reason),
          ),
        ),
    );
    await expect(
      resolveIdentityId(identity, {
        signal: signal(),
        post: hang,
        timeoutMs: 5,
      }),
    ).resolves.toBeUndefined();
  });
});

describe("nextRevisit", () => {
  it("returns the previous count per os and arch", () => {
    expect(nextRevisit("windows", "amd64")).toBe(0);
    expect(nextRevisit("windows", "amd64")).toBe(1);
    expect(nextRevisit("macos", "arm64")).toBe(0);
    expect(sessionStorage.getItem("downloadSuccess:visits:windows:amd64")).toBe(
      "2",
    );
  });
});

describe("gatewayAnonUserId", () => {
  it("prefers the id the link carried, then a UUID sid", () => {
    expect(gatewayAnonUserId(ANON, "sid-1")).toBe(ANON);
    const sid = "0b6f8a52-5d0e-4a8f-9c1d-2e3f4a5b6c7d";
    expect(gatewayAnonUserId("not-a-uuid", sid)).toBe(sid);
  });

  it("mints one UUID and keeps it when neither is usable", () => {
    const minted = gatewayAnonUserId(null, "sid-1");
    expect(minted).toMatch(/^[0-9a-f-]{36}$/);
    expect(gatewayAnonUserId(undefined, "sid-2")).toBe(minted);
  });
});
