// from: marketplace@3bf3662f:webapp/src/components/SettingsPage/{SettingsPage.tsx,SettingsPage.css},
//       webapp/src/components/SettingsPage/Authorization/{Authorization.tsx,Authorization.css},
//       webapp/src/components/SignInPage (the signed-out redirect target, drawn by ../seller/pages/SellerSignIn),
//       webapp/src/modules/translation/locales/en.json (settings_page.*, authorization.authorize, networks.*),
//       webapp/src/modules/vendor/decentraland/contracts.ts (which Bids/Rentals/Marketplace each row names);
//       decentraland-transactions@0556256:src/contracts/{bid,bidV2,rentals}.ts (their addresses);
//       ui@8a63476:src/components/Radio/Radio.css + src/assets/check.svg (the radio);
//       shop@2d3bf62:app/src/App.tsx MarketplaceSettingsRedirect (/authorizations sends here);
//       shop@94584a5c:app/src/lib/authorizations.ts.
//       Marketplace UI: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd. /shop/authorizations aliases here.
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";

import Spinner from "../../atoms/Spinner";
import { shortAddress } from "../model/address";
import { CHAIN, chainLabel, type ChainId } from "../model/chain";
import { MarketSignIn } from "../seller/pages/SellerSignIn";
import { useShopWallet } from "../wallet/context";
import { ethereumChainId, settlementChainId } from "./chain/config";
import { chainContracts } from "./chain/contracts";
import {
  AuthorizationKind,
  getAuthorizationStatus,
  getManaSpendingAuthorization,
  setAuthorization,
  type ShopAuthorizationDescriptor,
} from "./lib/authorizations";
import { friendlyError, isRejection } from "./lib/errors";
import { isWrongNetworkError } from "./lib/network";
import { showsWalletConfirmations } from "./lib/wallet-kind";
import FlowBanner from "./components/FlowBanner";
import Blockie from "./settings/Blockie";
import { EnsureQueryClient, useShopServices } from "./services";
import "./settingspage.css";

type Grant = ShopAuthorizationDescriptor & { spenderName: string; tokenName: string };

const BIDS: Partial<Record<number, string>> = {
  [CHAIN.ETHEREUM]: "0xe479dfd9664c693b2e2992300930b00bfde08233",
  [CHAIN.SEPOLIA]: "0x2c2835b95852fd975e087b3b25297322728792e2",
  [CHAIN.POLYGON]: "0xb96697fa4a3361ba35b774a42c58daccaad1b8e1",
  [CHAIN.AMOY]: "0x4b66eab79cc03a96fb6275cfcdf23c0db431606d",
};
const RENTALS: Partial<Record<number, string>> = {
  [CHAIN.ETHEREUM]: "0x3a1469499d0be105d4f77045ca403a5f6dc2f3f5",
  [CHAIN.SEPOLIA]: "0xe70db6319e9cee3f604909bdade58d1f5c1cf702",
};

const HELD_COLLECTIBLES_TO_SCAN = 1000;

function mana(chainId: ChainId, spender: string, spenderName: string, section: string): Grant {
  return {
    ...getManaSpendingAuthorization(chainId, spender),
    id: `${section}:${chainId}:${spender.toLowerCase()}`,
    group: "buying",
    spenderName,
    tokenName: "MANA",
  };
}

function buyingGrants(eth: ChainId, pol: ChainId): Grant[] {
  const e = chainContracts(eth);
  const p = chainContracts(pol);
  const out: Grant[] = [];
  if (e?.legacyMarketplace) out.push(mana(eth, e.legacyMarketplace.address, e.legacyMarketplace.label, "buying"));
  if (p?.legacyMarketplace) out.push(mana(pol, p.legacyMarketplace.address, p.legacyMarketplace.label, "buying"));
  for (const m of e?.offChainMarketplaces ?? []) out.push(mana(eth, m.address, m.label, "buying"));
  for (const m of p?.offChainMarketplaces ?? []) out.push(mana(pol, m.address, m.label, "buying"));
  if (p?.collectionStore) out.push(mana(pol, p.collectionStore.address, p.collectionStore.label, "buying"));
  if (p?.creditsManager) out.push(mana(pol, p.creditsManager.address, p.creditsManager.label, "buying"));
  return out;
}

