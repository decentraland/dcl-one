import { describe, expect, it } from "vitest";

import { landTokenId } from "../../data/catalyst/landListing";
import { LAND_NETWORKS } from "./contracts";
import { landIdentity, landRedirectTarget } from "./redirects";

const NET = LAND_NETWORKS[1]!;

describe("old LAND URLs land on the pages that do the job", () => {
  it("/marketplace/land goes to the LAND browse, its category as the section, the rest of the query kept", () => {
    expect(landRedirectTarget("lands", "")).toBe("/shop/lands");
    expect(landRedirectTarget("lands", "?category=estate&page=2")).toBe("/shop/lands?page=2&section=estates");
  });

  it("/marketplace/rent-land goes to the rent page of the LAND it names, else the LAND for rent", () => {
    expect(landRedirectTarget("rent", `?id=${NET.land}-42&from=map`)).toBe(`/shop/token/${NET.land}/42/rent?from=map`);
    expect(landRedirectTarget("rent", "?x=-3&y=12")).toBe(`/shop/token/${NET.land}/${landTokenId(-3, 12)}/rent`);
    expect(landRedirectTarget("rent", "?from=home")).toBe("/shop/lands?from=home&onlyOnRent=true");
  });

  it("the Builder's transfer, operator and NAME-assignment URLs go to the LAND's transfer or manage page", () => {
    expect(landRedirectTarget("transfer", "?id=12,-4")).toBe(`/shop/token/${NET.land}/${landTokenId(12, -4)}/transfer`);
    expect(landRedirectTarget("operator", "?id=4213")).toBe(`/shop/token/${NET.estate}/4213/manage`);
    expect(landRedirectTarget("assign-ens", `?contractAddress=${NET.estate.toUpperCase().replace("0X", "0x")}&tokenId=7`)).toBe(`/shop/token/${NET.estate}/7/manage`);
  });

  it("without a LAND to name, the Builder URLs go to the Creator Hub's LAND list (Builder authority)", () => {
    expect(landRedirectTarget("operator", "")).toBe("/create/land");
    expect(landRedirectTarget("transfer", "?ref=mail")).toBe("/create/land?ref=mail");
  });

  it("reads every identity form, and nothing else", () => {
    expect(landIdentity(new URLSearchParams(`id=${NET.land}:5`))).toEqual({ contract: NET.land, tokenId: "5" });
    expect(landIdentity(new URLSearchParams("estateId=9"))).toEqual({ contract: NET.estate, tokenId: "9" });
    expect(landIdentity(new URLSearchParams("id=nope"))).toBeNull();
    expect(landIdentity(new URLSearchParams("contractAddress=0x12&tokenId=1"))).toBeNull();
  });
});
