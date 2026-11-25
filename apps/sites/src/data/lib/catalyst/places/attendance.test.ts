import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const http = vi.hoisted(() => ({ signedGetJSON: vi.fn() }));
vi.mock("../client", async (actual) => ({
  ...(await actual<typeof import("../client")>()),
  signedGetJSON: http.signedGetJSON,
}));

import type { AuthIdentity } from "../../auth/types";
import { attendanceKeys, attendingListQuery } from "./attendance";

describe("attendingListQuery", () => {
  let identity: AuthIdentity;
  let client: QueryClient;

  beforeEach(() => {
    identity = { signer: "0xAB" } as unknown as AuthIdentity;
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  describe("when the endpoint answers every unfinished event with the attending flag", () => {
    let result: Promise<string[]>;

    beforeEach(() => {
      http.signedGetJSON.mockResolvedValueOnce({
        ok: true,
        data: [
          { id: "going", name: "Going", attending: true },
          { id: "not", name: "Not going", attending: false },
        ],
      });
      result = client.fetchQuery(attendingListQuery(identity));
    });

    it("should keep only the events the member attends", async () => {
      await expect(result).resolves.toEqual(["going"]);
    });

    it("should read the attending endpoint", async () => {
      await result;
      expect(http.signedGetJSON.mock.calls[0]?.[0]).toBe("/events/api/events/attending");
    });
  });

  describe("when a row has no attending flag", () => {
    beforeEach(() => {
      http.signedGetJSON.mockResolvedValueOnce({ ok: true, data: [{ id: "x" }] });
    });

    it("should reject the row rather than guess it", async () => {
      await expect(client.fetchQuery(attendingListQuery(identity))).rejects.toThrow(/attending/);
    });
  });

  it("should keep the list under the member's private prefix", () => {
    expect(attendanceKeys.list("0xAB")).toEqual(["private", "0xab", "attendance", "list"]);
  });
});
