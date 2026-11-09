// from: shop@3a322cf9:app/src/components/PaymentMethodStep/PaymentMethodStep.tsx, app/src/assets/payment/credits-coin.webp,
//       app/src/assets/mana-matic.svg, app/src/intl/en.json (buyModal)
// "Choose your payment method": Buy now's payment step for a buyer holding MANA. Two rows, Credits and MANA, and one
// Buy. The rows are CHECKBOXES, not radio buttons, because ticking both is how the buyer pays with Credits and MANA
// together, so the ticks are a set mapped onto the rails the dialog can settle:
//
//   {credits}        -> "credits"   the whole price from the Credits balance
//   {mana}           -> "mana"      the whole price in MANA, no Credit spent
//   {credits, mana}  -> "combined"  the Credits balance first, MANA for the rest
//
// A rail that cannot pay stays on screen, disabled, with the shortfall as its reason ("Not enough MANA"); a missing
// option would read as a bug next to the balances shown. Buy stays disabled until the ticks are a rail that settles.
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";

import { formatCredits } from "../../checkout/lib/currency";
import CategoryFlap from "../../discovery/components/CategoryFlap";
import Icon from "../../discovery/components/Icon";
import { useCreatorName } from "../../discovery/hooks";
import { rarityCardPalette } from "../../discovery/lib/rarity";
import { formatMana, formatManaPrice, type ManaWei } from "../../model/money";
import type { CartLineDisplay } from "../../model/purchase";
import type { MixedPayment } from "../../payment/present";
import creditsCoin from "../assets/credits-coin.webp?url";
import manaCoin from "../assets/mana-matic.svg?url";
import styles from "./PaymentMethodStep.module.css";

const TEXT = {
  title: "Choose your payment method",
  close: "Close",
  credits: "Credits",
  creditsBalance: "Credits Balance:",
  mana: "Polygon MANA",
  manaBalance: "Mana Balance:",
  notEnoughMana: "Not enough MANA",
  buy: "Buy",
  itemFallback: "Item",
} as const;

export type StepRail = "credits" | "mana" | "combined";

/** The rails this purchase can settle on now; null where one cannot pay. */
export type StepRails = {
  /** The whole price from the Credits, at its whole-credit charge. */
  credits: { credits: number } | null;
  /** The whole price in MANA. */
  mana: { manaWei: bigint } | null;
  /** The Credits balance first (whole credits), MANA for the rest. */
  combined: MixedPayment | null;
};

type Ticked = "credits" | "mana";

/** The rail a ticked set settles as, or null when that combination cannot pay. */
export function railFor(picked: ReadonlySet<Ticked>, rails: StepRails): StepRail | null {
  const c = picked.has("credits");
  const m = picked.has("mana");
  if (c && m) return rails.combined ? "combined" : null;
  if (c) return rails.credits ? "credits" : null;
  if (m) return rails.mana ? "mana" : null;
  return null;
}

/** What is ticked first: the Credits when they pay, both when only the split does, MANA last. */
function seed(credits: boolean, combined: boolean, mana: boolean): Set<Ticked> {
  if (credits) return new Set<Ticked>(["credits"]);
  if (combined) return new Set<Ticked>(["credits", "mana"]);
  if (mana) return new Set<Ticked>(["mana"]);
  return new Set<Ticked>();
}

export type PaymentMethodStepProps = {
  display: CartLineDisplay;
  /** The item's price in whole Credits. */
  priceCredits: number;
  rails: StepRails;
  /** What the item costs in MANA now (0n when unknown): the MANA row's amount when that row cannot pay. */
  priceManaWei: bigint;
  /** The buyer's Credits balance, in whole credits. */
  balanceCredits: number;
  /** The buyer's MANA balance, in wei. */
  manaBalanceWei: bigint;
  /** The rail the ticks resolve to, each time they change: the dialog makes it the purchase's. */
  onPick(rail: StepRail | null): void;
  /** Whether Buy can settle the picked rail now (priced, a wallet to sign): Buy waits otherwise. */
  ready: boolean;
  onBuy(rail: StepRail): void;
  onClose(): void;
  busy?: boolean;
  /** Something to read before confirming, above Buy: a price that changed, what Buy waits for. */
  notice?: ReactNode;
};

