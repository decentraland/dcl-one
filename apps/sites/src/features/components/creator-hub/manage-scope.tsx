import { useEffect } from "react";
import type { ReactNode } from "react";
import { useSearchParams } from "react-router";

import Button from "@ui/atoms/Button";
import EmptyState from "@ui/components/EmptyState";
import { useAuth } from "@/data/lib/auth/index";
import { openSignIn } from "../auth/signin-store";

/** The Manage pages (Worlds, LAND, NAMEs) scope their loader by wallet: the `dcl_wallet`
    cookie or an explicit `?address=`. A wallet that is signed in only in the browser is
    written into the URL once, so the server can load for it and the link stays shareable. */
export function useManageScope(address: string) {
  const [, setSearchParams] = useSearchParams();
  const { isConnected, address: walletAddress } = useAuth();
  const rescoping = isConnected && Boolean(walletAddress) && !address;

  useEffect(() => {
    if (!rescoping || !walletAddress) return;
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.set("address", walletAddress);
        return next;
      },
      { replace: true, preventScrollReset: true },
    );
  }, [rescoping, walletAddress, setSearchParams]);

  return {
    rescoping,
    needsConnect: !address && !rescoping,
    isConnected,
    walletAddress,
  };
}

export function ManageLoading({ label }: { label: string }) {
  return (
    <div
      className="chm__loader"
      style={{ flexDirection: "column", gap: 14 }}
      role="status"
      aria-live="polite"
    >
      <span className="chm__spinner" aria-hidden="true" />
      <p style={{ margin: 0, color: "var(--ink-45)", fontSize: 14 }}>{label}</p>
    </div>
  );
}

export function ManageSignIn({
  icon,
  title,
  subtitle,
}: {
  icon: ReactNode;
  title: string;
  subtitle: string;
}) {
  return (
    <EmptyState
      className="es--card"
      icon={icon}
      iconWash
      title={title}
      subtitle={subtitle}
      actions={
        <Button variant="secondary" onClick={() => openSignIn()}>
          Sign in
        </Button>
      }
      variant={undefined}
      tone={undefined}
      actionsGap={undefined}
      style={undefined}
    />
  );
}

export function ManageError({
  what,
  onRetry,
  retrying,
}: {
  what: string;
  onRetry: () => void;
  retrying: boolean;
}) {
  return (
    <div
      className="chm__sim"
      role="alert"
      style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap" }}
    >
      <span>
        <strong>Couldn&apos;t load {what}.</strong> The service was unreachable just now, so
        this list is empty rather than showing sample data.
      </span>
      <button
        type="button"
        onClick={onRetry}
        disabled={retrying}
        style={{
          flex: "0 0 auto",
          border: "1px solid color-mix(in srgb, var(--error) 45%, transparent)",
          background: "color-mix(in srgb, var(--error) 16%, transparent)",
          color: "inherit",
          borderRadius: "var(--r-control)",
          padding: "6px 14px",
          fontSize: 13,
          fontWeight: 600,
          cursor: "pointer",
        }}
      >
        {retrying ? "Retrying\u{2026}" : "Try again"}
      </button>
    </div>
  );
}
