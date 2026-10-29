// from: shop@94584a5c:app/src/lib/network.ts
// Upstream's network helpers under their names, on the foundation's EIP-1193 seam (wallet/chain.ts):
// read the chain, require it before a gas-paying leg, switch only from the buyer's own click. Adds the
// unauthorized-request classifier, which the foundation left to this area.

export { chainLabel } from "../../model/chain";
export {
  AMOY_ADD_PARAMS,
  isWrongNetworkError,
  readChainId as activeChainId,
  requestChainSwitch as switchChain,
  requireChain,
  WrongNetworkError,
} from "../../wallet/chain";

/**
 * Did the WALLET refuse the request as unauthorized (-32006 / 4100, or an HTTP 401 inside a provider
 * error)? Never a contract problem, though providers dress it up as a revert. Matched on codes only,
 * walking the nested errors wallets stack up; a message match would also catch our own API's 401s.
 */
export function isWalletUnauthorizedError(e: unknown, depth = 0): boolean {
  if (!e || typeof e !== "object" || depth > 4) return false;
  const err = e as { code?: unknown; data?: { httpStatus?: number }; error?: unknown; cause?: unknown };
  if (err.code === -32006 || err.code === 4100) return true;
  if (err.data?.httpStatus === 401) return true;
  return isWalletUnauthorizedError(err.error, depth + 1) || isWalletUnauthorizedError(err.cause, depth + 1);
}
