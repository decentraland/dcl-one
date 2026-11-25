import { describe, expect, it } from "vitest";

import { buildGooglePlayUrl, GOOGLE_PLAY_BASE_URL, googlePlayHref } from "./google-play";

const params = (href: string) => Object.fromEntries(new URL(href).searchParams);

describe("googlePlayHref", () => {
  it("tags the listing with the site's own campaign by default, mirrored into referrer", () => {
    expect(googlePlayHref()).toBe(
      "https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer&utm_org=dclrgl&utm_source=fdn&utm_medium=qr&utm_campaign=dclpage&utm_content=android&referrer=utm_org%3Ddclrgl%26utm_source%3Dfdn%26utm_medium%3Dqr%26utm_campaign%3Ddclpage%26utm_content%3Dandroid",
    );
    expect(googlePlayHref("?position=1,1")).toBe(googlePlayHref());
  });

  it("overlays the visitor's normalized utm_* on the defaults, keeping the defaults' order", () => {
    const href = googlePlayHref("?utm_source=She%20Fi&utm_campaign=Launch!!&utm_term=wearables");
    expect(params(href)).toEqual({
      id: "org.decentraland.godotexplorer",
      utm_org: "dclrgl",
      utm_source: "she_fi",
      utm_medium: "qr",
      utm_campaign: "launch",
      utm_content: "android",
      utm_term: "wearables",
      referrer: "utm_org=dclrgl&utm_source=she_fi&utm_medium=qr&utm_campaign=launch&utm_content=android&utm_term=wearables",
    });
  });

  it("keeps a default where the visitor's value normalizes to nothing", () => {
    expect(params(googlePlayHref("?utm_source=%21%21&utm_medium="))).toMatchObject({ utm_source: "fdn", utm_medium: "qr" });
  });

  it("percent-encodes the nested referrer so it stays one parameter", () => {
    const href = googlePlayHref("?utm_content=a-b_c");
    expect(href).toContain("&referrer=utm_org%3Ddclrgl%26utm_source%3Dfdn%26utm_medium%3Dqr%26utm_campaign%3Ddclpage%26utm_content%3Da-b_c");
    expect([...new URL(href).searchParams.keys()].filter((k) => k === "referrer")).toHaveLength(1);
  });
});

describe("buildGooglePlayUrl", () => {
  it("mirrors utm_* already on the base URL and leaves other parameters out of referrer", () => {
    const href = buildGooglePlayUrl(`${GOOGLE_PLAY_BASE_URL}&utm_term=x&hl=es`, { utm_source: "s" });
    expect(params(href).referrer).toBe("utm_term=x&utm_source=s");
    expect(params(href).hl).toBe("es");
  });

  it("replaces an existing referrer rather than nesting it", () => {
    const href = buildGooglePlayUrl(`${GOOGLE_PLAY_BASE_URL}&referrer=old`, { utm_source: "s" });
    expect(params(href).referrer).toBe("utm_source=s");
  });
});
