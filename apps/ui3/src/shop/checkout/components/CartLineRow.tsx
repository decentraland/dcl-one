// from: shop@2d3bf62:app/src/pages/Cart.tsx (the cart line card: thumbnail with the ready check, name,
//       "By {creator}", the Creator tag, quantity stepper, live price with the old one struck through,
//       availability, favourite + remove) and Cart.styles.ts (Card, Thumb, Info, Desc, Foot, Stepper, Price)
import { useEffect, useState, type ReactNode } from "react";

import { Trash } from "../../../atoms/icons";
import Icon from "../../discovery/components/Icon";
import { CreatorBadge } from "../../discovery/components/Hero";
import { fetchCatalogByIds } from "../../market/lib/catalog";
import { shopFavorites, useFavorites } from "../../market/store/favorites";
import { useShopWallet } from "../../wallet/context";
import { CreditsAmount, ManaAmount } from "../../credits/components/Amount";
import type { LineChange } from "../../cart/revalidate";
import type { LineRefusal } from "../../cart/partition";
import { lineUnits } from "../../cart/partition";
import { creditsCeil, creditsFloor, formatMana, mulCents, mulMana, type ManaWei } from "../../model/money";
import type { CartLine, PurchaseOffer } from "../../model/purchase";
import type { PaymentMethod, Quote } from "../../model/settlement";
import type { PaymentPresentation } from "../../payment/present";
import { manaChainOf } from "../../payment/rule";
import { outageText } from "../../payment/copy";
import ManaGuidance, { type ManaGuidanceProps } from "../../wallet/ManaGuidance";
import PaymentChoice from "./PaymentChoice";
import "./cartline.css";

function quoteText(q: Quote): string {
  return q.method === "credits" ? `${creditsCeil(q.quoted)} Credits` : `${formatMana(q.amount, { group: true })} MANA`;
}

function changeText(change: LineChange): string {
  switch (change.kind) {
    case "unavailable":
      return "No longer available: sold or removed.";
    case "priceChanged":
      return `Price changed from ${quoteText(change.before)} to ${quoteText(change.after)}.`;
    case "quoteUnavailable":
      return outageText(change.method, change.missing);
    case "methodWithdrawn":
      return change.allowed.length ? `Can't be paid with ${change.method === "credits" ? "Credits" : "MANA"} any more. Choose another way to pay.` : "Can't be bought right now.";
    case "insufficientStock":
      return `Only ${change.available} left: lower the quantity to continue.`;
  }
}

function refusalText(refusal: LineRefusal): string {
  switch (refusal.code) {
    case "methodNotAllowed":
      return refusal.allowed.length ? "This item can't be paid the way it's set. Choose how to pay." : "This item can't be bought right now.";
    case "quoteUnavailable":
      return outageText(refusal.method ?? "credits", refusal.missing ?? null);
    case "insufficientStock":
      return "Not enough left for this quantity.";
    case "groupTooLarge":
      return "Too many items to pay with Credits in one go. Remove some and buy them separately.";
    case "offerUnavailable":
      return "Checking this item's price…";
    default:
      return "This item can't be bought right now.";
  }
}

export type CartLineRowProps = {
  line: CartLine;
  offer: PurchaseOffer | null;
  view: PaymentPresentation | null;
  changes: LineChange[];
  refusal?: LineRefusal;
  disabled?: boolean;
  onMethod(method: PaymentMethod): void;
  onQuantity(quantity: number): void;
  onRemove(): void;
  onRetry(): void;
  guidance?: Omit<ManaGuidanceProps, "guidance" | "required" | "reasons" | "item">;
};

