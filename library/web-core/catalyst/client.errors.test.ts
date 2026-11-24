import { describe, expect, it } from "vitest";

import { CatalystError, envelopeMessage, forwardFailure, sendJSON } from "./client";

const BASE = "https://node.example";

function refusal(status: number, body: string | null, statusText = "Bad Request"): Response {
  return new Response(body, {
    status,
    statusText,
    headers: { "content-type": "application/json" },
  });
}

describe("envelopeMessage", () => {
  it("reads the places and events {ok, error, code} envelope", () => {
    expect(envelopeMessage({ ok: false, error: "Invalid event id", code: "bad_request" })).toBe(
      "Invalid event id",
    );
  });

  it("prefers a host's message field and answers nothing for other shapes", () => {
    expect(envelopeMessage({ message: "Rate limited", error: "ignored" })).toBe("Rate limited");
    expect(envelopeMessage({ ok: false, error: 42 })).toBe("");
    expect(envelopeMessage(null)).toBe("");
    expect(envelopeMessage("text")).toBe("");
  });
});

describe("sendJSON", () => {
  it("throws the server's message from the {ok, error, code} envelope", async () => {
    const body = JSON.stringify({ ok: false, error: "You don't have permission to edit this event", code: "forbidden" });
    const fetchImpl = (async () => refusal(403, body, "Forbidden")) as unknown as typeof fetch;
    const failing = sendJSON("/api/events/1", { base: BASE, method: "PATCH", body: {}, fetchImpl });
    await expect(failing).rejects.toBeInstanceOf(CatalystError);
    await expect(failing).rejects.toMatchObject({
      message: "You don't have permission to edit this event",
      status: 403,
      url: `${BASE}/api/events/1`,
    });
  });

  it("falls back to the status line when the envelope carries no text", async () => {
    const fetchImpl = (async () => refusal(503, JSON.stringify({ ok: false }), "Service Unavailable")) as unknown as typeof fetch;
    await expect(sendJSON("/api/events", { base: BASE, body: {}, fetchImpl })).rejects.toMatchObject({
      message: "Catalyst returned 503 Service Unavailable",
      status: 503,
    });
  });
});

describe("forwardFailure", () => {
  it("carries the status, the body's retryAfter and its code", () => {
    const coded = forwardFailure("u", 503, JSON.stringify({ error: "Tip confirmation is unavailable on this node", code: "tip-index-unavailable" }));
    expect(coded).toBeInstanceOf(CatalystError);
    expect(coded).toMatchObject({ status: 503, code: "tip-index-unavailable", message: "Tip confirmation is unavailable on this node" });
    expect(coded.retryAfterSecs).toBeUndefined();
    const lagging = forwardFailure("u", 503, JSON.stringify({ error: "behind", retryAfter: 60 }));
    expect(lagging).toMatchObject({ status: 503, retryAfterSecs: 60 });
    expect(lagging.code).toBeUndefined();
  });

  it("keeps a body that is not JSON as the message, and names the status without one", () => {
    expect(forwardFailure("u", 502, "Bad Gateway")).toMatchObject({ status: 502, message: "Bad Gateway" });
    expect(forwardFailure("u", 503, "")).toMatchObject({ status: 503, message: "Catalyst returned 503" });
  });
});
