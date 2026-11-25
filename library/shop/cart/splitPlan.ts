// How Credits and MANA pay one cart together: which lines Credits pay whole (K), which one line, if any, Credits and MANA
// pay together (S), and which lines MANA pays (M). Deterministic and in cart order; it is not the best split, only a stable
// one. The host runs the parts and works out S's MANA amounts from the sale's exact price; this module only places lines.
//
// A line's cost in Credits is what partitionCart charges for it: each unit's quote up to the next whole credit, times its
// units. S's Credits part is the whole credits left, against the line's unit quote as quoted (not rounded up).

import {
  addCents,
  addMana,
  subCents,
  wholeCreditsFloor,
  ZERO_CENTS,
  ZERO_MANA,
  type ManaWei,
  type UsdCents,
} from "../model/money";
import { offerQuote } from "../model/purchase";
import { hasWallet, type PaymentMethod, type Session } from "../model/settlement";
import { lineUnits, partitionCart, type CartGroup, type LineRefusal, type PricedLine } from "./partition";

export type CartSplitLine = {
  lineId: string;
  /** What Credits pay for it: whole credits only, more than nothing and less than `priceCents`. */
  legCents: UsdCents;
  /** The line's Credits quote for its one unit, not rounded up: the price the credit is authorized against. */
  priceCents: UsdCents;
};

export type CartSplitPlan = {
  /** Lines Credits pay whole, in cart order. */
  credits: string[];
  /** The one line Credits and MANA pay together: a single unit the server marks `splittable`. */
  split: CartSplitLine | null;
  /** Lines MANA pays whole, in cart order. */
  mana: string[];
  /** What the plan takes from the Credits balance: the `credits` lines' charge plus `split.legCents`. */
  creditsCents: UsdCents;
};

export type CartSplitRefusal =
  | { reason: "empty" }
  /** Only a session with a web3 wallet pays MANA, so only it can have a split. */
  | { reason: "noWallet" }
  | { reason: "duplicateLine"; lineId: string }
  /** Neither Credits nor MANA can pay this line: the cart has no split. */
  | { reason: "noMethod"; lineId: string }
  /** The lines only Credits can pay cost more than the balance. */
  | { reason: "creditsShort"; required: UsdCents; balance: UsdCents }
  /** The parts, partitioned as checkout would, refuse these lines (a Credits group over its unit cap, say). */
  | { reason: "refused"; refusals: LineRefusal[] }
  /** A sum of the plan does not fit its type: no plan is given on a total that cannot be told. */
  | { reason: "overflow" }
  /** Credits pay nothing: the cart pays as it does without a split. */
  | { reason: "noCreditsPart" }
  /** Credits pay everything: the cart pays as it does without a split. */
  | { reason: "noManaPart" }
  /** The MANA lines alone (S's MANA part not counted) cost more than the wallet holds. */
  | { reason: "manaShort"; required: ManaWei; balance: ManaWei };

export type CartSplitResult = { ok: true; plan: CartSplitPlan } | { ok: false; refusal: CartSplitRefusal };

export type CartSplitBalances = {
  /** The spendable Credits balance, held credits already taken off. */
  creditsBalance: UsdCents;
  /** The wallet's MANA, for a first check of the MANA lines; the host quotes them for real. */
  manaBalance?: ManaWei | null;
};

// The cart's own partition already reports an inconsistent offer; these partitions would only repeat it.
const quiet = { report: () => {} };

function as(entry: PricedLine, method: PaymentMethod): PricedLine {
  return { line: { ...entry.line, method }, offer: entry.offer };
}

type Alone = {
  /** Nothing but a Credits group's unit cap refuses the line. */
  pays: boolean;
  /** What checkout charges for it in Credits (each unit rounded up to a whole credit), when Credits pay it. */
  charged: UsdCents | null;
  overflow: boolean;
};

/**
 * The line partitioned alone with `method`, as checkout would. A Credits group's unit cap is not the line's own: a line
 * over it alone still goes where the walk puts it, and the parts' partition refuses it there.
 */
function alone(entry: PricedLine, method: PaymentMethod, session: Session): Alone {
  const { groups, refused } = partitionCart([as(entry, method)], session, quiet);
  const pays = refused.every((r) => r.code === "groupTooLarge");
  const total = groups[0]?.total;
  return {
    pays,
    charged: pays && total?.method === "credits" ? total.charged : null,
    overflow: refused.some((r) => r.code === "amountOverflow"),
  };
}

/** Across groups, partitionCart's totals keep the last sum that fit; a plan needs the whole one. */
function manaTotal(groups: CartGroup[]): ManaWei | null {
  let sum: ManaWei | null = ZERO_MANA;
  for (const { total } of groups) if (total.method === "mana" && sum !== null) sum = addMana(sum, total.amount);
  return sum;
}

