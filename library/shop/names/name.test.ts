import { decodeFunctionData, encodeFunctionResult } from "viem";
import { describe, expect, it } from "vitest";

import { LAND_NETWORKS } from "../land/contracts";
import type { ChainReader } from "../land/read";
import { CHAIN } from "../model/chain";
import { checkNameAvailability, registrarAbi } from "./name";

/** A reader on `chainId` whose registrar answers `available(name)` with `free(name)`, running `beforeAnswer` first. */
function registrarReader(chainId: number, free: (name: string) => boolean, beforeAnswer: () => void = () => {}) {
  const asked: { to: string; name: string }[] = [];
  const reader: ChainReader = {
    chainId,
    async call(to, data) {
      const name = String(decodeFunctionData({ abi: registrarAbi, data }).args[0]);
      asked.push({ to, name });
      beforeAnswer();
      return encodeFunctionResult({ abi: registrarAbi, functionName: "available", result: free(name) });
    },
  };
  return { reader, asked };
}

describe("a NAME's availability", () => {
  it("asks the registrar of the given chain through the given reader", async () => {
    const { reader, asked } = registrarReader(CHAIN.SEPOLIA, (name) => name !== "taken");
    expect(await checkNameAvailability("free", { reader, chainId: CHAIN.SEPOLIA })).toBe("available");
    expect(await checkNameAvailability("taken", { reader, chainId: CHAIN.SEPOLIA })).toBe("taken");
    const registrar = LAND_NETWORKS[CHAIN.SEPOLIA]!.nameRegistrar;
    expect(asked).toEqual([
      { to: registrar, name: "free" },
      { to: registrar, name: "taken" },
    ]);
  });

  it("refuses as an abort a read that answers after its caller aborted", async () => {
    const ctrl = new AbortController();
    const { reader } = registrarReader(CHAIN.ETHEREUM, () => true, () => ctrl.abort());
    const read = checkNameAvailability("free", { reader, chainId: CHAIN.ETHEREUM, signal: ctrl.signal });
    await expect(read).rejects.toBeInstanceOf(Error);
    await expect(read).rejects.toMatchObject({ name: "AbortError" });
  });
});
