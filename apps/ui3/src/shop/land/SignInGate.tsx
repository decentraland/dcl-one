// from: marketplace@3bf3662f:webapp/src/components/SignInPage/SignInPage.{tsx,css}; decentraland-ui
// src/components/SignIn/SignIn.{tsx,css}; decentraland-dapps en.json `sign_in` copy.
// Copyright 2020 Metaverse Holdings Ltd. (marketplace, decentraland-ui, decentraland-dapps), Apache-2.0.
import StarWalletIcon from "../../atoms/StarWalletIcon";
import "./signingate.css";

export default function SignInGate({ onSignIn, connecting = false }: { onSignIn?: () => void; connecting?: boolean }) {
  return (
    <div className="shop-land-signin" data-testid="land-sign-in">
      <div className="shop-land-signin__card">
        <h1 className="shop-land-signin__header">Get Started</h1>
        <div className="shop-land-signin__icon">
          <StarWalletIcon />
        </div>
        <p className="shop-land-signin__message">
          You can use the{" "}
          <a href="https://metamask.io" target="_blank" rel="noopener noreferrer">
            MetaMask
          </a>{" "}
          extension or your email account
        </p>
        <button type="button" className="shop-land-signin__connect" onClick={() => onSignIn?.()} disabled={!onSignIn || connecting}>
          {connecting ? "Connecting..." : "Connect"}
        </button>
      </div>
    </div>
  );
}
