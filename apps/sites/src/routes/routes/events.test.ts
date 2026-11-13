import { expect, test } from "vitest";
import { loader } from "./events";
import { loader as legacyEvent } from "./events_.event";

test("legacy public events links keep their filters when redirected", () => {
  const response = loader({ request: new Request("https://example.org/events/?search=music&category=art") });
  expect(response.status).toBe(302);
  expect(response.headers.get("Location")).toBe("/whats-on?search=music&category=art");
});

test("an event link of the events page opens the event's page", () => {
  const response = loader({ request: new Request("https://example.org/events?id=abc%20d") });
  expect(response.headers.get("Location")).toBe("/whats-on/abc%20d");
});

test("the legacy event link opens the event's page", () => {
  expect(legacyEvent({ request: new Request("https://example.org/events/event?id=e1") }).headers.get("Location")).toBe("/whats-on/e1");
  expect(legacyEvent({ request: new Request("https://example.org/events/event") }).headers.get("Location")).toBe("/whats-on");
});
