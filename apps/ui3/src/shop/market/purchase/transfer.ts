// from: shop@2d3bf62:app/src/lib/buy.ts (transferItem, transferViaMetaTransaction);
//       marketplace@3bf3662f:webapp/src/modules/nft/sagas.ts (TRANSFER_NFT)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { encodeFunctionData } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import { parseEvmAddress, sameAddress } from "../../model/address";
import { isPolygon, type ChainId } from "../../model/chain";
import { collectionV2, ERC721_ABI } from "../lib/contracts";
import { MetaTransactionError, sendMetaTransaction } from "../lib/metaTx";
import { declined, isTxPending, sendCall, waitForReceipt, walletAccount, type WaitOptions } from "../lib/tx";

const ZERO = "0x0000000000000000000000000000000000000000";

export type TransferProblem = "invalidAddress" | "sameAddress" | "zeroAddress";

export function transferProblem(to: string, from: string | null): TransferProblem | null {
  const address = parseEvmAddress(to.trim());
  if (!address) return "invalidAddress";
  if ((address as string) === ZERO) return "zeroAddress";
  if (from && sameAddress(address, from)) return "sameAddress";
  return null;
}

export async function transferItem(opts: {
  provider: Eip1193Provider;
  contractAddress: string;
  chainId: ChainId;
  tokenId: string;
  to: string;
  collection?: boolean;
  gasless?: boolean;
  onSubmitted?: (hash: string) => void;
  wait?: WaitOptions;
}): Promise<string> {
  const from = await walletAccount(opts.provider);
  const problem = transferProblem(opts.to, from);
  if (problem) throw new Error(problem === "sameAddress" ? "You already own this." : "Enter a valid address to send it to.");
  const to = opts.to.trim().toLowerCase() as `0x${string}`;
  const args = [from as `0x${string}`, to, BigInt(opts.tokenId)] as const;
  if (isPolygon(opts.chainId) && (opts.collection ?? true) && opts.gasless !== false) {
    try {
      const hash = await sendMetaTransaction({
        provider: opts.provider,
        contract: collectionV2(opts.chainId, opts.contractAddress),
        kind: "collection",
        functionData: encodeFunctionData({ abi: ERC721_ABI, functionName: "transferFrom", args }),
      });
      opts.onSubmitted?.(hash);
      return await waitForReceipt(opts.chainId, hash, "The transfer", opts.wait);
    } catch (e) {
      if (declined(e) || isTxPending(e) || !(e instanceof MetaTransactionError)) throw e;
    }
  }
  const hash = await sendCall(opts.provider, { chainId: opts.chainId, to: opts.contractAddress, abi: ERC721_ABI, functionName: "transferFrom", args });
  opts.onSubmitted?.(hash);
  return waitForReceipt(opts.chainId, hash, "The transfer", opts.wait);
}