export function planCartSplit(priced: PricedLine[], session: Session, balances: CartSplitBalances): CartSplitResult {
  const no = (refusal: CartSplitRefusal): CartSplitResult => ({ ok: false, refusal });
  if (priced.length === 0) return no({ reason: "empty" });
  if (!hasWallet(session)) return no({ reason: "noWallet" });
  const ids = new Set<string>();
  for (const { line } of priced) {
    if (ids.has(line.id)) return no({ reason: "duplicateLine", lineId: line.id });
    ids.add(line.id);
  }

  const lanes = priced.map((entry) => {
    const byCredits = alone(entry, "credits", session);
    const byMana = alone(entry, "mana", session);
    return { entry, cost: byCredits.charged, manaPays: byMana.pays, overflow: byCredits.overflow || byMana.overflow };
  });
  const none = lanes.find((l) => l.cost === null && !l.manaPays);
  if (none) return no(none.overflow ? { reason: "overflow" } : { reason: "noMethod", lineId: none.entry.line.id });

  // Lines only Credits can pay come first: without them covered there is no split.
  const given = balances.creditsBalance;
  const balance = Number.isSafeInteger(given) && given > 0 ? given : ZERO_CENTS;
  let required: UsdCents | null = ZERO_CENTS;
  for (const { cost, manaPays } of lanes) {
    if (cost !== null && !manaPays && required !== null) required = addCents(required, cost);
  }
  if (required === null) return no({ reason: "overflow" });
  if (required > balance) return no({ reason: "creditsShort", required, balance });
  let left = subCents(balance, required)!;

  const credits: PricedLine[] = [];
  const mana: PricedLine[] = [];
  let split: CartSplitLine | null = null;
  // Then, in cart order: a line whose cost fits what is left goes to Credits. The first that does not fit and can be split
  // takes the whole credits left, and MANA pays every line after it. One that cannot be split (several units, a price the
  // node does not split) goes to MANA, and the walk goes on.
  // The walk does not count a Credits group's units, on purpose. A line the balance covers goes to Credits even if its group
  // then passes the cap, and the parts' partition refuses the plan naming that line, rather than the walk moving it to MANA
  // unasked. So the result is not monotonic in the balance: more Credits can mean no plan where fewer would send that line to
  // MANA and give one. The in-world cart never meets this: it holds at most `MAX_PURCHASE_UNITS` units (`cart/lines.ts`),
  // the same 30.
  for (const { entry, cost, manaPays } of lanes) {
    if (cost !== null && !manaPays) {
      credits.push(entry);
      continue;
    }
    if (cost === null || split) {
      mana.push(entry);
      continue;
    }
    if (cost <= left) {
      credits.push(entry);
      left = subCents(left, cost)!;
      continue;
    }
    const quote = entry.offer ? offerQuote(entry.offer, "credits") : null;
    const leg = wholeCreditsFloor(left);
    if (lineUnits(entry.line) === 1 && entry.offer?.splittable === true && quote?.method === "credits" && leg > 0 && leg < quote.quoted) {
      split = { lineId: entry.line.id, legCents: leg, priceCents: quote.quoted };
      left = subCents(left, leg)!;
      continue;
    }
    mana.push(entry);
  }

  if (credits.length === 0 && !split) return no({ reason: "noCreditsPart" });
  if (mana.length === 0 && !split) return no({ reason: "noManaPart" });

  const cartIndex = new Map(priced.map((p, i) => [p.line.id, i]));
  const inCart = (r: LineRefusal): LineRefusal => ({ ...r, index: cartIndex.get(r.lineId) ?? r.index });
  const byCredits = partitionCart(credits.map((e) => as(e, "credits")), session, quiet);
  const byMana = partitionCart(mana.map((e) => as(e, "mana")), session, quiet);
  const refusals = [...byCredits.refused, ...byMana.refused].map(inCart).sort((a, b) => a.index - b.index);
  if (refusals.length > 0) return no({ reason: "refused", refusals });

  const manaRequired = manaTotal(byMana.groups);
  if (manaRequired === null) return no({ reason: "overflow" });
  const manaBalance = balances.manaBalance;
  if (manaBalance != null && manaRequired > manaBalance) {
    return no({ reason: "manaShort", required: manaRequired, balance: manaBalance });
  }

  // Every line in `credits` cost what it took from what was left, and the split line its part: what the plan takes.
  const creditsCents = subCents(balance, left)!;
  const lineIds = (lines: PricedLine[]) => lines.map((e) => e.line.id);
  return { ok: true, plan: { credits: lineIds(credits), split, mana: lineIds(mana), creditsCents } };
}
