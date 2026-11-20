import { describe, expect, it } from "vitest";

import { CHAIN } from "../model/chain";
import { LAND_NETWORKS, landTokenId } from "./contracts";
import { landIdentity, landRedirectTarget } from "./redirects";

const NET = LAND_NETWORKS[1]!;

describe("old LAND URLs land on the pages that do the job", () => {
  it("/marketplace/land goes to the LAND browse, its category as the section, the rest of the query kept", () => {
    expect(landRedirectTarget("lands", "", CHAIN.ETHEREUM)).toBe("/shop/lands");
    expect(landRedirectTarget("lands", "?category=estate&page=2", CHAIN.ETHEREUM)).toBe("/shop/lands?page=2&section=estates");
  });

  it("/marketplace/rent-land goes to the rent page of the LAND it names, else the LAND for rent", () => {
    expect(landRedirectTarget("rent", `?id=${NET.land}-42&from=map`, CHAIN.ETHEREUM)).toBe(`/shop/token/${NET.land}/42/rent?from=map`);
    expect(landRedirectTarget("rent", "?x=-3&y=12", CHAIN.ETHEREUM)).toBe(`/shop/token/${NET.land}/${landTokenId(-3, 12)}/rent`);
    expect(landRedirectTarget("rent", "?from=home", CHAIN.ETHEREUM)).toBe("/shop/lands?from=home&onlyOnRent=true");
  });

  it("the Builder's transfer, operator and NAME-assignment URLs go to the LAND's transfer or manage page", () => {
    expect(landRedirectTarget("transfer", "?id=12,-4", CHAIN.ETHEREUM)).toBe(`/shop/token/${NET.land}/${landTokenId(12, -4)}/transfer`);
    expect(landRedirectTarget("operator", "?id=4213", CHAIN.ETHEREUM)).toBe(`/shop/token/${NET.estate}/4213/manage`);
    expect(landRedirectTarget("assign-ens", `?contractAddress=${NET.estate.toUpperCase().replace("0X", "0x")}&tokenId=7`, CHAIN.ETHEREUM)).toBe(`/shop/token/${NET.estate}/7/manage`);
  });

  it("without a LAND to name, the Builder URLs go to the account's assets", () => {
    expect(landRedirectTarget("operator", "", CHAIN.ETHEREUM)).toBe("/profile/me/assets");
    expect(landRedirectTarget("transfer", "?ref=mail", CHAIN.ETHEREUM)).toBe("/profile/me/assets?ref=mail");
  });

  it("reads every identity form, and nothing else", () => {
    expect(landIdentity(new URLSearchParams(`id=${NET.land}:5`), CHAIN.ETHEREUM)).toEqual({ contract: NET.land, tokenId: "5" });
    expect(landIdentity(new URLSearchParams("estateId=9"), CHAIN.ETHEREUM)).toEqual({ contract: NET.estate, tokenId: "9" });
    expect(landIdentity(new URLSearchParams("id=nope"), CHAIN.ETHEREUM)).toBeNull();
    expect(landIdentity(new URLSearchParams("contractAddress=0x12&tokenId=1"), CHAIN.ETHEREUM)).toBeNull();
  });

  it("should resolve an implicit estate on the selected Ethereum network", () => {
    const { estate } = LAND_NETWORKS[CHAIN.SEPOLIA]!;
    expect(landRedirectTarget("operator", "?estateId=9&from=builder", CHAIN.SEPOLIA)).toBe(
      `/shop/token/${estate}/9/manage?from=builder`,
    );
  });
});
