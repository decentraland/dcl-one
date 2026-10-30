import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { FakeBridge } from "../../test/fakeBridge";
import { fetchLiveNotifications, markNotificationsRead } from "./notifications";
import { parseNotificationsLoose } from "./notificationsView";

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
