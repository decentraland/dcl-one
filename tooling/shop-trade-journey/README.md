# Shop trade journey

Drives the Shop's trade loop, and its primary loop, end to end on Decentraland's testnet. Two real wallets use the real UI of a
local stack running the `testnet` profile, and every signature and transaction lands on Amoy. The Shop's
unit tests cover the same flows over fixtures; this is the run that shows they work against the chain,
the relayer and the order book.

```sh
pnpm install                                   # playwright-core and viem, in this folder only
set -a; eval "$(foundation-dev env)"; set +a   # exports the running stack's URL and profile
node journey.mts --preflight                   # stack, sign-in and balances; nothing on chain
E2E_ENV_FILE=<file outside the repo> node journey.mts
E2E_ENV_FILE=<file outside the repo> node journey.mts --primary   # discount, MANA and credits purchases
pnpm test && pnpm typecheck
```

## The stack

Run it in its own worktree, started in the testnet profile:

```sh
FOUNDATION_DEV_PROFILE=testnet foundation-dev up
```

The profile mirrors the .zone marketplace API's order book and reads Amoy and Sepolia's subgraphs and
RPCs. Its relay admits Amoy's Decentraland contracts and forwards gasless transactions to the .zone
transactions API, and it builds the Shop to settle on Amoy. The journey does not override the Shop's
chain, so a stack that is not wired for Amoy fails at its first signature. A stack that already holds data of another profile refuses to switch to it, so the two networks
never share a database, and a testnet stack refuses `foundation-dev trades-snapshot`. The journey refuses
to run unless `FOUNDATION_DEV_STACK_PROFILE` (from `foundation-dev env`) is `testnet`.

## Settings

Set these in the environment, or as `KEY=value` lines in the file `E2E_ENV_FILE` names. Keep that file
outside the repository. The journey prints addresses, never keys.

| Variable | |
| --- | --- |
| `E2E_SELLER_KEY`, `E2E_BUYER_KEY` | Private keys of two testnet wallets. Never use a wallet that holds anything on mainnet. |
| `E2E_TOKEN` | `<contract>:<tokenId>`, an Amoy NFT the seller owns. |
| `E2E_PRICE`, `E2E_REPRICE`, `E2E_BID` | MANA amounts. The defaults are 1, 2 and 1. |
| `E2E_SITES_URL` | The stack's Sites URL. The default is `FOUNDATION_DEV_SITES_URL`, and any other value is refused, since `FOUNDATION_DEV_STACK_PROFILE` describes only that stack. Both come from the shell, never from `E2E_ENV_FILE`. |
| `E2E_ITEM` | `--primary` only: `<collection>:<itemId>`, an Amoy item of a collection the seller created, listed in the Shop for credits and MANA. |
| `E2E_DISCOUNT` | `--primary` only: the discount, a whole percentage from 5 to 70. The default is 10. |
| `E2E_TOPUP` | `--primary` only: the test credits the buyer tops up with the mock card. The default is 100. |
| `E2E_CREDITS_SIGNER` | `--primary` only, optional: the address of the stack's credits signer. The preflight checks it holds `CREDITS_SIGNER_ROLE` on Amoy's CreditsManager. |
| `E2E_RPC_AMOY` | The Amoy RPC for balance and ownership reads. The default is `rpc.decentraland.org/amoy`. |

## The loop

A complete run ends where it started, so it can run again.

1. The seller lists the token, changes its price, and cancels the listing.
2. The buyer bids, cancels the bid, and bids again.
3. The seller accepts the bid, and the buyer now owns the token.
4. The buyer lists it, and the seller buys it back with MANA through the cart.

What each step costs on Amoy:

- Listing and bidding are signatures.
- Approvals and cancels are relayed by the .zone transactions API, so they cost no gas. Both cancel steps
  fail if the Shop falls back to a transaction the wallet pays for.
- Accepting a bid is a transaction the seller pays for in POL.
- A purchase pays the price in MANA, plus gas if the relay refuses it.

So both wallets need a little POL, the buyer at least `E2E_BID` MANA and the seller at least `E2E_PRICE`
MANA. The preflight prints both balances and names what is missing.

A run that fails part way leaves things where that step left them. The screenshot and the log name the
step. Before running again:

- After the seller's listing: cancel it in the Shop (`/shop/token/<contract>/<id>/cancel`), or leave it, since
  listing again replaces it.
- After a bid: the buyer cancels it from the token's Bids tab. The preflight stops while the buyer has an
  open bid on the token.
- After the accept: the buyer owns the token. Swap `E2E_SELLER_KEY` and `E2E_BUYER_KEY` for the next run,
  or let the buyer list it and buy it back by hand.

## The primary loop

`--primary` runs on `E2E_ITEM` instead of `E2E_TOKEN`:

1. The seller starts a discount on the item's collection from My Store. That is a signature; the stack
   stores the coupon.
2. The item page shows the regular price crossed out.
3. The buyer buys one unit with MANA through the cart. The step reports the MANA it paid.
4. The buyer tops up `E2E_TOPUP` test credits through the stack's mock card route, signed as the buyer.
   Nothing is charged.
5. The buyer buys one unit with credits through the cart.
6. The seller ends the discount, a transaction the seller pays for in POL.

Each run mints two units, so it runs while the item has supply. The seller needs a little POL, and the
buyer POL and enough MANA for one unit at the discounted price.

### Credits

The stack signs credits with its own key against Amoy's CreditsManager
(`0x8052a560e6e6ac86eeb7e711a4497f639b322fb3`), and the contract accepts only a signer that holds
`CREDITS_SIGNER_ROLE`. The testnet profile turns on credits when the gitignored stack state holds that key:

```sh
install -m 600 <key file outside the repo> .cache/dev/credits-signer.key
foundation-dev down && foundation-dev up
```

The profile also runs the CreditsManager usage index on Amoy, and the stack signs only once it has caught
up. `GET /credits/usage-index/status` reports both, and the primary preflight reads it. Without the key,
the preflight says so and the full loop stops before the discount.

## The wallet

`lib/wallet.mts` is the EIP-1193 provider the page sees, as an extension and as `window.ethereum`. The key
stays in Node: the page forwards each request over a Playwright binding, and signing and sending happen
there with viem. Only the Shop's own top-level page can reach it. It refuses:

- any chain but Amoy and Sepolia;
- typed data whose domain names another chain, in `chainId` or in `salt` (where Decentraland's trade and
  meta-transaction domains carry it), or names no chain at all;
- a transaction for a chain other than the current one, or to a contract outside Amoy's trade contracts
  (MANA, the marketplaces, the bid contract, the collection store) and the token's collection; the
  primary loop adds the CouponManager, the CreditsManager and the item's collection;
- a typed-data `chainId` that is not a number, since viem would leave it out of what is signed;
- requests for another address, and `eth_sign`, `eth_signTransaction` and `eth_sendRawTransaction`.

`personal_sign` is not chain-bound, which is one more reason to use a wallet that holds nothing on
mainnet.

A failed step leaves a screenshot under `state/shop-trade-journey/`. The run ends with the Polygonscan
link of every transaction it sent.
