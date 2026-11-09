import { describe, expect, it } from "vitest";

import { CatalystError, errorFromResponse } from "./client";

const URL = "https://node.example/api/events/not-a-uuid";

function refusal(status: number, body: string | null, statusText = "Bad Request"): Response {
  return new Response(body, {
    status,
    statusText,
    headers: { "content-type": "application/json" },
  });
}

describe("errorFromResponse", () => {
  it("reads the places and events {ok, error, code} envelope", async () => {
    const body = JSON.stringify({ ok: false, error: "Invalid event id", code: "bad_request" });
    const err = await errorFromResponse(refusal(400, body), URL);
    expect(err).toBeInstanceOf(CatalystError);
    expect(err.message).toBe("Invalid event id");
    expect(err.status).toBe(400);
    expect(err.url).toBe(URL);
    expect(err.serverMessage).toBe(true);
  });

  it("keeps reading a host's message field", async () => {
    const body = JSON.stringify({ message: "Rate limited", error: "ignored" });
    const err = await errorFromResponse(refusal(429, body, "Too Many Requests"), URL);
    expect(err.message).toBe("Rate limited");
    expect(err.serverMessage).toBe(true);
  });

  it("falls back to the status line when the envelope carries no text", async () => {
    for (const body of [null, "", "not json", JSON.stringify({ ok: false, error: "  ", code: "bad_request" })]) {
      const err = await errorFromResponse(refusal(503, body, "Service Unavailable"), URL);
      expect(err.message).toBe("Catalyst returned 503 Service Unavailable");
      expect(err.status).toBe(503);
      expect(err.serverMessage).toBe(false);
    }
  });
});
