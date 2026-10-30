import { getBridge, sendBridge, subscribeBridge } from "../../overlay/bridge";
import type { OverlayPush } from "../../generated/bridge/OverlayPush";

type FriendsState = Extract<OverlayPush, { kind: "friends" }>;
const lists = (target: string, entries: { address: string }[] | undefined) => !!entries?.some(entry => entry.address.toLowerCase() === target);

function friendAction(action: string, address: string, done: (state: FriendsState, target: string) => boolean): Promise<void> {
  if (!/^0x[0-9a-f]{40}$/i.test(address)) return Promise.reject(new Error("Enter a valid wallet address."));
  if (!getBridge()) return Promise.reject(new Error("Not connected. Try again."));
  const target = address.toLowerCase();
  return new Promise((resolve, reject) => {
    let settled = false;
    let unsubscribe = () => {};
    const finish = (error?: Error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      unsubscribe();
      if (error) reject(error); else resolve();
    };
    const timer = setTimeout(() => finish(new Error("No reply from the server. Try again.")), 10000);
    unsubscribe = subscribeBridge((push) => {
      const state = push as FriendsState | null;
      if (state?.kind === "friends" && done(state, target)) finish();
    });
    if (settled) unsubscribe();
    else requestFriendAction(action, target);
  });
}

export function sendFriendRequest(address: string): Promise<void> {
  return friendAction("request", address, (state, target) => lists(target, state.sent?.map(request => request.friend)) || lists(target, state.friends));
}

export function acceptFriendRequest(address: string): Promise<void> {
  return friendAction("accept", address, (state, target) => lists(target, state.friends));
}

export const FRIEND_ACTIONS = Object.freeze({
  REQUEST: "request",
  ACCEPT: "accept",
  CANCEL: "cancel",
  REJECT: "reject",
  DELETE: "delete",
  BLOCK: "block",
  UNBLOCK: "unblock",
});

export function requestFriendAction(
  action: string,
  address: string,
  extra: { message?: string } = {},
): boolean {
  const valid = (Object.values(FRIEND_ACTIONS) as string[]).includes(action);
  if (!valid) {
    if (typeof console !== "undefined") {
      console.warn(`[friends] ignored unknown action: ${action}`);
    }
    return false;
  }
  sendBridge("SignRequest", {
    kind: "upsert_friendship",
    action,
    address,
    ...extra,
  });
  return true;
}
