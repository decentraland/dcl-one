// from: marketplace@3bf3662f:webapp/src/components/BidPage (BidPage.tsx, BidModal/BidModal.{tsx,css}),
//       webapp/src/components/ConfirmInputValueModal, webapp/src/modules/bid/sagas.ts (PLACE_BID),
//       webapp/src/components/Routes/Routes.tsx (ProtectedRoute: signed out goes to sign-in);
//       catalyrst-v2 ui3/src/marketplace/pages/{MkBidPage,MkBidPage2}.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useId, useMemo, useState, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import type { EvmAddress } from "../../model/address";
import { CHAIN, chainLabel, isPolygon, nativeGasRequired, type ChainId } from "../../model/chain";
import { shopClient } from "../../api/shop";
import type { CatalogItem } from "@catalyst-api/market/CatalogItem";
import ManaMark from "../../../atoms/ManaMark";
import { formatMana, type ManaWei } from "../../model/money";
import type { WalletGuidance } from "../../model/settlement";
import { shopPath } from "../../routes";
import ManaGuidance from "../../wallet/ManaGuidance";
import { useShopWallet } from "../../wallet/context";
import { fetchBidsByAsset } from "../lib/api";
import { parseManaInput, readManaBalance } from "../lib/mana";
import { DEFAULT_BID_DAYS, placeBid, type BidPhase, type BidTarget } from "../purchase/bids";
import { EnsureQueryClient, formatDate, LoadFailure, Loading, NetworkNotice, useShopProvider, useWrite, WalletGate, WriteStatus } from "../components/kit";
import { TokenFrame, useIsOwner, useToken, useTokenParams } from "../components/TokenAction";
import { ConfirmValueModal, dateInputMs, dateInputValue, manaNetwork, RequireSignIn, TaButton, TaField, TokenActionRoot } from "./tokenActionUi";

const DAY = 86_400_000;

const PHASE_TEXT: Record<BidPhase, string> = {
  approving: "Allowing the marketplace to take the MANA if your bid is accepted…",
  signing: "Sign your bid in your wallet…",
  publishing: "Publishing your bid…",
};

export type BidSubject = { target: BidTarget; name: string; image: string; owner: string | null; href: string; rarity?: string | null };

export function BidGuidance({ subject, amount }: { subject: BidSubject; amount: ManaWei | null }) {
  const wallet = useShopWallet();
  const location = useLocation();
  const chainId = subject.target.chainId;
  const guidance: WalletGuidance = {
    chainId,
    nativeGasRequired: nativeGasRequired(chainId),
    account: wallet.session.kind === "web2" ? (wallet.session.account as EvmAddress) : null,
  };
  return (
    <ManaGuidance
      guidance={guidance}
      required={amount}
      requiredNote={`The MANA you offer, held on ${chainLabel(chainId)}.`}
      reasons={["secondarySale"]}
      item={{ name: subject.name, href: subject.href }}
      returnTo={location.pathname + location.search}
      onSignIn={wallet.host?.onSignIn}
      onConnectWallet={wallet.host?.onConnectWallet}
      onSwitchAccount={wallet.host?.onSwitchAccount}
    />
  );
}

