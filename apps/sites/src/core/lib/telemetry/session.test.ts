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
    expect(new Set(Array.from({ length: 100 }, () => createSid())).size).toBe(100);
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
    expect(expireSidCookie({ domain: "example.org" })).toContain("Domain=example.org");
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

    const request = (cookie: string) => new Request("https://play.example.org/", { headers: { cookie } });
    expect(hasSplitSidCookie(request("sid=a; sid=b"))).toBe(true);
    expect(hasSplitSidCookie(request("sid=a; other=b"))).toBe(false);
    expect(hasSplitSidCookie(new Request("https://play.example.org/"))).toBe(false);
  });
});
