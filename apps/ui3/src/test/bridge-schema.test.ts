import { describe, expect, test } from "vitest";
import { OverlayPushSchema } from "../generated/bridge-schemas";

const request = (createdAt: unknown) => ({
  id: "44893230-3207-46bd-8ae8-b6cb632366fe",
  createdAt,
  message: null,
  friend: { address: "0x41623faf931cbf93dd7ad5d2303d04494c2f264b", name: "", hasClaimedName: false, profilePictureUrl: "" },
});
const friendsPush = (lists: { sent?: unknown[]; received?: unknown[] }) =>
  JSON.parse(JSON.stringify({ kind: "friends", onlineCount: 0, friends: [], received: [], sent: [], blocked: [], blockedByMe: [], ...lists }));

describe("bridge push validation", () => {
  test("accepts what the engine sends and rejects renamed fields, wrong types, unknown kinds and missing required fields", () => {
    const cases: [string, unknown, boolean][] = [
      ["valid identity", { kind: "identity", address: "0x1", signerAddress: "0x2", isGuest: false }, true],
      ["renamed field", { kind: "identity", addr: "0x1", signerAddress: "0x2", isGuest: false }, false],
      ["wrong type", { kind: "identity", address: 42, signerAddress: "0x2", isGuest: false }, false],
      ["unknown kind", { kind: "totallyNew", x: 1 }, false],
      ["chat missing timestamp", { kind: "chat", senderName: "a", senderAddress: "0x1", message: "hi", channel: "n" }, false],
      ["valid permission withdrawn", { kind: "permissionWithdrawn", id: 3 }, true],
      ["permission withdrawn without an id", { kind: "permissionWithdrawn" }, false],
      ["permission withdrawn with a string id", { kind: "permissionWithdrawn", id: "3" }, false],
      ["friends with a sent request", friendsPush({ sent: [request(1790807652815)] }), true],
      ["friends with a received request", friendsPush({ received: [request(1790807652815)] }), true],
      ["friends with a string request time", friendsPush({ sent: [request("1790807652815")] }), false],
    ];
    const verdicts = cases.map(([name, value]) => [name, OverlayPushSchema.safeParse(value).success]);
    expect(verdicts).toEqual(cases.map(([name, , shouldPass]) => [name, shouldPass]));
  });
});
