// from: sites@17b3535a:src/components/LandingNavbar/LandingNavbar.tsx (CreditsChip, CreditsIcon, formatCredits) and
//       LandingNavbar.styled.ts (CreditsChip).
// The signed-in visitor's credits in the sites bar, before the bell: the library's NavCreditsBalance linking to the
// credits page. Hidden while the balance loads or when it fails, as the shop's chip is.
import { NavCreditsBalance } from "@dcl/one-ui/components";
import { Link } from "react-router";

import { EnsureQueryClient } from "../../shop/checkout/services";
import { useBalance } from "../../shop/credits/hooks";
import { formatCreditsBalance } from "../../shop/discovery/frames/ShopCreditsChip";
import { SHOP_BASE } from "../../shop/routes";

function Chip({ account }: { account: string }) {
  const { data, isLoading, isError } = useBalance(account);
  if (isLoading || isError) return null;
  const shown = formatCreditsBalance(data?.credits ?? 0);
  return (
    <NavCreditsBalance as={Link} to={`${SHOP_BASE}/credits`} label={shown} aria-label={`${shown} credits`} />
  );
}

export default function SitesCreditsChip({ account }: { account: string }) {
  return (
    <EnsureQueryClient>
      <Chip account={account} />
    </EnsureQueryClient>
  );
}
