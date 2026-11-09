import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { collectionOfItem, creatorOfProfile } from "./discover.mts";

const CONTRACT = "0x68D953c22C43B03d2F33f7C33a2E2feA45D90d26";
const CREATOR = "0x5985Eb4A8E0e1f7bcA9cC0D7aE81c2943Fb205bD";

describe("discover", () => {
  it("takes an item's collection from its contract, lowercased", () => {
    assert.equal(collectionOfItem(`/shop/item/${CONTRACT}/0`), `/shop/collection/${CONTRACT.toLowerCase()}`);
  });

  it("finds no collection outside an item's path", () => {
    assert.equal(collectionOfItem(`/shop/collection/${CONTRACT}`), null);
    assert.equal(collectionOfItem(`/shop/item/0x1234/0`), null);
    assert.equal(collectionOfItem(`/other/shop/item/${CONTRACT}/0`), null);
  });

  it("takes a creator from a profile link, relative or absolute, lowercased", () => {
    const page = `/shop/items/creator/${CREATOR.toLowerCase()}`;
    assert.equal(creatorOfProfile(`/profile/${CREATOR}`), page);
    assert.equal(creatorOfProfile(`https://decentraland.org/profile/${CREATOR}/assets?x=1`), page);
  });

  it("finds no creator in a link that holds no whole address", () => {
    assert.equal(creatorOfProfile("/profile/0xabc"), null);
    assert.equal(creatorOfProfile(`/profile/${CREATOR}ff`), null);
    assert.equal(creatorOfProfile(`/creator/${CREATOR}`), null);
  });
});