export function BidForm({ subject, now = Date.now }: { subject: BidSubject; now?: () => number }) {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const { account } = useShopWallet();
  const provider = useShopProvider();
  const id = useId();
  const [price, setPrice] = useState("");
  const [expiresAt, setExpiresAt] = useState(() => dateInputValue(now() + DEFAULT_BID_DAYS * DAY));
  const [confirming, setConfirming] = useState(false);
  const write = useWrite();
  const amount = useMemo(() => parseManaInput(price), [price]);
  const chainId = subject.target.chainId;
  const network = manaNetwork(chainId);
  const expiresAtMs = dateInputMs(expiresAt);
  const invalidDate = expiresAtMs < now();
  const balance = useQuery({
    queryKey: ["mana-balance", chainId, account],
    enabled: !!account,
    queryFn: () => readManaBalance(chainId as ChainId, account!),
    staleTime: 30_000,
  });
  const short = amount !== null && balance.data !== undefined && balance.data < amount;
  const invalidPrice = price !== "" && !amount;

  if (write.state.kind === "done") {
    return (
      <div className="shop-ta-done" data-testid="shop-bid-done">
        <p role="status">
          Your bid of {amount ? formatMana(amount, { group: true }) : ""} MANA is placed. The owner can accept it until {formatDate(expiresAtMs)}.
        </p>
        <Link to={subject.href}>Back to the item</Link>
      </div>
    );
  }

  const submit = () => {
    if (!amount) return;
    void write.run("Preparing your bid…", "Bid placed.", async (progress) => {
      const trade = await placeBid({
        provider: provider!,
        target: subject.target,
        priceWei: amount,
        expiresAtMs,
        onPhase: (phase) => progress(PHASE_TEXT[phase]),
      });
      await qc.invalidateQueries({ queryKey: ["bids-by-asset"] });
      await qc.invalidateQueries({ queryKey: ["bids-by-address"] });
      return trade.id;
    });
  };

  return (
    <form
      className="shop-ta-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (!amount || short || invalidDate || write.busy) return;
        write.reset();
        setConfirming(true);
      }}
    >
      {!isPolygon(chainId) ? <NetworkNotice chainId={chainId} /> : null}
      <div className="shop-ta-fields shop-ta-fields--bid">
        <TaField label="Price" htmlFor={`${id}-price`} mana={network} invalid={invalidPrice || short} message={short ? "You don't have enough MANA" : invalidPrice ? "Enter a positive MANA amount." : undefined}>
          <input
            id={`${id}-price`}
            inputMode="decimal"
            value={price}
            onChange={(e) => setPrice(e.target.value.replace(/[^\d.]/g, ""))}
            placeholder="1000"
            disabled={write.busy}
            aria-invalid={invalidPrice || short || undefined}
          />
        </TaField>
        <TaField label="Expiration date" htmlFor={`${id}-expires`} invalid={invalidDate} message={invalidDate ? "This date has already passed" : undefined}>
          <input
            id={`${id}-expires`}
            type="date"
            value={expiresAt}
            min={dateInputValue(now())}
            onChange={(e) => setExpiresAt(e.target.value || dateInputValue(now() + DEFAULT_BID_DAYS * DAY))}
            disabled={write.busy}
            aria-invalid={invalidDate || undefined}
          />
        </TaField>
      </div>
      {isPolygon(chainId) ? (
        <span className="shop-ta-fee">
          Pay with Polygon MANA to have gas fees <span className="shop-ta-fee__covered">covered for you by the DAO</span> (item must be at least 1 MANA).
        </span>
      ) : null}
      <div className="shop-ta-buttons">
        <TaButton disabled={write.busy} onClick={() => navigate(subject.href)}>
          Cancel
        </TaButton>
        <TaButton type="submit" primary disabled={!amount || short || invalidDate || write.busy}>
          {write.busy ? "Placing bid…" : "Bid"}
        </TaButton>
      </div>
      {!confirming ? <WriteStatus state={write.state} /> : null}
      {confirming && amount ? (
        <ConfirmValueModal
          testId="shop-bid-confirm"
          value={price}
          mana={network}
          busy={write.busy}
          content={
            <>
              <p>
                You are about to bid on <b>{subject.name}</b> for <ManaMark size={14} network={network} /> {formatMana(amount, { group: true })}.
              </p>
              <p>Please re-enter the price to confirm:</p>
            </>
          }
          footer={<WriteStatus state={write.state} />}
          onCancel={() => setConfirming(false)}
          onConfirm={submit}
        />
      ) : null}
    </form>
  );
}

