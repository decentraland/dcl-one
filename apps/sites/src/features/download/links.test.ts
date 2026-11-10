import { describe, expect, it } from "vitest";

import { cdnInstallerUrl, resolveInstallerUrl } from "./links";

const gateway = "https://gateway.example/";
const anonUserId = "3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f";

describe("resolveInstallerUrl", () => {
  it.each([
    ["windows", "amd64", "decentraland.exe"],
    ["macos", "arm64", "decentraland.dmg"],
  ] as const)(
    "uses the identity gateway URL on %s/%s when an identity resolved",
    (os, arch, file) => {
      const result = resolveInstallerUrl({
        os,
        arch,
        gateway,
        identityId: "id-123",
        anonUserId,
      });
      expect(result.source).toBe("identity");
      expect(result.url).toBe(
        `https://gateway.example/id-123/${file}?anon_user_id=${anonUserId}`,
      );
    },
  );

  it("falls back to the anonymous gateway when the identity lookup failed", () => {
    const result = resolveInstallerUrl({
      os: "windows",
      arch: "amd64",
      gateway,
      anonUserId,
    });
    expect(result.source).toBe("anonymous");
    expect(result.url).toBe(
      `https://gateway.example/anonymous/decentraland.exe?anon_user_id=${anonUserId}`,
    );
  });

  it("carries deep link and referrer on the gateway URL", () => {
    const { url } = resolveInstallerUrl({
      os: "macos",
      arch: "arm64",
      gateway,
      anonUserId,
      query: {
        position: "1,2",
        realm: "foo.dcl.eth",
        referrer: "0xabc",
        empty: undefined,
      },
    });
    const params = new URL(url).searchParams;
    expect(params.get("position")).toBe("1,2");
    expect(params.get("realm")).toBe("foo.dcl.eth");
    expect(params.get("referrer")).toBe("0xabc");
    expect(params.has("empty")).toBe(false);
  });

  describe("with a gateway configured", () => {
    it.each([
      ["a plain download", {}],
      ["a deep link", { position: "1,2", realm: "foo.dcl.eth" }],
      ["a referrer", { referrer: "0xabc" }],
    ])(
      "never chooses the CDN for %s, and keeps its query",
      (_, query: Record<string, string>) => {
        const result = resolveInstallerUrl({
          os: "windows",
          arch: "amd64",
          gateway,
          anonUserId,
          query,
        });
        expect(result.source).toBe("anonymous");
        const params = new URL(result.url).searchParams;
        for (const [key, value] of Object.entries(query))
          expect(params.get(key)).toBe(value);
        expect(params.get("anon_user_id")).toBe(anonUserId);
      },
    );
  });

  describe("without a usable gateway (DOWNLOAD_GATEWAY_URL unset or blank)", () => {
    it.each([
      ["unset", undefined],
      ["empty", ""],
      ["blank", "   "],
    ])(
      "serves the generic CDN installer for a plain download when %s",
      (_, value) => {
        expect(
          resolveInstallerUrl({
            os: "windows",
            arch: "amd64",
            gateway: value,
            anonUserId,
          }),
        ).toEqual({
          url: cdnInstallerUrl("windows", "amd64"),
          source: "cdn",
          filename: "Decentraland_installer.exe",
        });
      },
    );

    // There is no gateway to bake them into, so a deep link or referrer cannot reach the binary.
    // The production server logs a startup warning for this configuration (download-gateway.test.ts).
    it.each([
      ["unset", undefined],
      ["blank", "   "],
    ])(
      "serves the generic CDN installer and drops a deep link and referrer when %s",
      (_, value) => {
        const result = resolveInstallerUrl({
          os: "macos",
          arch: "arm64",
          gateway: value,
          anonUserId,
          query: { position: "1,2", realm: "foo.dcl.eth", referrer: "0xabc" },
        });
        expect(result).toEqual({
          url: cdnInstallerUrl("macos", "arm64"),
          source: "cdn",
          filename: "Decentraland_installer.dmg",
        });
      },
    );
  });

  it("always serves the legacy CDN DMG on macOS amd64, even with an identity", () => {
    const result = resolveInstallerUrl({
      os: "macos",
      arch: "amd64",
      gateway,
      identityId: "id-123",
      anonUserId,
    });
    expect(result.source).toBe("cdn");
    expect(result.url.startsWith(cdnInstallerUrl("macos", "amd64"))).toBe(true);
    expect(result.url).not.toContain("id-123");
  });

  it("produces no identity or attribution when the gateway is not configured", () => {
    const result = resolveInstallerUrl({
      os: "windows",
      arch: "amd64",
      gateway: undefined,
      identityId: "id-123",
      anonUserId,
      query: { position: "1,2", referrer: "0xabc" },
    });
    expect(result).toEqual({
      url: cdnInstallerUrl("windows", "amd64"),
      source: "cdn",
      filename: "Decentraland_installer.exe",
    });
  });
});
