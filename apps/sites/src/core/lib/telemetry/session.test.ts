import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createSid,
  ensureSid,
  expireSidCookie,
  hasSplitSidCookie,
  readSid,
  serializeSidCookie,
  sharedSidDomain,
} from "./session";

function requestWithSid(sid: string): Request {
  return new Request("https://sites.example.org/", {
    headers: { cookie: `sid=${sid}` },
  });
}

describe("telemetry session cookies", () => {
  afterEach(() => vi.unstubAllEnvs());

  it("reads an existing SID and mints a unique one otherwise", () => {
    const request = requestWithSid("abc-123");
    expect(readSid(request)).toBe("abc-123");
    expect(ensureSid(request)).toEqual({ sid: "abc-123", created: false });

    const bare = new Request("https://sites.example.org/");
    expect(readSid(bare)).toBeNull();
    const created = ensureSid(bare);
    expect(created.created).toBe(true);
    expect(created.sid.length).toBeGreaterThan(0);
    expect(new Set(Array.from({ length: 100 }, () => createSid())).size).toBe(
      100,
    );
  });

  it("shares a newly issued SID across loaders for one request and isolates separate requests", () => {
    const request = new Request("https://sites.example.org/");
    expect(ensureSid(request)).toBe(ensureSid(request));
    expect(ensureSid(new Request(request)).sid).not.toBe(
      ensureSid(request).sid,
    );
  });

  it("keeps interleaved requests isolated without mutating their headers or enumerable properties", () => {
    const first = new Request("https://sites.example.org/");
    const second = requestWithSid("known-cookie");
    const keys = Object.keys(first);
    const issued = ensureSid(first);
    const known = ensureSid(second);
    expect(known).toEqual({ sid: "known-cookie", created: false });
    expect(ensureSid(first)).toBe(issued);
    expect(ensureSid(second)).toBe(known);
    expect(issued.sid).not.toBe(known.sid);
    expect(Object.keys(first)).toEqual(keys);
    const metadata = Object.getOwnPropertySymbols(first).find(
      (symbol) => symbol.description === "telemetry-session",
    )!;
    expect(Object.getOwnPropertyDescriptor(first, metadata)?.enumerable).toBe(
      false,
    );
    expect(first.headers.get("cookie")).toBeNull();
    expect(second.headers.get("cookie")).toBe("sid=known-cookie");
  });

  it("rejects an unsupported request before issuing a session or changing its headers", () => {
    const request = Object.preventExtensions(
      new Request("https://sites.example.org/"),
    );
    expect(() => ensureSid(request)).toThrow(
      "Telemetry sessions require an extensible Request.",
    );
    expect(() => ensureSid(request)).toThrow(
      "Telemetry sessions require an extensible Request.",
    );
    expect(request.headers.get("cookie")).toBeNull();
    expect(
      Object.getOwnPropertySymbols(request).some(
        (symbol) => symbol.description === "telemetry-session",
      ),
    ).toBe(false);
  });

  it("serializes secure SID cookies and expires them with matching attributes", () => {
    const cookie = serializeSidCookie("s-1");
    expect(cookie).toContain("sid=s-1");
    expect(cookie).toContain("Path=/");
    expect(cookie).toContain("HttpOnly");
    expect(cookie).toContain("SameSite=Lax");
    expect(cookie).toContain("Secure");

    const expired = expireSidCookie();
    expect(expired.startsWith("sid=;")).toBe(true);
    expect(expired).toContain("Max-Age=0");
    expect(expired).not.toContain("Domain=");
    expect(expireSidCookie({ domain: "example.org" })).toContain(
      "Domain=example.org",
    );
  });

  it("widens only hosts under the configured shared parent and detects duplicate SID cookies", () => {
    const domainFor = (url: string) => sharedSidDomain(new Request(url));
    vi.stubEnv("SID_COOKIE_DOMAIN", "");
    expect(domainFor("https://play.example.org/play")).toBeUndefined();
    vi.stubEnv("SID_COOKIE_DOMAIN", "example.org");
    expect(domainFor("https://play.example.org/play")).toBe("example.org");
    expect(domainFor("https://example.org/")).toBe("example.org");
    expect(domainFor("https://sites.example.com/")).toBeUndefined();
    expect(domainFor("https://evilexample.org/")).toBeUndefined();

    const request = (cookie: string) =>
      new Request("https://play.example.org/", { headers: { cookie } });
    expect(hasSplitSidCookie(request("sid=a; sid=b"))).toBe(true);
    expect(hasSplitSidCookie(request("sid=a; other=b"))).toBe(false);
    expect(hasSplitSidCookie(new Request("https://play.example.org/"))).toBe(
      false,
    );
  });
});