export function TokenBidView({ subject, isOwner }: { subject: BidSubject; isOwner: boolean }) {
  const { session } = useShopWallet();
  const provider = useShopProvider();
  const existing = useQuery({
    queryKey: ["bids-by-asset", subject.target.contractAddress, "tokenId" in subject.target ? subject.target.tokenId : subject.target.itemId],
    queryFn: () => fetchBidsByAsset(subject.target),
    enabled: session.kind === "web3",
  });
  const mine = existing.data?.results.filter((b) => session.kind === "web3" && b.bidder.toLowerCase() === session.wallet) ?? [];
  return (
    <TokenActionRoot rarity={subject.rarity}>
      <TokenFrame token={{ name: subject.name, image: subject.image, contractAddress: subject.target.contractAddress, tokenId: "tokenId" in subject.target ? subject.target.tokenId : "0" }} title="Place a bid">
        <p className="shop-ta-subtitle shop-ta-subtitle--bid">
          Set a price and expiration date for your bid on <b>{subject.name}</b>.
        </p>
        {isOwner ? (
          <p className="shop-ta-subtitle shop-ta-subtitle--error" role="alert" data-testid="shop-own-item">
            You own <b>{subject.name}</b>, so you cannot bid on it. Bids others place show up in My items.
          </p>
        ) : session.kind !== "web3" || !provider ? (
          <BidGuidance subject={subject} amount={null} />
        ) : (
          <>
            {existing.isError ? <LoadFailure error={existing.error} what="Your existing bids" onRetry={() => void existing.refetch()} /> : null}
            {mine.length > 0 ? (
              <p className="shop-ta-message" role="status">
                You already have {mine.length === 1 ? "a bid" : `${mine.length} bids`} on this item. A new bid does not replace it; cancel the old one from My items if you like.
              </p>
            ) : null}
            <WalletGate what="Placing a bid">
              <BidForm subject={subject} />
            </WalletGate>
          </>
        )}
      </TokenFrame>
    </TokenActionRoot>
  );
}

function Frame({ children }: { children: ReactNode }) {
  return (
    <TokenActionRoot>
      <TokenFrame token={null} title="Place a bid">
        {children}
      </TokenFrame>
    </TokenActionRoot>
  );
}

function TokenBidRoute({ contractAddress, tokenId }: { contractAddress: string; tokenId: string }) {
  const query = useToken(contractAddress, tokenId);
  const owner = useIsOwner(query.data ?? null);
  if (query.isPending)
    return (
      <Frame>
        <Loading label="Loading the item…" />
      </Frame>
    );
  if (query.isError || !query.data)
    return <Frame>{query.isError ? <LoadFailure error={query.error} what="This item" onRetry={() => void query.refetch()} /> : <p role="alert">This item does not exist or is not indexed yet.</p>}</Frame>;
  const token = query.data;
  const subject: BidSubject = {
    target: { contractAddress: token.contractAddress, chainId: token.chainId as ChainId, tokenId: token.tokenId },
    name: token.name,
    image: token.image,
    owner: token.owner,
    rarity: token.rarity,
    href: shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId }),
  };
  return <TokenBidView subject={subject} isOwner={owner} />;
}

function ItemBidRoute({ contractAddress, itemId }: { contractAddress: string; itemId: string }) {
  const query = useQuery({
    queryKey: ["catalog-item", contractAddress, itemId],
    queryFn: async () => (await shopClient().catalog.items<{ data?: CatalogItem[] }>({ contractAddress, itemId, first: 1 })).data?.[0] ?? null,
  });
  if (query.isPending)
    return (
      <Frame>
        <Loading label="Loading the item…" />
      </Frame>
    );
  const item = query.data;
  const subject: BidSubject = {
    target: { contractAddress, chainId: (item?.chainId ?? CHAIN.POLYGON) as ChainId, itemId },
    name: item?.name ?? `Item ${itemId}`,
    image: item?.thumbnail ?? "",
    owner: null,
    rarity: item?.rarity ?? null,
    href: shopPath("itemDetail", { contractAddress, itemId }),
  };
  if (!isPolygon(subject.target.chainId))
    return (
      <Frame>
        <p role="status" data-testid="shop-item-bid-unsupported">
          Bids on any copy of an item are only taken for Polygon collections. Bid on a specific token instead.
        </p>
      </Frame>
    );
  return (
    <>
      {query.isError ? <LoadFailure error={query.error} what="This item's details" onRetry={() => void query.refetch()} /> : null}
      <TokenBidView subject={subject} isOwner={false} />
    </>
  );
}

export default function TokenBid() {
  const ids = useTokenParams();
  return (
    <EnsureQueryClient>
      <RequireSignIn>
        {!ids ? (
          <Frame>
            <p role="alert">This link does not name an item.</p>
          </Frame>
        ) : ids.tokenId ? (
          <TokenBidRoute contractAddress={ids.contractAddress} tokenId={ids.tokenId} />
        ) : (
          <ItemBidRoute contractAddress={ids.contractAddress} itemId={ids.itemId!} />
        )}
      </RequireSignIn>
    </EnsureQueryClient>
  );
}
