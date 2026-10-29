// from: marketplace@f3956826:webapp/src/components/Bid (the bid row of the account's bids);
//       catalyrst-v2 ui3/src/marketplace/pages/MkMyBids.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// One bid: the MANA offered, who and until when. The owner reviews it on the token's bids page (where it
// can be accepted) or archives it locally; the bidder cancels it. Bids are MANA only.

import { useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";

import Button from "../../../atoms/Button";
import { sameAddress } from "../../model/address";
import { shopPath } from "../../routes";
import type { Bid } from "../lib/api";
import { bidPrice, cancelBid, isBidExpired } from "../purchase/bids";
import { archiveBid, unarchiveBid } from "../store/archivedBids";
import { formatDate, ManaAmount, manaNetworkOf, shortAddr, useShopProvider, useWrite, WriteStatus } from "./kit";

export default function BidRow({ bid, viewer, archived = false }: { bid: Bid; viewer: string; archived?: boolean }) {
  const qc = useQueryClient();
  const provider = useShopProvider();
  const write = useWrite();
  const mine = sameAddress(bid.bidder, viewer);
  const price = bidPrice(bid);
  const target = bid.tokenId
    ? shopPath("tokenDetail", { contractAddress: bid.contractAddress, tokenId: bid.tokenId })
    : shopPath("itemDetail", { contractAddress: bid.contractAddress, itemId: bid.itemId ?? "0" });
  const expired = isBidExpired(bid);

  return (
    <li className="mk-row" data-testid="shop-bid-row">
      <div className="mk-row__main">
        <Link to={target} className="mk-link">
          {bid.tokenId ? `Token #${bid.tokenId}` : `Item ${bid.itemId}`}
        </Link>
        <span className="mk-row__sub">
          {mine ? `To ${shortAddr(bid.seller)}` : `From ${shortAddr(bid.bidder)}`} · {expired ? "expired" : `until ${formatDate(bid.expiresAt)}`}
        </span>
      </div>
      <div className="mk-row__price">{price === null ? "—" : <ManaAmount wei={price} network={manaNetworkOf(bid.chainId)} />}</div>
      <div className="mk-row__actions">
        {mine ? (
          <Button
            size="sm"
            variant="secondary"
            disabled={!provider || write.busy || write.state.kind === "done"}
            onClick={() =>
              void write.run("Cancelling your bid…", "Bid cancelled.", async () => {
                const hash = await cancelBid({ provider: provider!, bid });
                await qc.invalidateQueries({ queryKey: ["bids-by-address"] });
                return hash;
              })
            }
          >
            Cancel bid
          </Button>
        ) : (
          <>
            {bid.tokenId && !expired ? (
              <Link className="mk-link" to={shopPath("tokenBids", { contractAddress: bid.contractAddress, tokenId: bid.tokenId })}>
                Review
              </Link>
            ) : null}
            <Button size="sm" variant="ghost" onClick={() => (archived ? unarchiveBid(viewer, bid.id) : archiveBid(viewer, bid.id))}>
              {archived ? "Unarchive" : "Archive"}
            </Button>
          </>
        )}
      </div>
      <WriteStatus state={write.state} />
    </li>
  );
}
