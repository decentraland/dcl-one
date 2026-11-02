# Shop trade journey

Drives the Shop's trade loop end to end on Decentraland's testnet. Two real wallets use the real UI of a
local stack running the `testnet` profile, and every signature and transaction lands on Amoy. The Shop's
unit tests cover the same flows over fixtures; this is the run that shows they work against the chain,
the relayer and the order book.

```sh
pnpm install                                   # playwright-core and viem
eval "$(foundation-dev env)"                   # FOUNDATION_DEV_SITES_URL of the running stack
node journey.mts --preflight                   # stack, sign-in and balances; nothing on chain
E2E_ENV_FILE=<file outside the repo> node journey.mts
pnpm test && pnpm typecheck
```

## The stack

Run it in its own worktree, started in the testnet profile:

```sh
FOUNDATION_DEV_PROFILE=testnet foundation-dev up
```

The profile mirrors the .zone marketplace API's order book and reads Amoy and Sepolia's subgraphs and
RPCs. It relays gasless transactions through the .zone transactions API, and builds the Shop to settle
on Amoy. A stack that already holds mainnet data refuses to switch to it, so the two networks never share
a database.

## Settings

Set these in the environment, or as `KEY=value` lines in the file `E2E_ENV_FILE` names. Keep that file
outside the repository. The journey prints addresses, never keys.

| Variable | |
| --- | --- |
| `E2E_SELLER_KEY`, `E2E_BUYER_KEY` | Private keys of two testnet wallets. Never use a wallet that holds anything on mainnet. |
| `E2E_TOKEN` | `<contract>:<tokenId>`, an Amoy NFT the seller owns. |
| `E2E_PRICE`, `E2E_REPRICE`, `E2E_BID` | MANA amounts. The defaults are 1, 2 and 1. |
| `E2E_SITES_URL` | The stack's Sites URL. The default is `FOUNDATION_DEV_SITES_URL`. |
| `E2E_RPC_AMOY` | The Amoy RPC for balance and ownership reads. The default is `rpc.decentraland.org/amoy`. |

## The loop

It ends where it started, so it can run again.

1. The seller lists the token, changes its price, and cancels the listing.
2. The buyer bids, cancels the bid, and bids again.
3. The seller accepts the bid, and the buyer now owns the token.
4. The buyer lists it, and the seller buys it back with MANA through the cart.

What each step costs on Amoy:

- Listing and bidding are signatures.
- Approvals and cancels are relayed by the .zone transactions API, so they cost no gas.
- Accepting a bid is a transaction the seller pays for in POL.
- A purchase pays the price in MANA, plus gas if the relay refuses it.

So both wallets need a little POL and some testnet MANA. The preflight prints both balances and names
what is missing.

## The wallet

`lib/wallet.mts` is the EIP-1193 provider the page sees, as an extension and as `window.ethereum`. The key
stays in Node: the page forwards each request over a Playwright binding, and signing and sending happen
there with viem. It refuses any chain but Amoy and Sepolia, typed data for another chain's domain, and
requests for another address. A key handed to it therefore cannot move mainnet funds.

A failed step leaves a screenshot under `state/shop-trade-journey/`. The run ends with the Polygonscan
link of every transaction it sent.
