// from: shop@94584a5c:app/src/lib/urn.spec.ts
import { describe, it, expect } from "vitest"
import { itemIdFromUrn, peerUrlFor, urnNetwork } from "./urn"

const MATIC = "urn:decentraland:matic:collections-v2:0x90f3d8780f8e32c0f1f937edfc0ad930b2e7347f:0"
const AMOY = "urn:decentraland:amoy:collections-v2:0xba7f33d73aa04b81cbee3f2ecf95f5d442939d0e:12"
const BASE = "urn:decentraland:off-chain:base-avatars:cord_bracelet"

describe("the network a list of urns belongs to", () => {
  it("comes from the items, whichever position they are in", () => {
    expect(urnNetwork([MATIC])).toBe("matic")
    expect(urnNetwork([AMOY])).toBe("amoy")
  })

  /**
   * The case that left an outfit's wearables unrendered: with the shopper's own avatar composed in, the
   * list opens with their base wearables, and reading only the first urn resolved every dev item against
   * the mainnet catalyst, where none of them exist.
   */
  it("ignores the off-chain base wearables the try-on composition puts first", () => {
    expect(urnNetwork([BASE, BASE, AMOY, MATIC])).toBe("amoy")
    expect(urnNetwork([BASE, MATIC])).toBe("matic")
  })

  it("falls back to the caller's chain when nothing names a network", () => {
    expect(urnNetwork([BASE])).toBe("matic")
    expect(urnNetwork([], "amoy")).toBe("amoy")
  })
})

describe("the catalyst that holds a urn's content", () => {
  it("is the one the host names for the urns' network, else the host's base", () => {
    const node = { byNetwork: {}, base: "https://node.test/" }
    expect(peerUrlFor([MATIC], node)).toBe("https://node.test")
    const peers = { byNetwork: { amoy: "https://peer.decentraland.zone" }, base: "https://node.test" }
    expect(peerUrlFor([BASE, AMOY], peers)).toBe("https://peer.decentraland.zone")
    expect(peerUrlFor([MATIC], peers)).toBe("https://node.test")
  })
})

describe("itemIdFromUrn", () => {
  it("parses a collections-v2 item URN into a `contract-itemId` catalog id", () => {
    expect(itemIdFromUrn("urn:decentraland:matic:collections-v2:0x90f3d8780f8e32c0f1f937edfc0ad930b2e7347f:0")).toBe(
      "0x90f3d8780f8e32c0f1f937edfc0ad930b2e7347f-0"
    )
    expect(itemIdFromUrn("urn:decentraland:amoy:collections-v2:0xBA7F33D73AA04B81CBEE3F2ECF95F5D442939D0E:12")).toBe(
      "0xba7f33d73aa04b81cbee3f2ecf95f5d442939d0e-12"
    )
  })

  it("returns null for URNs that are not collections-v2 items", () => {
    expect(itemIdFromUrn("urn:decentraland:off-chain:base-avatars:BaseFemale")).toBeNull()
    expect(itemIdFromUrn("../OutfitStudio/Poses/Pose_15")).toBeNull()
    expect(itemIdFromUrn("")).toBeNull()
  })
})
