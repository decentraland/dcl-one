import { describe, expect, it } from "vitest";

import {
  notificationBody,
  notificationImage,
  notificationLink,
  notificationMillis,
  notificationTitle,
  parseNotificationsLoose,
  relativeTime,
  unreadCount,
  type Notification,
} from "./notificationsView";

const row = (over: Partial<Notification> = {}): Notification => ({
  id: "n1",
  type: "some_new_type",
  address: "0x1111111111111111111111111111111111111111",
  timestamp: 0,
  read: false,
  created_at: "",
  updated_at: "",
  metadata: {},
  ...over,
});

describe("parseNotificationsLoose", () => {
  it("reads a bare array or a { notifications } envelope, newest first, with defaults for loose fields", () => {
    const rows = [
      { id: "a", type: "t", timestamp: "100", metadata: [1] },
      { id: "b", type: "t", timestamp: 300, read: true, address: "0xabc", metadata: { title: "x" } },
    ];
    const expected = [
      { id: "b", type: "t", address: "0xabc", timestamp: 300, read: true, created_at: "", updated_at: "", metadata: { title: "x" } },
      { id: "a", type: "t", address: "", timestamp: 100, read: false, created_at: "", updated_at: "", metadata: {} },
    ];
    expect(parseNotificationsLoose(rows)).toEqual(expected);
    expect(parseNotificationsLoose({ notifications: rows })).toEqual(expected);
  });

  it("drops rows without a string id and type, and anything that is not a list", () => {
    expect(parseNotificationsLoose([null, 1, { id: "", type: "t" }, { id: "x" }, { id: "y", type: 2 }])).toEqual([]);
    expect(parseNotificationsLoose(undefined)).toEqual([]);
    expect(parseNotificationsLoose({ notifications: "no" })).toEqual([]);
  });
});

describe("notificationMillis", () => {
  it("accepts finite numbers and digit strings only", () => {
    expect(notificationMillis(5)).toBe(5);
    expect(notificationMillis("42")).toBe(42);
    expect(notificationMillis(Number.NaN)).toBeNull();
    expect(notificationMillis("4.2")).toBeNull();
    expect(notificationMillis(null)).toBeNull();
  });
});

describe("relativeTime", () => {
  const now = 10 * 24 * 60 * 60 * 1000;
  it.each([
    [now - 30_000, "Just Now"],
    [now + 60_000, "Just Now"],
    [now - 5 * 60_000, "5m"],
    [now - 3 * 60 * 60_000, "3h"],
    [now - 24 * 60 * 60_000, "Yesterday"],
    [now - 3 * 24 * 60 * 60_000, "3d"],
  ])("%d gives %s", (timestamp, label) => {
    expect(relativeTime(timestamp, now)).toBe(label);
  });
});

describe("the row view", () => {
  it("prefers the server's title and description, else humanizes the type", () => {
    expect(notificationTitle(row({ metadata: { title: "Hello", description: "World" } }))).toBe("Hello");
    expect(notificationBody(row({ metadata: { title: "Hello", description: "World" } }))).toBe("World");
    expect(notificationTitle(row({ metadata: { title: "  " } }))).toBe("Some New Type");
    expect(notificationBody(row())).toBe("");
    expect(notificationTitle(null)).toBe("");
  });

  it("keeps only http(s) and same-site path links", () => {
    expect(notificationLink(row({ metadata: { link: "https://decentraland.org/x" } }))).toBe("https://decentraland.org/x");
    expect(notificationLink(row({ metadata: { link: "/shop/item/1?x=2" } }))).toBe("/shop/item/1?x=2");
    expect(notificationLink(row({ metadata: { link: "//evil.example/x" } }))).toBeNull();
    expect(notificationLink(row({ metadata: { link: "javascript:alert(1)" } }))).toBeNull();
    expect(notificationLink(row())).toBeNull();
  });

  it("takes the first non-blank image field in order", () => {
    expect(notificationImage(row({ metadata: { image: "i", thumbnailUrl: "t", badgeImageUrl: " " } }))).toBe("t");
    expect(notificationImage(row())).toBeNull();
  });

  it("counts unread rows", () => {
    expect(unreadCount([row(), row({ read: true }), row()])).toBe(2);
    expect(unreadCount(null)).toBe(0);
  });
});
