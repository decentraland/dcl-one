// from: marketplace@f3956826:webapp/src/modules/rental/contract.ts, webapp/src/modules/rental/utils.ts (the
// nonce reads), catalyrst-v2 sites/src/data/lib/catalyst/marketplace/name-claim.ts (transaction + receipt)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// A contract read on whatever answers `eth_call`, with viem for the ABI. Which answerer, the node's read-only relay
// or the buyer's wallet, is the host's (ui3: land/evm.ts).

import { decodeFunctionResult, encodeFunctionData, type Abi, type Hex } from "viem";

/** Something that answers `eth_call`: the node's relay, or a connected wallet. */
export type ChainReader = { chainId: number; call(to: string, data: Hex, signal?: AbortSignal): Promise<Hex> };

/** One `view` call, ABI-encoded and decoded by viem; `T` is what the function returns. */
export async function readContract<T>(
  reader: ChainReader,
  address: string,
  abi: Abi,
  functionName: string,
  args: readonly unknown[],
  signal?: AbortSignal,
): Promise<T> {
  const data = encodeFunctionData({ abi, functionName, args } as never);
  const raw = await reader.call(address, data, signal);
  return decodeFunctionResult({ abi, functionName, data: raw } as never) as T;
}
