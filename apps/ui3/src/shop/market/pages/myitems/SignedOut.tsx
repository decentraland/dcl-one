// from: shop@2d3bf62:app/src/pages/MyAssets.tsx (the sign-in gate), app/src/components/EmptyState/EmptyState.tsx
// signin-empty.svg from: shop@2d3bf62:app/src/assets/empty/signin-empty.svg (Apache-2.0, Metaverse Holdings Ltd.)

import { useShopWallet } from "../../../wallet/context";
import signInIllustration from "./signin-empty.svg";
import "./myitems.css";

export default function MyItemsSignedOut() {
  const { host } = useShopWallet();
  const signIn = host?.onSignIn;
  return (
    <div className="shop-myitems__centered">
      <div className="shop-myitems-empty" data-fill="true" data-testid="shop-signin-gate">
        <img className="shop-myitems-empty__art" src={signInIllustration} alt="" aria-hidden />
        <div className="shop-myitems-empty__text">
          <p className="shop-myitems-empty__title">Sign In to view your items</p>
          <p className="shop-myitems-empty__body">Sign in to see the items you own and manage them.</p>
        </div>
        {signIn ? (
          <button type="button" className="shop-myitems-empty__cta" onClick={() => signIn()}>
            Sign in
          </button>
        ) : null}
      </div>
    </div>
  );
}