const ReadyCheck = () => (
  <svg viewBox="0 0 20 20" width="12" height="12" aria-hidden="true">
    <path d="M5 10.5l3 3 7-7.5" fill="none" stroke="#fff" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const Minus = () => (
  <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false">
    <path d="M3.5 8h9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const Plus = () => (
  <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false">
    <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const CreatorLeaf = () => (
  <svg className="shop-cartline__tag-ico" viewBox="0 0 12 11" aria-hidden="true">
    <path d="M1 10C1 5 4.5 1.5 11 1c-.4 6.2-4 9-9 9H1zm0 0l5-5" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

function favoriteKeyOf(line: CartLine): string | null {
  return line.asset.token.kind === "item" ? `${line.asset.contract.toLowerCase()}-${line.asset.token.itemId}` : null;
}

function FavoriteToggle({ favKey, name, disabled }: { favKey: string; name: string; disabled: boolean }) {
  const { account } = useShopWallet();
  const favorites = useFavorites();
  const [busy, setBusy] = useState(false);
  const saved = !!favorites.items[favKey];
  useEffect(() => {
    shopFavorites.reloadFor(account);
  }, [account]);
  async function toggle() {
    setBusy(true);
    try {
      const owner = shopFavorites.getState().account;
      const item = shopFavorites.getState().items[favKey] ?? (await fetchCatalogByIds([favKey]))[0];
      if (item && shopFavorites.getState().account === owner) await shopFavorites.toggle(item);
    } catch {
    } finally {
      setBusy(false);
    }
  }
  return (
    <button
      type="button"
      className="shop-cartline__icon-btn shop-cartline__fav"
      data-on={saved || undefined}
      onClick={() => void toggle()}
      disabled={disabled || busy}
      aria-label={saved ? `Remove ${name} from favorites` : `Add ${name} to favorites`}
      title={saved ? "Remove from favorites" : "Add to favorites"}
    >
      <Icon name={saved ? "heart-solid" : "heart"} size={24} />
    </button>
  );
}

export default function CartLineRow({ line, offer, view, changes, refusal, disabled = false, onMethod, onQuantity, onRemove, onRetry, guidance }: CartLineRowProps) {
  const [showGuidance, setShowGuidance] = useState(false);
  const quantity = lineUnits(line);
  const isMint = line.asset.token.kind === "item";
  const max = offer?.available ?? null;
  const manaChain = offer ? manaChainOf(offer) : line.asset.chainId;
  const manaQuote = view?.options.find((o) => o.method === "mana")?.quote;
  const required = manaQuote?.method === "mana" ? mulMana(manaQuote.amount, quantity) : null;
  const unavailable = changes.some((c) => c.kind === "unavailable");
  const soldOut = changes.some((c) => c.kind === "insufficientStock" && c.available === 0);
  const namedOutage = (method: PaymentMethod | null) => !!view?.options.some((o) => o.method === (method ?? "credits") && o.state.kind === "unavailable");
  const shownChanges = changes.filter((c) => c.kind !== "unavailable" && (c.kind !== "quoteUnavailable" || !namedOutage(c.method)));
  const gone = changes.find((c) => c.kind === "unavailable");
  const quoteChangeShown = shownChanges.some((c) => c.kind === "quoteUnavailable");
  const shownRefusal =
    refusal && refusal.code !== "offerUnavailable" && !(refusal.code === "quoteUnavailable" && (namedOutage(refusal.method) || quoteChangeShown)) ? refusal : null;

  const current = view ? (line.method ?? view.primary) : null;
  const option = view?.options.find((o) => o.method === current) ?? null;
  const was = changes.find((c): c is Extract<LineChange, { kind: "priceChanged" }> => c.kind === "priceChanged" && c.before.method === current);
  let price: ReactNode = null;
  if (option?.method === "credits" && option.charge !== null) {
    const total = mulCents(option.charge, quantity);
    if (total !== null) price = <CreditsAmount credits={creditsFloor(total)} size={24} />;
  } else if (option?.method === "mana" && option.quote?.method === "mana") {
    const total = mulMana(option.quote.amount, quantity);
    if (total !== null) price = <ManaAmount wei={total} chainId={manaChain} size={24} />;
  }
  const showChoice = !!view;
  const choicePriced = !!view && view.options.length > 1;
  const favKey = favoriteKeyOf(line);
  const href = line.display.href;

  return (
    <article className="shop-cartline" data-unavailable={unavailable || undefined} aria-label={line.display.name}>
      <div className="shop-cartline__thumb" data-thumb>
        {href ? (
          <a className="shop-cartline__thumb-link" href={href} aria-label={line.display.name}>
            {line.display.thumbnail ? <img src={line.display.thumbnail} alt={line.display.name} loading="lazy" /> : null}
          </a>
        ) : line.display.thumbnail ? (
          <img src={line.display.thumbnail} alt={line.display.name} loading="lazy" />
        ) : null}
        <span className="shop-cartline__check" data-check aria-hidden="true">
          <ReadyCheck />
        </span>
      </div>
      <div className="shop-cartline__main">
        <div className="shop-cartline__desc" data-desc>
          {href ? (
            <a className="shop-cartline__name" href={href} title={line.display.name}>
              {line.display.name}
            </a>
          ) : (
            <span className="shop-cartline__name" title={line.display.name}>
              {line.display.name}
            </span>
          )}
          {line.display.creator ? (
            <span className="shop-cartline__creator">
              <CreatorBadge address={line.display.creator} prefix />
            </span>
          ) : null}
          {isMint && offer?.sale === "primaryMint" ? (
            <span className="shop-cartline__tag" data-testid="cart-creator-tag">
              <CreatorLeaf />
              Creator
            </span>
          ) : null}
        </div>
        <div className="shop-cartline__foot">
          {unavailable || soldOut ? (
            <span className="shop-cartline__unavailable">
              <Icon name="warning-fill" size={24} className="shop-cartline__warn" />
              {gone ? changeText(gone) : "Out of stock"}
            </span>
          ) : (
            <>
              {isMint ? (
                <div className="shop-cartline__qty" role="group" aria-label={`Quantity of ${line.display.name}`}>
                  <button type="button" onClick={() => onQuantity(quantity - 1)} disabled={disabled || quantity <= 1} aria-label={`Fewer ${line.display.name}`}>
                    <Minus />
                  </button>
                  <output aria-live="polite">{quantity}</output>
                  <button type="button" onClick={() => onQuantity(quantity + 1)} disabled={disabled || (max !== null && quantity >= max)} aria-label={`More ${line.display.name}`}>
                    <Plus />
                  </button>
                </div>
              ) : null}
              {price ? (
                <div className="shop-cartline__price" data-testid="cart-line-price">
                  {price}
                  {was ? <s className="shop-cartline__was">{quoteText(was.before)}</s> : null}
                </div>
              ) : !view ? (
                <p className="shop-cartline__pending" role="status">
                  Checking the price…
                </p>
              ) : null}
            </>
          )}
        </div>
        {showChoice && view && !unavailable ? (
          <PaymentChoice view={view} selected={line.method} onSelect={onMethod} quantity={quantity} manaChainId={manaChain} disabled={disabled} onRetry={onRetry} label={line.display.name} hidePrice={!!price && !choicePriced} />
        ) : null}
        {max !== null && isMint && !unavailable && max > 0 && max <= 10 ? <span className="shop-cartline__stock">{max} left</span> : null}
        {shownChanges.map((c, i) => (
          <p key={`${c.kind}-${i}`} className="shop-cartline__change" data-kind={c.kind} role="status">
            {changeText(c)}
          </p>
        ))}
        {shownRefusal ? (
          <p className="shop-cartline__change" data-kind="refused" role="status">
            {refusalText(shownRefusal)}
          </p>
        ) : null}
        {view?.guidance && guidance ? (
          <div className="shop-cartline__guidance">
            <button type="button" className="shop-cartline__toggle" aria-expanded={showGuidance} onClick={() => setShowGuidance((v) => !v)}>
              {showGuidance ? "Hide how to pay with MANA" : "How to pay with MANA"}
            </button>
            {showGuidance ? (
              <ManaGuidance
                {...guidance}
                guidance={view.guidance}
                required={required as ManaWei | null}
                reasons={view.reasons}
                item={{ name: line.display.name, href: line.display.href ?? "/shop/cart" }}
                carry={{ asset: line.asset, listing: line.listing, display: line.display, provenance: line.provenance, quantity: line.quantity }}
              />
            ) : null}
          </div>
        ) : null}
      </div>
      <div className="shop-cartline__actions">
        {favKey ? <FavoriteToggle favKey={favKey} name={line.display.name} disabled={disabled} /> : null}
        <button type="button" className="shop-cartline__icon-btn shop-cartline__remove" onClick={onRemove} disabled={disabled} aria-label={`Remove ${line.display.name}`} title="Remove">
          <Trash size={24} />
        </button>
      </div>
    </article>
  );
}
