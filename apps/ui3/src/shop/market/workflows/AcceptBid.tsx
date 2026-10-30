// from: marketplace@3bf3662f:webapp/src/components/AssetPage/BidsTable (BidsTable.tsx, BidsTable.module.css, utils.tsx,
//       BidsTableContent/BidsTableContent.{tsx,module.css}), webapp/src/components/Table/TableContainer/TableContainer.{tsx,css},
//       webapp/src/components/Table/TableContent/TableContent.css, webapp/src/components/AssetPage/OnBack/OnBack.{tsx,css},
//       webapp/src/images/{empty-bids,onBack}.png, webapp/src/components/ConfirmInputValueModal,
//       webapp/src/modules/bid/sagas.ts (ACCEPT_BID, CANCEL_BID); catalyrst-v2 ui3/src/marketplace/workflows/MkAcceptBidWizardView.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { formatEther } from "viem";
import { useEffect, useId, useRef, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";

import ManaMark from "../../../atoms/ManaMark";
import { sameAddress } from "../../model/address";
import { formatMana } from "../../model/money";
import { isPolygon } from "../../model/chain";
import { shopPath } from "../../routes";
import { patchManageCaches } from "../../seller/lib/manage-cache";
import { useShopWallet } from "../../wallet/context";
import { fetchBidsByAsset, type Bid } from "../lib/api";
import { acceptBid, bidPrice, cancelBid, isBidExpired } from "../purchase/bids";
import { EnsureQueryClient, LoadFailure, Loading, ManaAmount, manaNetworkOf, NetworkNotice, shortAddr, useShopProvider, useWrite, WalletGate, WriteStatus } from "../components/kit";
import { useIsOwner, WithRouteToken, type TokenRecord } from "../components/TokenAction";
import { ConfirmValueModal, distanceToNow, TaButton, TokenActionRoot } from "../pages/tokenActionUi";
import emptyBids from "./empty-bids.png";
import onBackIcon from "./onBack.png";
import "./acceptbid.css";

function AcceptConfirm({ token, bid, onDone, onBack }: { token: TokenRecord; bid: Bid; onDone: () => void; onBack: () => void }) {
  const qc = useQueryClient();
  const { account } = useShopWallet();
  const provider = useShopProvider();
  const write = useWrite();
  const price = bidPrice(bid);
  const network = manaNetworkOf(bid.chainId);
  return (
    <WalletGate what="Accepting a bid">
      <ConfirmValueModal
        testId="shop-accept-confirm"
        value={price !== null ? formatEther(price) : null}
        mana={network}
        busy={write.busy}
        content={
          <>
            <p>
              You are about to accept a bid on <b>{token.name}</b> for {price !== null ? <ManaAmount wei={price} network={network} /> : "the bid's MANA"}.
            </p>
            <p className="shop-ta-confirm__hint">
              {token.name} goes to {shortAddr(bid.bidder)} and you receive the MANA. This cannot be undone.
            </p>
            {token.isOnSale ? <p className="shop-ta-confirm__hint">Your open listing for this item stops working once it is sold.</p> : null}
            {token.openRentalId ? <p className="shop-ta-confirm__hint">This LAND has an open rental listing; accepting ends it.</p> : null}
            {price !== null ? <p>Please re-enter the price to confirm:</p> : null}
          </>
        }
        footer={
          <>
            {!isPolygon(bid.chainId) ? <NetworkNotice chainId={bid.chainId} /> : null}
            <WriteStatus state={write.state} />
          </>
        }
        onCancel={onBack}
        onConfirm={() =>
          void write.run("Confirm in your wallet…", "Bid accepted. The MANA is on its way to your wallet.", async (progress) => {
            const hash = await acceptBid({ provider: provider!, bid, onSubmitted: () => progress("Waiting for the network to confirm…") });
            patchManageCaches(qc, { address: account ?? undefined, contractAddress: token.contractAddress, tokenId: token.tokenId }, { kind: "gone" });
            await qc.invalidateQueries({ queryKey: ["bids-by-asset"] });
            await qc.invalidateQueries({ queryKey: ["token", token.contractAddress, token.tokenId] });
            onDone();
            return hash;
          })
        }
      />
    </WalletGate>
  );
}

type SortBy = "most_expensive" | "recently_offered" | "recently_updated";

const SORT_OPTIONS: { value: SortBy; text: string }[] = [
  { value: "most_expensive", text: "Highest" },
  { value: "recently_offered", text: "Recently offered" },
  { value: "recently_updated", text: "Recently updated" },
];

const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

function dateAndMonth(ms: number): string {
  const d = new Date(ms);
  return `${MONTHS[d.getMonth()]} ${d.getDate()}`;
}

function BidRow({ bid, canAccept, isMine, onAccept }: { bid: Bid; canAccept: boolean; isMine: boolean; onAccept: () => void }) {
  const qc = useQueryClient();
  const provider = useShopProvider();
  const write = useWrite();
  const price = bidPrice(bid);
  const expired = isBidExpired(bid);
  const network = manaNetworkOf(bid.chainId);
  return (
    <tr className="shop-bids__row" data-testid="shop-token-bid">
      <td data-label="From">
        <Link className="shop-bids__from" to={shopPath("account", { address: bid.bidder })} title={bid.bidder}>
          {isMine ? "You" : shortAddr(bid.bidder)}
        </Link>
      </td>
      <td className="shop-bids__wide" data-label="Published date">
        {dateAndMonth(bid.createdAt)}
      </td>
      <td className="shop-bids__wide" data-label="Expiration date">
        {expired ? "Expired" : distanceToNow(bid.expiresAt, { addSuffix: true })}
      </td>
      <td data-label="Offer">
        <div className="shop-bids__offer">
          <span className="shop-bids__price">
            {price === null ? (
              "—"
            ) : (
              <>
                <ManaMark size={14} network={network} /> {formatMana(price, { group: true })}
              </>
            )}
          </span>
          {(canAccept && !expired) || isMine ? (
            <span className="shop-bids__actions">
              {isMine ? (
                <TaButton
                  className="shop-ta-btn--small"
                  disabled={!provider || write.busy || write.state.kind === "done"}
                  onClick={() =>
                    void write.run("Cancelling your bid…", "Bid cancelled.", async () => {
                      const hash = await cancelBid({ provider: provider!, bid });
                      await qc.invalidateQueries({ queryKey: ["bids-by-asset"] });
                      return hash;
                    })
                  }
                >
                  Cancel
                </TaButton>
              ) : null}
              {canAccept && !expired ? (
                <TaButton primary className="shop-ta-btn--small" onClick={onAccept}>
                  Accept
                </TaButton>
              ) : null}
            </span>
          ) : null}
        </div>
        <WriteStatus state={write.state} />
      </td>
    </tr>
  );
}

function OnBack({ onClick }: { onClick: () => void }) {
  return (
    <div className="shop-bids__top">
      <button type="button" className="shop-bids__back" onClick={onClick}>
        <img src={onBackIcon} alt="" width={20} height={20} />
        Back
      </button>
    </div>
  );
}

export function AcceptBidView({ token }: { token: TokenRecord }) {
  const owner = useIsOwner(token);
  const { account } = useShopWallet();
  const navigate = useNavigate();
  const [reviewing, setReviewing] = useState<Bid | null>(null);
  const [accepted, setAccepted] = useState(false);
  const [sortBy, setSortBy] = useState<SortBy>("most_expensive");
  const bids = useQuery({
    queryKey: ["bids-by-asset", token.contractAddress, token.tokenId, sortBy],
    queryFn: () => fetchBidsByAsset({ contractAddress: token.contractAddress, tokenId: token.tokenId }, { sortBy }),
  });
  const [params] = useSearchParams();
  const linkedId = params.get("bid");
  const opened = useRef(false);
  useEffect(() => {
    if (opened.current || !owner || !linkedId || !bids.data) return;
    opened.current = true;
    const bid = bids.data.results.find((b) => b.id === linkedId);
    if (bid && !isBidExpired(bid)) setReviewing(bid);
  }, [owner, linkedId, bids.data]);
  const tokenHref = shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId });
  const sortId = useId();

  return (
    <TokenActionRoot rarity={token.rarity}>
      <div className="shop-bids">
        <OnBack onClick={() => navigate(tokenHref)} />
        <div className="shop-bids__media">
          {token.image ? <img src={token.image} alt={token.name} /> : <span className="shop-bids__glyph">{token.name.slice(0, 1)}</span>}
        </div>
        <h1 className="shop-bids__title">
          <Link to={tokenHref}>
            {token.name} #{token.tokenId.length > 12 ? `${token.tokenId.slice(0, 6)}…` : token.tokenId}
          </Link>
        </h1>
        <section className="shop-bids__table" aria-labelledby={`${sortId}-title`}>
          <div className="shop-bids__tabs">
            <h2 id={`${sortId}-title`} className="shop-bids__tab">
              Offers
            </h2>
            <label className="shop-bids__sort">
              <span className="u-sr-only">Sort offers by</span>
              <select id={sortId} value={sortBy} onChange={(e) => setSortBy(e.target.value as SortBy)}>
                {SORT_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.text}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {accepted ? (
            <div className="shop-ta-done shop-bids__done" data-testid="shop-accept-done">
              <p role="status">
                Bid accepted. <b>{token.name}</b> is going to its new owner.
              </p>
              <Link to={shopPath("myItems")}>Back to My items</Link>
            </div>
          ) : bids.isPending ? (
            <div className="shop-bids__status">
              <Loading label="Loading bids…" />
            </div>
          ) : bids.isError ? (
            <div className="shop-bids__status">
              <LoadFailure error={bids.error} what="The bids on this item" onRetry={() => void bids.refetch()} />
            </div>
          ) : bids.data.results.length === 0 ? (
            <div className="shop-bids__empty" data-testid="shop-empty">
              <img src={emptyBids} alt="" className="shop-bids__empty-icon" />
              <span className="shop-bids__empty-text">
                The are no offers yet
                {!owner ? (
                  <Link className="shop-bids__make-offer" to={shopPath("tokenBid", { contractAddress: token.contractAddress, tokenId: token.tokenId })}>
                    Make an offer
                  </Link>
                ) : null}
              </span>
            </div>
          ) : (
            <table className="shop-bids__grid">
              <thead>
                <tr>
                  <th>From</th>
                  <th className="shop-bids__wide">Published date</th>
                  <th className="shop-bids__wide">Expiration date</th>
                  <th>Offer</th>
                </tr>
              </thead>
              <tbody>
                {bids.data.results.map((bid) => (
                  <BidRow key={bid.id} bid={bid} canAccept={owner} isMine={sameAddress(bid.bidder, account)} onAccept={() => setReviewing(bid)} />
                ))}
              </tbody>
            </table>
          )}
        </section>
        {reviewing && !accepted ? <AcceptConfirm token={token} bid={reviewing} onBack={() => setReviewing(null)} onDone={() => setAccepted(true)} /> : null}
      </div>
    </TokenActionRoot>
  );
}

export default function AcceptBid() {
  return (
    <EnsureQueryClient>
      <TokenActionRoot>
        <WithRouteToken title="Bids">{(token) => <AcceptBidView token={token} />}</WithRouteToken>
      </TokenActionRoot>
    </EnsureQueryClient>
  );
}
