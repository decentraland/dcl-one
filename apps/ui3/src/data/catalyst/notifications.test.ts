import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { FakeBridge } from "../../test/fakeBridge";
import { fetchLiveNotifications, markNotificationsRead } from "./notifications";
import {
  actorAddress,
  categoryForType,
  notificationBody,
  notificationImage,
  notificationLink,
  notificationTitle,
  parseNotificationsLoose,
  type Notification,
} from "./notificationsView";

type SignedFetchSent = { id: string; url: string; method: string };

function installBridge(): FakeBridge {
  const bridge = new FakeBridge();
  window.dclBridge = bridge;
  return bridge;
}

function lastSignedFetch(bridge: FakeBridge): SignedFetchSent {
  const sent = bridge.sent.filter((c) => c.action === "SignedFetch");
  return sent.at(-1)!.payload as SignedFetchSent;
}

afterEach(() => {
  delete window.dclBridge;
});

it("reads and marks the inbox on the node's /notifications mount", async () => {
  const bridge = installBridge();
  const list = fetchLiveNotifications();
  const read = lastSignedFetch(bridge);
  expect(new URL(read.url).pathname).toBe("/notifications/notifications");
  expect(read.method).toBe("GET");
  bridge.push({ kind: "signedFetchResult", id: read.id, status: 200, body: JSON.stringify({ notifications: [] }) });
  await expect(list).resolves.toEqual([]);

  const mark = markNotificationsRead(["n1"]);
  const put = lastSignedFetch(bridge);
  expect(new URL(put.url).pathname).toBe("/notifications/notifications/read");
  expect(put.method).toBe("PUT");
  bridge.push({ kind: "signedFetchResult", id: put.id, status: 204, body: "" });
  await expect(mark).resolves.toBeUndefined();
});

const nodeResponse = readFileSync(join(__dirname, "../../../../../services/catalyst/tests/fixtures/notifications/list-response.json"), "utf8");

it("keeps every row of a real node inbox, with its string timestamp as epoch ms", async () => {
  const bridge = installBridge();
  const list = fetchLiveNotifications();
  bridge.push({ kind: "signedFetchResult", id: lastSignedFetch(bridge).id, status: 200, body: nodeResponse });
  const rows = await list;
  expect(rows.map((n) => [n.id, n.type, n.timestamp, n.read])).toEqual([
    ["00000000-0000-0000-0000-000000000001", "badge_granted", 1699999999999, false],
  ]);
  expect(rows[0]!.metadata).toEqual({ badgeId: "welcome" });
  expect(parseNotificationsLoose(JSON.parse(nodeResponse)).map((n) => n.timestamp)).toEqual([1699999999999]);
});

it("keeps a broadcast row, whose address is null", async () => {
  const bridge = installBridge();
  const list = fetchLiveNotifications();
  const broadcast = { id: "b1", type: "governance_announcement", address: null, timestamp: "1700000000000", read: false, created_at: "", updated_at: "", metadata: { title: "t" } };
  bridge.push({ kind: "signedFetchResult", id: lastSignedFetch(bridge).id, status: 200, body: JSON.stringify({ notifications: [broadcast] }) });
  const rows = await list;
  expect(rows.map((n) => [n.id, n.address, n.timestamp])).toEqual([["b1", "", 1700000000000]]);
});

it("names no actor for a broadcast, even when its metadata holds an address", () => {
  const other = "0x00000000000000000000000000000000000000aa";
  const row = (address: string): Notification => ({
    id: "b1", type: "governance_announcement", address, timestamp: 1, read: false, created_at: "", updated_at: "",
    metadata: { title: "t", link: `https://example.org/${other}`, author: other },
  });
  expect(actorAddress(row(""))).toBeNull();
  expect(actorAddress(row("0x0000000000000000000000000000000000000001"))).toBe(other);
});

it("shows a favorite on sale as a marketplace notice with its server copy, image and shop link", () => {
  const base: Notification = {
    id: "d1", type: "item_discounted", address: "0x0000000000000000000000000000000000000001", timestamp: 1,
    read: false, created_at: "", updated_at: "",
    metadata: {
      title: "A favorite is on sale", description: "Hat is 25% off.", image: "https://example.com/t.png",
      link: "https://example.com/shop/item/0x0000000000000000000000000000000000000002/7", nftName: "Hat", discountPct: 25,
    },
  };
  expect(categoryForType(base.type)).toBe("marketplace");
  expect([notificationTitle(base), notificationBody(base), notificationImage(base), notificationLink(base)]).toEqual([
    "A favorite is on sale", "Hat is 25% off.", "https://example.com/t.png", "https://example.com/shop/item/0x0000000000000000000000000000000000000002/7",
  ]);
  const bare = { ...base, metadata: { nftName: "Hat", discountPct: 25 } };
  expect([notificationTitle(bare), notificationBody(bare)]).toEqual(["A favorite is on sale", "Hat is 25% off."]);
});
