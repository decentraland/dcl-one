import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const http = vi.hoisted(() => ({ signedGetJSON: vi.fn() }));
vi.mock("../client", async (actual) => ({ ...(await actual<typeof import("../client")>()), signedGetJSON: http.signedGetJSON }));

import type { AuthIdentity } from "../../auth/types";
import { attendanceKeys, attendingListQuery } from "./attendance";

const identity = { signer: "0xAB" } as unknown as AuthIdentity;

beforeEach(() => http.signedGetJSON.mockReset());

describe("attendingListQuery", () => {
  it("should keep only the events the member attends, out of every unfinished event the endpoint answers", async () => {
    http.signedGetJSON.mockResolvedValue({
      ok: true,
      data: [
        { id: "going", name: "Going", attending: true },
        { id: "not", name: "Not going", attending: false },
      ],
    });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    await expect(client.fetchQuery(attendingListQuery(identity))).resolves.toEqual(["going"]);
    expect(http.signedGetJSON.mock.calls[0]?.[0]).toBe("/events/api/events/attending");
  });

  it("should reject a row without the attending flag rather than guess it", async () => {
    http.signedGetJSON.mockResolvedValue({ ok: true, data: [{ id: "x" }] });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    await expect(client.fetchQuery(attendingListQuery(identity))).rejects.toThrow();
  });

  it("should keep the list under the member's private prefix", () => {
    expect(attendanceKeys.list("0xAB")).toEqual(["private", "0xab", "attendance", "list"]);
  });
});
