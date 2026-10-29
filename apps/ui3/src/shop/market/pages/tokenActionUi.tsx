// from: marketplace@3bf3662f:webapp/src/components/AssetAction (AssetAction.css), ManaField, ConfirmInputValueModal,
//       Wallet (the signed-out redirect of ProtectedRoute), lib/date (formatDistanceToNow);
//       decentraland-ui Field / Button (dark theme)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace, decentraland-ui), Apache-2.0.

import type { ButtonHTMLAttributes, CSSProperties, ReactNode } from "react";
import { useState } from "react";

import ManaMark from "../../../atoms/ManaMark";
import Modal from "../../../components/Modal";
import { rarityMedia } from "../../discovery/lib/rarity";
import { isPolygon } from "../../model/chain";
import SignInGate from "../../land/SignInGate";
import { useShopWallet } from "../../wallet/context";
import "./tokenactions.css";

export function TokenActionRoot({ children, rarity }: { children: ReactNode; rarity?: string | null }) {
  const media = rarity ? rarityMedia(rarity) : "none";
  return (
    <div className="shop-ta" style={media !== "none" ? ({ "--ta-media": `${media}, #242129` } as CSSProperties) : undefined}>
      {children}
    </div>
  );
}

export function RequireSignIn({ children }: { children: ReactNode }) {
  const { account, host } = useShopWallet();
  if (!account)
    return (
      <div className="shop-ta-signin">
        <SignInGate onSignIn={host?.onSignIn} />
      </div>
    );
  return <>{children}</>;
}

export function manaNetwork(chainId: number): "polygon" | "ethereum" {
  return isPolygon(chainId) ? "polygon" : "ethereum";
}

export function TaField({
  label,
  htmlFor,
  invalid,
  message,
  mana,
  children,
}: {
  label: ReactNode;
  htmlFor?: string;
  invalid?: boolean;
  message?: ReactNode;
  mana?: "polygon" | "ethereum";
  children: ReactNode;
}) {
  return (
    <div className="shop-ta-field">
      <label className="shop-ta-field__label" htmlFor={htmlFor}>
        {label}
      </label>
      <span className="shop-ta-field__control" data-invalid={invalid ? "" : undefined}>
        {mana ? <ManaMark size={16} network={mana} /> : null}
        {children}
      </span>
      {message ? <p className={invalid ? "shop-ta-field__error" : "shop-ta-field__help"}>{message}</p> : null}
    </div>
  );
}

export function TaButton({ primary, className = "", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean }) {
  return <button type="button" className={"shop-ta-btn" + (primary ? " shop-ta-btn--primary" : "") + (className ? " " + className : "")} {...rest} />;
}

export function ConfirmValueModal({
  title = "Please confirm",
  content,
  value,
  label = "Price",
  mana,
  busy,
  footer,
  onConfirm,
  onCancel,
  testId,
}: {
  title?: string;
  content: ReactNode;
  value: string | null;
  label?: string;
  mana?: "polygon" | "ethereum";
  busy?: boolean;
  footer?: ReactNode;
  onConfirm: () => void;
  onCancel: () => void;
  testId?: string;
}) {
  const [typed, setTyped] = useState("");
  const expected = value === null ? null : String(parseFloat(value));
  const matches = expected === null || (typed.trim() !== "" && String(parseFloat(typed)) === expected);
  return (
    <Modal onClose={busy ? undefined : onCancel} className="shop-ta-confirm" width={560} closeOnBackdrop={!busy} closeOnEsc={!busy}>
      <div data-testid={testId}>
        <h2 className="shop-ta-confirm__title">{title}</h2>
        <div className="shop-ta-confirm__content">{content}</div>
        {expected !== null ? (
          <label className="shop-ta-confirm__field">
            <span className="shop-ta-confirm__label">{label}</span>
            <span className="shop-ta-confirm__control">
              {mana ? <ManaMark size={16} network={mana} /> : null}
              <input inputMode="decimal" value={typed} placeholder={expected} aria-label={`Confirm the ${label.toLowerCase()}`} disabled={busy} onChange={(e) => setTyped(e.target.value.replace(/[^\d.]/g, ""))} autoFocus />
            </span>
          </label>
        ) : null}
        {footer}
        <div className="shop-ta-confirm__actions">
          <TaButton
            disabled={busy}
            onClick={() => {
              setTyped("");
              onCancel();
            }}
          >
            Cancel
          </TaButton>
          <TaButton primary disabled={busy || !matches} onClick={onConfirm}>
            Proceed
          </TaButton>
        </div>
      </div>
    </Modal>
  );
}

const UNITS: [number, string][] = [
  [365 * 86_400_000, "year"],
  [30 * 86_400_000, "month"],
  [86_400_000, "day"],
  [3_600_000, "hour"],
  [60_000, "minute"],
];

export function distanceToNow(ms: number, { addSuffix = false, now = Date.now() }: { addSuffix?: boolean; now?: number } = {}): string {
  const diff = ms - now;
  const abs = Math.abs(diff);
  let text = "less than a minute";
  for (const [size, unit] of UNITS) {
    if (abs >= size) {
      const n = Math.round(abs / size);
      text = `${unit === "hour" || unit === "month" || unit === "year" ? "about " : ""}${n} ${unit}${n === 1 ? "" : "s"}`;
      break;
    }
  }
  if (!addSuffix) return text;
  return diff < 0 ? `${text} ago` : `in ${text}`;
}

export function categoryLabel(category: string): string {
  switch (category) {
    case "wearable":
      return "Wearable";
    case "emote":
      return "Emote";
    case "ens":
      return "Name";
    case "parcel":
      return "Parcel";
    case "estate":
      return "Estate";
    default:
      return "Item";
  }
}

export function dateInputValue(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function dateInputMs(value: string): number {
  const [y, m, d] = value.split("-").map(Number);
  return new Date(y!, (m ?? 1) - 1, d ?? 1).getTime();
}
