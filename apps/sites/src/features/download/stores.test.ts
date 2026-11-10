import { describe, expect, it } from "vitest";

import { googlePlayUrl } from "./stores";

const params = (url: string) => Object.fromEntries(new URL(url).searchParams);

describe("googlePlayUrl", () => {
  it("tags a direct visit with the site's default campaign, mirrored into referrer", () => {
    const query = params(googlePlayUrl());

    expect(query).toMatchObject({
      id: "org.decentraland.godotexplorer",
      utm_source: "fdn",
      utm_medium: "qr",
    });
    expect(Object.fromEntries(new URLSearchParams(query.referrer))).toEqual({
      utm_org: "dclrgl",
      utm_source: "fdn",
      utm_medium: "qr",
      utm_campaign: "dclpage",
      utm_content: "android",
    });
  });

  it("carries the visitor's live campaign over the default into referrer", () => {
    const query = params(
      googlePlayUrl({ utm_source: "partner", utm_campaign: "launch" }),
    );

    expect(new URLSearchParams(query.referrer).get("utm_source")).toBe(
      "partner",
    );
    expect(new URLSearchParams(query.referrer).get("utm_campaign")).toBe(
      "launch",
    );
    expect(new URLSearchParams(query.referrer).get("utm_medium")).toBe("qr");
  });
});