export default function PaymentMethodStep(props: PaymentMethodStepProps) {
  const { display, rails, busy = false } = props;
  const hasCredits = !!rails.credits;
  const hasMana = !!rails.mana;
  const hasCombined = !!rails.combined;
  const [picked, setPicked] = useState<Set<Ticked>>(() => seed(hasCredits, hasCombined, hasMana));
  const creditsUsable = hasCredits || hasCombined;
  const manaUsable = hasMana || hasCombined;

  // The rails follow the balances and the price as they resolve, so the ticks do too: one that stopped paying is
  // dropped, and an empty set is seeded again once a rail turns up (a set seeded before the balances landed is empty).
  useEffect(() => {
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has("credits") && !creditsUsable) next.delete("credits");
      if (next.has("mana") && !manaUsable) next.delete("mana");
      if (next.size === 0) for (const r of seed(hasCredits, hasCombined, hasMana)) next.add(r);
      // By content, not size: a swapped tick keeps the count.
      const same = next.size === prev.size && [...next].every((r) => prev.has(r));
      return same ? prev : next;
    });
  }, [creditsUsable, manaUsable, hasCredits, hasMana, hasCombined]);

  const rail = railFor(picked, rails);
  const onPick = useRef(props.onPick);
  onPick.current = props.onPick;
  useEffect(() => onPick.current(rail), [rail]);

  // Each row states its own leg: on the split, the Credits the balance gives and the MANA for the rest.
  const creditsLeg = rail === "combined" && rails.combined ? rails.combined.credits : (rails.credits?.credits ?? props.priceCredits);
  const manaLeg = rail === "combined" && rails.combined ? rails.combined.manaWei : (rails.mana?.manaWei ?? props.priceManaWei);

  const toggle = (r: Ticked) =>
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(r)) next.delete(r);
      else next.add(r);
      return next;
    });

  const tile: CSSProperties = { background: rarityCardPalette(display.rarity).background };
  const hasC = picked.has("credits");
  const hasM = picked.has("mana");
  return (
    <div className={styles.step} data-testid="payment-method-step">
      <div className={styles.head}>
        <h2 className={styles.title}>{TEXT.title}</h2>
        <button type="button" className={styles.close} onClick={props.onClose} disabled={busy} aria-label={TEXT.close} data-testid="buy-modal-close">
          <Icon name="close" className={styles.closeIco} />
        </button>
      </div>

      <div className={styles.asset} data-testid="buy-modal-asset">
        <div className={styles.thumb} style={tile}>
          <CategoryFlap item={{ category: display.category ?? "wearable", wearableCategory: display.wearableCategory, rarity: display.rarity ?? "" }} size="sm" testId="payment-flap" />
          {display.thumbnail ? <img src={display.thumbnail} alt="" /> : null}
        </div>
        <div className={styles.info}>
          <div>
            <p className={styles.name} title={display.name}>
              {display.name || TEXT.itemFallback}
            </p>
            {display.creator ? <Creator address={display.creator} /> : null}
          </div>
          <div className={styles.assetPrice}>
            <Icon name="credits" className={styles.assetPriceIco} />
            <span>{formatCredits(props.priceCredits)}</span>
          </div>
        </div>
      </div>

      <div className={styles.options}>
        <button
          type="button"
          className={styles.row}
          data-testid="pay-with-credits"
          data-selected={hasC}
          data-disabled={!creditsUsable}
          disabled={busy || !creditsUsable}
          aria-pressed={hasC}
          onClick={() => toggle("credits")}
        >
          <span className={styles.left}>
            <span className={styles.box} data-checked={hasC}>
              <Icon name="check" className={styles.boxIco} />
            </span>
          </span>
          <span className={styles.content}>
            <span className={styles.infoGroup}>
              <span className={styles.logo}>
                <img className={styles.creditsArt} src={creditsCoin} alt="" aria-hidden="true" />
              </span>
              <span className={styles.text}>
                <span className={styles.label}>{TEXT.credits}</span>
                <span className={styles.balance}>
                  {TEXT.creditsBalance}
                  <Icon name="credits" className={styles.balanceIco} />
                  <span className={styles.balanceValue}>{formatCredits(props.balanceCredits)}</span>
                </span>
              </span>
            </span>
            <span className={styles.priceCol}>
              <span className={styles.price}>
                <Icon name="credits" className={styles.priceIco} />
                <span data-testid="pay-with-credits-amount">{formatCredits(creditsLeg)}</span>
              </span>
            </span>
          </span>
        </button>

        <button
          type="button"
          className={styles.row}
          data-testid="pay-with-mana"
          data-selected={hasM}
          data-disabled={!manaUsable}
          disabled={busy || !manaUsable}
          aria-pressed={hasM}
          onClick={() => toggle("mana")}
        >
          <span className={styles.left}>
            <span className={styles.box} data-checked={hasM}>
              <Icon name="check" className={styles.boxIco} />
            </span>
          </span>
          <span className={styles.content}>
            <span className={styles.infoGroup}>
              <span className={styles.logo}>
                <img className={styles.manaArt} src={manaCoin} alt="" aria-hidden="true" data-mana />
              </span>
              <span className={styles.text}>
                <span className={styles.label}>{TEXT.mana}</span>
                <span className={styles.balance}>
                  {TEXT.manaBalance}
                  <img className={styles.manaMini} src={manaCoin} alt="" aria-hidden="true" data-mana />
                  <span className={styles.balanceValue}>{formatMana(props.manaBalanceWei as ManaWei, { group: true, decimals: 2 })}</span>
                </span>
              </span>
            </span>
            <span className={styles.priceCol}>
              {manaUsable ? (
                <span className={styles.price}>
                  <img className={styles.manaPriceIco} src={manaCoin} alt="" aria-hidden="true" data-mana />
                  <span data-testid="pay-with-mana-amount">{formatManaPrice(manaLeg as ManaWei)}</span>
                </span>
              ) : (
                // The balance is short, said as the fact it is: the Shop quotes no MANA-to-Credits rate anywhere.
                <span className={styles.detail} data-testid="mana-shortfall-note">
                  {TEXT.notEnoughMana}
                </span>
              )}
            </span>
          </span>
        </button>
      </div>

      {props.notice ? (
        <div className={styles.notice} role="status">
          {props.notice}
        </div>
      ) : null}

      <button type="button" className={styles.buy} data-testid="confirm-payment" disabled={busy || rail === null || !props.ready} onClick={() => rail && props.onBuy(rail)}>
        {TEXT.buy}
      </button>
    </div>
  );
}

function Creator({ address }: { address: string }) {
  const creator = useCreatorName(address);
  return (
    <div className={styles.creator}>
      <div>{creator ? `By ${creator}` : " "}</div>
    </div>
  );
}
