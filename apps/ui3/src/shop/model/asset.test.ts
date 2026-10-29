import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { evmAddress, parseEvmAddress } from "./address";
import {
  assetKey,
  itemAsset,
  nameAsset,
  nameTokenId,
  parseAssetId,
  parseAssetKey,
  parseNameLabel,
  tokenAsset,
} from "./asset";
import { CHAIN, nativeGasRequired } from "./chain";
import { manaFromWhole, manaToWire } from "./money";
import { settlementKeyId, type SettlementGroupKey } from "./settlement";
import {
  decodeEligibility,
  decodePurchaseOffer,
  decodeSettlementGroup,
  encodePurchaseOffer,
} from "./wire";

const reads = JSON.parse(
  readFileSync(join(__dirname, "../../../../../services/catalyst/tests/fixtures/shop/chain-reads.json"), "utf8"),
) as { ethereum: { dclRegistrar: string; getTokenId: { Nacho: string } }; polygon: { creditsManager: { address: string } } };

describe("identities and the chain reads (tests/shop/policy.rs identities_and_the_chain_reads)", () => {
  it("a NAME is the registrar token keccak256(lower(label))", () => {
    const label = parseNameLabel("Nacho")!;
    expect(`0x${BigInt(nameTokenId(label)).toString(16).padStart(64, "0")}`).toBe(reads.ethereum.getTokenId.Nacho);
    const registrar = { chainId: CHAIN.ETHEREUM, contract: evmAddress(reads.ethereum.dclRegistrar) };
    expect(nameAsset(registrar, parseNameLabel("nacho")!)).toEqual(nameAsset(registrar, label));
  });

  it("addresses compare by bytes and print lowercase", () => {
    const mixed = parseEvmAddress("0x8B3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5");
    expect(mixed).toBe(reads.polygon.creditsManager.address);
    expect(parseEvmAddress("8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5")).toBeNull();
    expect(parseEvmAddress("0x8b3a")).toBeNull();
  });

  it("an item id and a token id with the same number are different assets", () => {
    const contract = evmAddress(reads.polygon.creditsManager.address);
    const item = itemAsset(CHAIN.POLYGON, contract, 7n);
    const token = tokenAsset(CHAIN.POLYGON, contract, 7n);
    expect(assetKey(item)).not.toBe(assetKey(token));
    expect(JSON.parse(JSON.stringify(item))).toEqual({
      chainId: 137,
      contract: "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5",
      token: { kind: "item", itemId: "7" },
    });
    expect(parseAssetId(JSON.parse(JSON.stringify(token)))).toEqual(token);
    expect(parseAssetKey(assetKey(token))).toEqual(token);
    expect(assetKey(item)).toBe("137:0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5:item:7");
    expect(parseAssetId({ ...item, token: { kind: "item", itemId: "007" } })).toEqual(item);
    expect(parseAssetId({ ...item, token: { kind: "item", itemId: "-7" } })).toBeNull();
    expect(manaToWire(manaFromWhole(2))).toBe("2000000000000000000");
  });

  it("NAME labels are 2-15 ASCII letters or digits", () => {
    expect(parseNameLabel("a")).toBeNull();
    expect(parseNameLabel("dash-es")).toBeNull();
    expect(parseNameLabel("sixteencharacter")).toBeNull();
    expect(parseNameLabel("Nacho")).toBe("Nacho");
  });

  it("Ethereum pays native gas, Polygon relays", () => {
    expect(nativeGasRequired(CHAIN.ETHEREUM)).toBe(true);
    expect(nativeGasRequired(CHAIN.AMOY)).toBe(false);
  });
});

describe("wire decoding refuses what serde refuses", () => {
  const contract = evmAddress("0x214ffc0f0103735728dc66b61a22e4f163e275ae");
  const key: SettlementGroupKey = { method: "credits", chainId: 137, contract, call: { kind: "storeBuy" } };

  it("decodes the policy::Eligibility serde form", () => {
    expect(
      decodeEligibility({
        allowed: ["mana"],
        default: "mana",
        reasons: ["secondarySale"],
        guidance: { chainId: 1, nativeGasRequired: true, account: null },
      }),
    ).toEqual({ allowed: ["mana"], default: "mana", reasons: ["secondarySale"], guidance: { chainId: 1, nativeGasRequired: true, account: null } });
    expect(decodeEligibility({ allowed: ["cash"], default: null, reasons: [], guidance: null })).toBeNull();
    expect(decodeEligibility({ allowed: [], default: "mana", reasons: ["primary"], guidance: null })).toBeNull();
  });

  it("decodes a policy::SettlementGroup", () => {
    const group = decodeSettlementGroup({
      key,
      lines: [0, 2],
      quotes: [{ method: "credits", quoted: 115 }, { method: "credits", quoted: 201 }],
      total: { method: "credits", quoted: 316, charged: 330 },
      manaRemainderAllowed: true,
    });
    expect(group?.total).toEqual({ method: "credits", quoted: 316, charged: 330 });
    expect(settlementKeyId(group!.key)).toBe("credits|137|0x214ffc0f0103735728dc66b61a22e4f163e275ae|storeBuy");
    expect(decodeSettlementGroup({ key, lines: [0], quotes: [], total: { method: "mana", amount: "1" }, manaRemainderAllowed: false })).toBeNull();
  });

  it("round-trips a PurchaseOffer through JSON", () => {
    const offer = decodePurchaseOffer({
      asset: { chainId: 137, contract, token: { kind: "item", itemId: "7" } },
      kind: "wearable",
      sale: "primaryMint",
      bundle: false,
      eligibility: { allowed: ["credits", "mana"], default: "credits", reasons: ["creditsFirst", "manaSecond"], guidance: null },
      quotes: [{ method: "credits", quoted: 115 }, { method: "mana", amount: "2300000000000000000" }],
      unavailable: [],
      settlement: [{ method: "credits", key }],
      available: 3,
      quotedAt: 1,
    });
    expect(offer).not.toBeNull();
    expect(decodePurchaseOffer(JSON.parse(JSON.stringify(encodePurchaseOffer(offer!))))).toEqual(offer);
    expect(decodePurchaseOffer({ ...encodePurchaseOffer(offer!), sale: "primary" })).toBeNull();
    expect(
      decodePurchaseOffer({ ...encodePurchaseOffer(offer!), settlement: [{ method: "mana", key }] }),
      "a settlement key names its own method",
    ).toBeNull();
  });
});
