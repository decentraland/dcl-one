// from: decentraland-ui2@3.23.8:dist/components/Navbar (ShopCreditsBalanceButton, ShopCreditsIcon, formatBalance);
//       shop@3a322cf9:app/src/components/NavBar/NavBar.tsx (shopCreditsBalance, onClickShopCredits, HeldCredits)
// The signed-in buyer's credits in the global bar, before the bell. Hidden while the balance loads or when it
// fails (a 0 there would read as "you have no credits"); a loaded 0 shows. It opens the pack picker, except in
// the iOS web view, which may not sell credits: there the number shows without being a way to buy more.
import { NavCreditsBalance } from "@dcl/one-ui";
import { Link } from "react-router";
import { formatCreditsBalance } from "@shop/credits/balance-query";

import { EnsureQueryClient } from "../../checkout/services";
import HeldCredits from "../../credits/components/HeldCredits";
import { useBalance } from "../../credits/hooks";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { isIapMode } from "../flags";


function Chip() {
  const { account } = useShopWallet();
  const { data, isLoading, isError } = useBalance(account);
  if (!account || isLoading || isError) return null;
  const shown = formatCreditsBalance(data?.credits ?? 0);
  const props = { label: shown, "aria-label": `${shown} shop credits`, "data-testid": "navbar-credits" };
  return isIapMode() ? (
    <NavCreditsBalance {...props} variant="shop" as="span" />
  ) : (
    <NavCreditsBalance {...props} variant="shop" as={Link} to={`${SHOP_BASE}/credits`} />
  );
}

export default function ShopCreditsChip() {
  return (
    <EnsureQueryClient>
      <Chip />
    </EnsureQueryClient>
  );
}

function Held() {
  const { account } = useShopWallet();
  const { data } = useBalance(account);
  return <HeldCredits held={account ? data?.held : undefined} />;
}

/** Credits a purchase holds, beside the subnav's Buy Credits: the nearest surface to the balance it explains. Nothing unless some are held. */
export function ShopHeldCredits() {
  return (
    <EnsureQueryClient>
      <Held />
    </EnsureQueryClient>
  );
}
