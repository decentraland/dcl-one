import { describe, expect, it } from "vitest";

import { collectCampaignParams, normalizeCampaignParamValue } from "./campaign-params";

describe("collectCampaignParams", () => {
  it("keeps every utm_* key, normalized as the legacy site did", () => {
    expect(
      collectCampaignParams(
        "?utm_org=DCL&utm_source=%20She%20Fi%20&utm_medium=e-mail&utm_campaign=Spring%20Sale!!&utm_content=a__b&utm_term=x&other=1",
      ),
    ).toEqual({
      utm_org: "dcl",
      utm_source: "she_fi",
      utm_medium: "e-mail",
      utm_campaign: "spring_sale",
      utm_content: "a_b",
      utm_term: "x",
    });
  });

  it("returns nothing without campaign parameters", () => {
    expect(collectCampaignParams("")).toEqual({});
    expect(collectCampaignParams("?position=1,1")).toEqual({});
  });

  it("drops a value that normalizes to nothing, and an empty one", () => {
    expect(collectCampaignParams("?utm_source=%21%40%23&utm_medium=&utm_campaign=___")).toEqual({});
  });

  it("caps a value at 256 characters", () => {
    expect(normalizeCampaignParamValue("a".repeat(300))).toHaveLength(256);
  });

  it("strips leading and trailing underscores left by whitespace", () => {
    expect(normalizeCampaignParamValue("  _Hello World_ ")).toBe("hello_world");
  });
});
