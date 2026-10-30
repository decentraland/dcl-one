import { expect, test, vi } from "vitest";
import { acceptFriendRequest, sendFriendRequest } from "./friendActions";
import { sendBridge } from "../../overlay/bridge";

const listeners = new Set<(push: unknown) => void>();
vi.mock("../../overlay/bridge", () => ({
  sendBridge: vi.fn(),
  getBridge: () => ({}),
  subscribeBridge: (listener: (push: unknown) => void) => { listeners.add(listener); return () => listeners.delete(listener); },
}));
const push = (state: unknown) => { for (const listener of [...listeners]) listener(state); };
const target = "0x00000000000000000000000000000000000000Ab";

test("accepting settles only once the engine lists the explorer as a friend", async () => {
  let settled = false;
  const accepted = acceptFriendRequest(target).then(() => { settled = true; });
  expect(sendBridge).toHaveBeenCalledWith("SignRequest", { kind: "upsert_friendship", action: "accept", address: target.toLowerCase() });
  push({ kind: "friends", received: [{ friend: { address: target } }] });
  await Promise.resolve();
  expect(settled).toBe(false);
  push({ kind: "friends", friends: [{ address: target }], received: [] });
  await accepted;
  expect(settled).toBe(true);
});

test("a request settles once the engine lists it as sent", async () => {
  const sent = sendFriendRequest(target);
  expect(sendBridge).toHaveBeenLastCalledWith("SignRequest", { kind: "upsert_friendship", action: "request", address: target.toLowerCase() });
  push({ kind: "friends", sent: [{ friend: { address: target.toLowerCase() } }] });
  await expect(sent).resolves.toBeUndefined();
});