function explorerAddressUrl(chainId: ChainId, address: string): string | null {
  const base =
    chainId === CHAIN.POLYGON
      ? "https://polygonscan.com"
      : chainId === CHAIN.AMOY
        ? "https://amoy.polygonscan.com"
        : chainId === CHAIN.ETHEREUM
          ? "https://etherscan.io"
          : chainId === CHAIN.SEPOLIA
            ? "https://sepolia.etherscan.io"
            : null;
  return base ? `${base}/address/${address}` : null;
}

function statusKey(auth: ShopAuthorizationDescriptor, account: string) {
  return ["shop-authorization", auth.id, account] as const;
}

function Authorization({ grant, account, canChange }: { grant: Grant; account: string; canChange: boolean }) {
  const services = useShopServices();
  const wallet = useShopWallet();
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ text: string; switchTo?: number } | null>(null);
  const status = useQuery({
    queryKey: statusKey(grant, account),
    queryFn: () => getAuthorizationStatus(grant, account, undefined, services.reader),
    staleTime: 30_000,
  });
  const granted = status.data === true;
  const pending = busy || status.isLoading;

  async function change(active: boolean) {
    const provider = wallet.host?.provider?.() ?? null;
    if (!provider) return;
    setBusy(true);
    setError(null);
    try {
      await setAuthorization({
        auth: grant,
        active,
        provider,
        account,
        walletKind: wallet.host?.walletKind ?? null,
        reader: services.reader,
        relayerUrl: services.relayerUrl,
        fetchImpl: services.fetchImpl,
      });
      await qc.invalidateQueries({ queryKey: ["shop-authorization", grant.id] });
    } catch (e) {
      if (isWrongNetworkError(e)) setError({ text: `Switch your wallet to ${chainLabel(e.required)} to change this.`, switchTo: e.required });
      else setError({ text: isRejection(e) ? "You declined it in your wallet." : friendlyError(e, "That didn't go through. Try again.") });
    } finally {
      setBusy(false);
    }
  }

  const href = explorerAddressUrl(grant.chainId, grant.spenderAddress);
  const network = chainLabel(grant.chainId);
  return (
    <div className="shop-settings-auth" data-testid="authorization" data-granted={granted || undefined} data-pending={pending || undefined}>
      <div className="shop-settings-auth__field">
        {pending ? (
          <span className="shop-settings-auth__loader" title={busy ? "Transaction pending" : undefined}>
            <Spinner size={16} />
          </span>
        ) : (
          <button
            type="button"
            role="switch"
            className="shop-settings-auth__radio"
            aria-checked={granted}
            aria-label={`${grant.tokenName} for ${grant.spenderName} on ${network}`}
            disabled={!canChange || status.isError}
            onClick={() => void change(!granted)}
          />
        )}
        <span className="shop-settings-auth__label">{grant.tokenName}</span>
        <span className="u-sr-only" role="status">
          {status.isLoading ? "Checking" : status.isError ? "Unknown" : busy ? "Confirm in your wallet" : granted ? "Authorized" : "Not authorized"}
        </span>
      </div>
      <div className="shop-settings-auth__description">
        Authorize the{" "}
        {href ? (
          <a href={href} target="_blank" rel="noopener noreferrer">
            {grant.spenderName}
          </a>
        ) : (
          grant.spenderName
        )}{" "}
        contract to operate {grant.tokenName} on your behalf on {network}
      </div>
      {error ? (
        <div className="shop-settings-auth__error" role="alert">
          {error.text}{" "}
          {error.switchTo ? (
            <button type="button" className="shop-settings__link" onClick={() => void wallet.switchTo(error.switchTo!)} disabled={wallet.pendingChainId != null}>
              Switch to {chainLabel(error.switchTo)}
            </button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function Group({ label, grants, account, canChange }: { label: string; grants: Grant[]; account: string; canChange: boolean }) {
  return (
    <div className="shop-settings__checks" role="group" aria-label={label}>
      <span className="shop-settings__group">{label}</span>
      {grants.map((g) => (
        <Authorization key={g.id} grant={g} account={account} canChange={canChange} />
      ))}
    </div>
  );
}

function useCopied(ms: number): [boolean, () => void] {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  return [
    copied,
    () => {
      setCopied(true);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), ms);
    },
  ];
}

function SettingsScreen() {
  const services = useShopServices();
  const wallet = useShopWallet();
  const account = wallet.account;
  const selfCustody = showsWalletConfirmations(wallet.host?.walletKind ?? null);
  const eth = ethereumChainId();
  const pol = settlementChainId();
  const [copied, markCopied] = useCopied(1200);

  const buying = useMemo(() => buyingGrants(eth, pol), [eth, pol]);
  const bidding = useMemo(
    () => [
      ...(BIDS[eth] ? [mana(eth, BIDS[eth]!, "Bids", "bidding")] : []),
      ...(BIDS[pol] ? [mana(pol, BIDS[pol]!, "Bids", "bidding")] : []),
    ],
    [eth, pol],
  );
  const renting = useMemo(() => (RENTALS[eth] ? [mana(eth, RENTALS[eth]!, "Rentals", "renting")] : []), [eth]);

  const held = useQuery({
    queryKey: ["shop-held-collections", account],
    enabled: !!account,
    queryFn: async (): Promise<Grant[]> => {
      const body = await services.client.market.nfts<{ data?: { nft?: { contractAddress?: string; chainId?: number } }[] }>({
        owner: account!,
        first: HELD_COLLECTIBLES_TO_SCAN,
      });
      const seen = new Set<string>();
      const out: Grant[] = [];
      for (const row of body?.data ?? []) {
        const contract = row.nft?.contractAddress?.toLowerCase();
        const chainId = row.nft?.chainId;
        if (!contract || typeof chainId !== "number" || seen.has(`${chainId}:${contract}`)) continue;
        seen.add(`${chainId}:${contract}`);
        for (const m of chainContracts(chainId)?.offChainMarketplaces ?? []) {
          out.push({
            id: `selling:${chainId}:${contract}:${m.address}`,
            group: "selling",
            kind: AuthorizationKind.Approval,
            contractAddress: contract,
            spenderAddress: m.address,
            chainId,
            spenderName: m.label,
            tokenName: shortAddress(contract),
          });
        }
      }
      return out;
    },
  });
  const sellingStatus = useQueries({
    queries: (held.data ?? []).map((g) => ({
      queryKey: statusKey(g, account ?? ""),
      queryFn: () => getAuthorizationStatus(g, account!, undefined, services.reader),
      staleTime: 30_000,
      enabled: !!account,
    })),
  });
  const selling = (held.data ?? []).filter((_, i) => sellingStatus[i]?.data === true);

  if (!account) {
    return (
      <div className="shop-settings shop-settings--signed-out" data-testid="settings-signed-out">
        <MarketSignIn />
      </div>
    );
  }

  return (
    <main className="shop-settings" aria-labelledby="shop-settings-title">
      <h1 id="shop-settings-title" className="u-sr-only">
        Settings
      </h1>
      <div className="shop-settings__row">
        <div className="shop-settings__label">Address</div>
        <div className="shop-settings__value shop-settings__address-row">
          <Blockie className="shop-settings__blockie" seed={account} scale={12} />
          <div className="shop-settings__address-container">
            <div className="shop-settings__address" title={account}>
              <span className="shop-settings__address-full">{account}</span>
              <span className="shop-settings__address-short">{shortAddress(account)}</span>
            </div>
            {copied ? (
              <span className="shop-settings__copy">Copied ✔</span>
            ) : (
              <button
                type="button"
                className="shop-settings__copy shop-settings__link"
                onClick={() => {
                  void navigator.clipboard?.writeText(account).catch(() => undefined);
                  markCopied();
                }}
              >
                Copy address
              </button>
            )}
          </div>
        </div>
      </div>
      <div className="shop-settings__row">
        <div className="shop-settings__label">Authorizations</div>
        <div className="shop-settings__value">
          {!selfCustody ? (
            <FlowBanner tone="info" title="Handled for you">
              Your account approves what each purchase needs as you buy, so there is nothing to set up here.
            </FlowBanner>
          ) : null}
          <Group label="For buying" grants={buying} account={account} canChange={selfCustody} />
          <Group label="For bidding" grants={bidding} account={account} canChange={selfCustody} />
          <Group label="For renting" grants={renting} account={account} canChange={selfCustody} />
          {selling.length > 0 ? <Group label="For selling" grants={selling} account={account} canChange={selfCustody} /> : null}
        </div>
      </div>
    </main>
  );
}

export default function SettingsPage() {
  return (
    <EnsureQueryClient>
      <SettingsScreen />
    </EnsureQueryClient>
  );
}
