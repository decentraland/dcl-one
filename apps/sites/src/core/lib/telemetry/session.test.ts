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

  it("adopts the legacy analytics.js anonymous id when there is no sid yet, and an existing sid wins", () => {
    const withCookie = (cookie: string) => new Request("https://sites.example.org/", { headers: { cookie } });
    const uuid = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
    expect(ensureSid(withCookie(`ajs_anonymous_id=${uuid}`))).toEqual({ sid: uuid, created: true });
    expect(ensureSid(withCookie(`ajs_anonymous_id=${encodeURIComponent(`"${uuid}"`)}`))).toEqual({ sid: uuid, created: true });
    expect(ensureSid(withCookie(`ajs_anonymous_id="${uuid}"`))).toEqual({ sid: uuid, created: true });
    expect(ensureSid(withCookie("ajs_anonymous_id=custom-id-42"))).toEqual({ sid: "custom-id-42", created: true });
    expect(ensureSid(withCookie(`sid=current; ajs_anonymous_id=${uuid}`))).toEqual({ sid: "current", created: false });

    for (const invalid of ["short", "has space here", "<script>alert(1)</script>", "a".repeat(65), '""', "%E0%A4%A"]) {
      const created = ensureSid(withCookie(`ajs_anonymous_id=${invalid}`));
      expect(created.created).toBe(true);
      expect(created.sid).not.toBe(invalid);
      expect(created.sid).toMatch(/^[0-9a-f-]{32,36}$/);
    }
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
