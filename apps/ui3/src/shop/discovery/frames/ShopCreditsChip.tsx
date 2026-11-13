// from: decentraland-ui2@3.23.8:dist/components/Navbar (ShopCreditsBalanceButton, ShopCreditsIcon, formatBalance);
//       shop@3a322cf9:app/src/components/NavBar/NavBar.tsx (shopCreditsBalance, onClickShopCredits, HeldCredits)
// The signed-in buyer's credits in the global bar, before the bell. Hidden while the balance loads or when it
// fails (a 0 there would read as "you have no credits"); a loaded 0 shows. It opens the pack picker, except in
// the iOS web view, which may not sell credits: there the number shows without being a way to buy more.
import { Link } from "react-router";

import { EnsureQueryClient } from "../../checkout/services";
import HeldCredits from "../../credits/components/HeldCredits";
import { useBalance } from "../../credits/hooks";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { isIapMode } from "../flags";
import styles from "./ShopCreditsChip.module.css";

/** As ui2's formatBalance: whole credits, grouped ("1,234"). */
export function formatCreditsBalance(credits: number): string {
  return Math.floor(credits).toLocaleString("en-US");
}

/** The shop's hexagonal "C" mark, drawn in the chip's colour. */
const ShopCreditsIcon = () => (
  <svg width="20" height="20" viewBox="0 0 20 21" fill="none" aria-hidden="true" focusable="false">
    <path d="M13.1318 0.891714L18.5 6.29015V14.0226L13.125 19.5831H6.26758L0.891602 14.0216V6.29113L6.26074 0.891714H13.1318Z" stroke="currentColor" strokeWidth="1.78379" />
    <path
      d="M7.81758 5.13837C8.7606 4.70684 9.80743 4.55428 10.8344 4.69871C11.8613 4.84315 12.8255 5.27853 13.6129 5.95344L12.0562 7.76966C11.6155 7.39196 11.076 7.1483 10.5012 7.06747C9.92652 6.98664 9.34068 7.07201 8.81294 7.31351C8.28519 7.555 7.8376 7.94253 7.52307 8.43029C7.20854 8.91805 7.0402 9.48566 7.03796 10.066C7.03572 10.6464 7.19967 11.2153 7.51042 11.7054C7.82117 12.1956 8.26576 12.5866 8.79162 12.8322C9.31748 13.0777 9.90265 13.1676 10.478 13.0912C11.0533 13.0148 11.5947 12.7753 12.0383 12.4011L13.581 14.2293C12.7884 14.8981 11.8209 15.326 10.7928 15.4625C9.7648 15.599 8.71916 15.4384 7.7795 14.9996C6.83984 14.5608 6.04542 13.8621 5.49014 12.9863C4.93485 12.1104 4.64191 11.0938 4.64592 10.0568C4.64992 9.01972 4.95071 8.00549 5.51274 7.13393C6.07478 6.26236 6.87456 5.5699 7.81758 5.13837Z"
      fill="currentColor"
    />
  </svg>
);

function Chip() {
  const { account } = useShopWallet();
  const { data, isLoading, isError } = useBalance(account);
  if (!account || isLoading || isError) return null;
  const shown = formatCreditsBalance(data?.credits ?? 0);
  const label = `${shown} shop credits`;
  const body = (
    <>
      <ShopCreditsIcon />
      {shown}
    </>
  );
  return isIapMode() ? (
    <span className={styles.chip} aria-label={label} data-testid="navbar-credits">
      {body}
    </span>
  ) : (
    <Link to={`${SHOP_BASE}/credits`} className={styles.chip} aria-label={label} data-testid="navbar-credits" data-link>
      {body}
    </Link>
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
