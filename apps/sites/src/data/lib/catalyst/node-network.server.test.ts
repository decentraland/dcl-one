import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let nodeNetworkId: typeof import("./node-network.server").nodeNetworkId;
let about: () => Response;
let reads: number;

beforeEach(async () => {
  vi.resetModules();
  vi.useFakeTimers();
  reads = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => {
      reads += 1;
      return about();
    }),
  );
  ({ nodeNetworkId } = await import("./node-network.server"));
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

const answering = (networkId: number) => () =>
  new Response(JSON.stringify({ configurations: { networkId } }), { headers: { "content-type": "application/json" } });

describe("when the server asks the node for its network", () => {
  describe("and the node answers", () => {
    beforeEach(() => {
      about = answering(11155111);
    });

    it("should give the network id of its about answer", async () => {
      await expect(nodeNetworkId()).resolves.toBe(11155111);
    });

    it("should ask the node only once", async () => {
      await nodeNetworkId();
      await nodeNetworkId();
      expect(reads).toBe(1);
    });
  });

  describe("and the node fails", () => {
    beforeEach(() => {
      about = () => new Response("down", { status: 503 });
    });

    it("should give no network", async () => {
      await expect(nodeNetworkId()).resolves.toBeUndefined();
    });

    it("should not ask again for ten seconds, then learn the network once the node answers", async () => {
      await nodeNetworkId();
      const failedReads = reads;
      about = answering(1);
      await expect(nodeNetworkId()).resolves.toBeUndefined();
      expect(reads).toBe(failedReads);
      vi.advanceTimersByTime(10_000);
      await expect(nodeNetworkId()).resolves.toBe(1);
    });
  });
});
