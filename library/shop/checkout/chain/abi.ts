// from: shop@94584a5c:app/src/lib/trade-encoding.ts (TRADE_TUPLE_ARRAY, COUPON_TUPLE_ARRAY, ITEM_TO_BUY_TUPLE_ARRAY),
//       app/src/lib/authorizations.ts (ERC20_ABI, ERC721_ABI, COLLECTION_MINTER_ABI), app/src/lib/mana-rate.ts,
//       app/src/lib/buy-gasless.ts (getNonce, executeMetaTransaction)
// The contract fragments the purchase rails call, as viem ABIs. Only what the Shop calls is here; the
// selectors are checked against tests/fixtures/shop/offchain-marketplace-contract@87350c63e/derived/selectors.json
// and chain-reads.json by chain.test.ts.

import { parseAbi } from "viem";

const STRUCTS = [
  "struct ExternalCheck { address contractAddress; bytes4 selector; bytes value; bool required; }",
  "struct Checks { uint256 uses; uint256 expiration; uint256 effective; bytes32 salt; uint256 contractSignatureIndex; uint256 signerSignatureIndex; bytes32 allowedRoot; bytes32[] allowedProof; ExternalCheck[] externalChecks; }",
  "struct Asset { uint256 assetType; address contractAddress; uint256 value; address beneficiary; bytes extra; }",
  "struct Trade { address signer; bytes signature; Checks checks; Asset[] sent; Asset[] received; }",
  "struct Coupon { bytes signature; Checks checks; address couponAddress; bytes data; bytes callerData; }",
] as const;

export const MARKETPLACE_ABI = parseAbi([
  ...STRUCTS,
  "function accept(Trade[] _trades)",
  "function acceptWithCoupon(Trade[] _trades, Coupon[] _coupons)",
  "function cancelSignature(Trade[] _trades)",
  "function manaUsdAggregator() view returns (address)",
  "function getNonce(address _signer) view returns (uint256)",
]);

export const CREDITS_MANAGER_ABI = parseAbi([
  "struct Credit { uint256 value; uint256 expiresAt; bytes32 salt; }",
  "struct ExternalCall { address target; bytes4 selector; bytes data; uint256 expiresAt; bytes32 salt; }",
  "struct UseCreditsArgs { Credit[] credits; bytes[] creditsSignatures; ExternalCall externalCall; bytes customExternalCallSignature; uint256 maxUncreditedValue; uint256 maxCreditedValue; }",
  "function useCredits(UseCreditsArgs _args)",
  "function getNonce(address _signer) view returns (uint256)",
]);

export const COLLECTION_STORE_ABI = parseAbi([
  "struct ItemToBuy { address collection; uint256[] ids; uint256[] prices; address[] beneficiaries; }",
  "function buy(ItemToBuy[] _itemsToBuy)",
  "function getNonce(address user) view returns (uint256)",
]);

/** executeMetaTransaction of the off-chain marketplace family (CreditsManager, marketplaces). */
export const OFFCHAIN_META_TX_ABI = parseAbi([
  "function executeMetaTransaction(address _userAddress, bytes _functionData, bytes _signature) payable returns (bytes)",
  "function getNonce(address _signer) view returns (uint256)",
]);

/** executeMetaTransaction of the original Decentraland NativeMetaTransaction (MANA, CollectionStore, collections). */
export const NATIVE_META_TX_ABI = parseAbi([
  "function executeMetaTransaction(address userAddress, bytes functionSignature, bytes32 sigR, bytes32 sigS, uint8 sigV) payable returns (bytes)",
  "function getNonce(address user) view returns (uint256 nonce)",
]);

export const ERC20_ABI = parseAbi([
  "function balanceOf(address owner) view returns (uint256)",
  "function allowance(address owner, address spender) view returns (uint256)",
  "function approve(address spender, uint256 amount) returns (bool)",
]);

export const ERC721_ABI = parseAbi([
  "function isApprovedForAll(address owner, address operator) view returns (bool)",
  "function setApprovalForAll(address operator, bool approved)",
  "function transferFrom(address from, address to, uint256 tokenId)",
]);

export const COLLECTION_MINTER_ABI = parseAbi([
  "function globalMinters(address minter) view returns (bool)",
  "function setMinters(address[] minters, bool[] values)",
]);

export const AGGREGATOR_ABI = parseAbi([
  "function decimals() view returns (uint8)",
  "function latestRoundData() view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)",
]);
