// from: shop@2d3bf62:app/src/components/EmptyState/{EmptyState.tsx,EmptyState.styles.ts} (the signed-out panel:
//       fill + solid CTA, inside EmptyStateCentered), app/src/assets/empty/signin-empty.svg (vendored in ./assets);
//       marketplace@3bf3662f:webapp/src/components/SignInPage (decentraland-dapps SignInPage over decentraland-ui
//       SignIn: "Get Started", the star wallet, the Connect button)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
import StarWalletIcon from "../../../atoms/StarWalletIcon";
import { useShopWallet } from "../../wallet/context";
import signInIllustration from "./assets/signin-empty.svg?url";
import "./sellerpages.css";

export function ShopSignInPanel({ page, title, body, testId = "shop-signin-gate" }: { page: string; title: string; body: string; testId?: string }) {
  const { host } = useShopWallet();
  return (
    <div className="seller-page">
      <h1 className="u-sr-only">{page}</h1>
      <div className="seller-empty-centered">
        <div className="seller-empty" data-fill="true" data-testid={testId}>
          <img className="seller-empty__art" src={signInIllustration} alt="" aria-hidden="true" />
          <div className="seller-empty__text">
            <p className="seller-empty__title">{title}</p>
            <p className="seller-empty__body">{body}</p>
          </div>
          {host?.onSignIn ? (
            <button type="button" className="seller-empty__cta" onClick={() => host.onSignIn?.()}>
              Sign in
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
}

export function MarketSignIn() {
  const { host } = useShopWallet();
  return (
    <div className="seller-market-signin" data-testid="shop-signin-gate">
      <h1 className="seller-market-signin__header">Get Started</h1>
      <div className="seller-market-signin__wallet">
        <StarWalletIcon />
      </div>
      <p className="seller-market-signin__message">
        You can use the{" "}
        <a href="https://metamask.io" target="_blank" rel="noopener noreferrer">
          MetaMask
        </a>{" "}
        extension or your email account
      </p>
      {host?.onSignIn ? (
        <button type="button" className="seller-market-signin__connect" onClick={() => host.onSignIn?.()}>
          Connect
        </button>
      ) : null}
    </div>
  );
}
