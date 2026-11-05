import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { journeyEnv, parseDiscount, parseEnvFile, parseItem, parseToken } from "./env.mts";

describe("parseEnvFile", () => {
  it("reads KEY=value lines, skipping comments and export, stripping quotes", () => {
    assert.deepEqual(parseEnvFile("# wallets\nexport A=1\nB = 'two'\nC=\"x=y\"\n\nnot a line\n"), { A: "1", B: "two", C: "x=y" });
  });
});

describe("journeyEnv", () => {
  it("lays the environment over the file E2E_ENV_FILE names", () => {
    const file = join(mkdtempSync(join(tmpdir(), "journey-")), "e2e.env");
    writeFileSync(file, "E2E_PRICE=5\nE2E_BID=3\n");
    const env = journeyEnv({ E2E_ENV_FILE: file, E2E_PRICE: "7" });
    assert.equal(env.E2E_PRICE, "7");
    assert.equal(env.E2E_BID, "3");
  });
});

describe("parseToken", () => {
  it("splits contract:tokenId and lowercases the contract", () => {
    assert.deepEqual(parseToken("0xABCDEF0123456789abcdef0123456789ABCDEF01:42"), { contract: "0xabcdef0123456789abcdef0123456789abcdef01", tokenId: "42" });
    assert.throws(() => parseToken("0x1234:42"), /E2E_TOKEN/);
    assert.throws(() => parseToken(undefined), /E2E_TOKEN/);
  });
});

describe("parseItem", () => {
  it("splits collection:itemId and lowercases the collection", () => {
    assert.deepEqual(parseItem("0xABCDEF0123456789abcdef0123456789ABCDEF01:0"), { contract: "0xabcdef0123456789abcdef0123456789abcdef01", itemId: "0" });
    assert.throws(() => parseItem("0xabcdef0123456789abcdef0123456789abcdef01"), /E2E_ITEM/);
    assert.throws(() => parseItem(undefined), /E2E_ITEM/);
  });
});

describe("parseDiscount", () => {
  it("defaults to 10 and keeps to the Shop's 5..70 whole percentages", () => {
    assert.equal(parseDiscount(undefined), 10);
    assert.equal(parseDiscount("70"), 70);
    for (const bad of ["4", "71", "12.5", "ten"]) assert.throws(() => parseDiscount(bad), /E2E_DISCOUNT/, bad);
  });
});
