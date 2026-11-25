import { ShopHeldCredits } from "@dcl/one-ui";
import type { HeldCredits as Held } from "@shop/credits/credits";

import { ClockIcon } from "../../../atoms/icons";
import { CreditsAmount } from "./Amount";

export default function HeldCredits({ held, now }: { held: Held | undefined; now?: number }) {
  if (!held || held.credits <= 0) return null;
  return (
    <ShopHeldCredits credits={held.credits} purchaseCount={held.purchases.length} releasesAtSeconds={held.releasesAtSeconds ?? null} now={now}>
      <ClockIcon size={14} />
      <CreditsAmount credits={held.credits} size={12} />
    </ShopHeldCredits>
  );
}
