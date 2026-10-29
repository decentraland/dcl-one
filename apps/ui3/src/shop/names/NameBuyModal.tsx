// from: shop@2d3bf62:app/src/components/NameBuyModal/NameBuyModal.tsx, marketplace@f3956826:
// webapp/src/components/Modals/ClaimNameFatFingerModal/ClaimNameFatFingerModal.tsx (the re-entry gate),
// catalyrst-v2 ui3/src/marketplace/workflows/MkClaimNameWizardView.tsx and pages/MkClaimNamePage.tsx (superseded)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace sources). Licensed under the Apache License, Version 2.0.
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";

import Button from "../../atoms/Button";
import Modal from "../../components/Modal";
import type { UnityCreditPack } from "@catalyst-api/credits/UnityCreditPack";
import { isShopApiError } from "../api/client";
import { shopClient, type ShopClient } from "../api/shop";
import { chainLabel } from "../model/chain";
import { creditsCeil, creditsFloor, formatMana, usdCents, type ManaWei } from "../model/money";
import { offerQuote, type PurchaseOffer } from "../model/purchase";
import { outageText, optionStateText } from "../payment/copy";
import { presentPayment } from "../payment/present";
import { landChainId, landNetwork } from "../land/contracts";
import { erc20Abi, readContract, rpcReader, txHref, TxPendingError, type ChainReader } from "../land/evm";
import { useLandWallet } from "../land/useLandWallet";
import { useResource } from "../land/useResource";
import { metaTxProviderShim } from "../checkout/lib/authorizations";
import {
  relayedUseCreditsSubmitter,
  walletUseCreditsSubmitter,
  creditsChainFor,
  InsufficientManaError,
  NameCreditsClosedError,
  NameNotRegisteredError,
  NameRouteCostTooHighError,
  NameSettlementUnknownError,
  NameTakenError,
  registerNameWithEthereumMana,
  registerNameWithUsdCredits,
  type NameRegistrationStage,
  type UseCreditsSubmitter,
} from "./names";
import { stashNameResume, clearNameResume } from "./resume";
import "./names.css";

type Rail = "credits" | "mana" | "mixed";
type Phase = "choose" | "confirm" | "completing" | "success" | "pending" | "error";

const POLYGON_MANA: Record<number, string> = { 137: "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", 80002: "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0" };

export type NameBuyModalProps = {
  name: string;
  offer: PurchaseOffer;
  onClose(): void;
  client?: ShopClient;
  readerFor?: (chainId: number) => ChainReader;
  submitUseCredits?: UseCreditsSubmitter;
  iapMode?: boolean;
  portal?: boolean;
  timing?: { wait?: { pollMs?: number; timeoutMs?: number }; acrossPoll?: { intervalMs?: number; maxAttempts?: number; fetchImpl?: typeof fetch } };
  use?: { href(name: string): string; label: string; onUse?(name: string): void };
};

function escapeName(name: string): string {
  return name.replace(/[^A-Za-z0-9]/g, "");
}

export default function NameBuyModal({ name, offer, onClose, client: givenClient, readerFor, submitUseCredits, iapMode = false, portal, timing, use }: NameBuyModalProps) {
  const client = givenClient ?? shopClient();
  const wallet = useLandWallet();
  const navigate = useNavigate();
  const ethChain = landChainId();
  const polygonChain = creditsChainFor(ethChain);
  const reader = (chainId: number) => (readerFor ? readerFor(chainId) : rpcReader(chainId));

  const credits = useResource(
    wallet.account ? (signal) => client.credits.balance(wallet.account!, { signal }).then((r) => usdCents(Number(r.usd?.balanceCents ?? 0))) : null,
    [wallet.account, client],
  );
  const ethMana = useResource<bigint>(
    wallet.session.kind === "web3" && wallet.account ? (signal) => readContract<bigint>(reader(ethChain), landNetwork(ethChain).mana, erc20Abi, "balanceOf", [wallet.account], signal) : null,
    [wallet.account, wallet.session.kind, ethChain],
  );
  const polygonMana = useResource<bigint>(
    wallet.session.kind === "web3" && wallet.account && POLYGON_MANA[polygonChain]
      ? (signal) => readContract<bigint>(reader(polygonChain), POLYGON_MANA[polygonChain]!, erc20Abi, "balanceOf", [wallet.account], signal)
      : null,
    [wallet.account, wallet.session.kind, polygonChain],
  );

  const view = useMemo(
    () => presentPayment(offer, wallet.session, { creditsBalance: credits.data ?? null, manaBalance: (ethMana.data ?? null) as ManaWei | null }),
    [offer, wallet.session, credits.data, ethMana.data],
  );
  const mixed = useMemo(() => {
    if (polygonMana.data == null) return null;
    const alt = presentPayment(offer, wallet.session, { creditsBalance: credits.data ?? null, manaBalance: polygonMana.data as ManaWei, report: () => {} });
    const creditsOpt = alt.options.find((o) => o.method === "credits");
    const opt = alt.options.find((o) => o.method === "mana");
    return creditsOpt?.state.kind === "topUpRequired" ? (opt?.mixed ?? null) : null;
  }, [offer, wallet.session, credits.data, polygonMana.data]);

  const creditsOpt = view.options.find((o) => o.method === "credits") ?? null;
  const manaOpt = view.options.find((o) => o.method === "mana") ?? null;
  const [rail, setRail] = useState<Rail>(view.primary === "mana" ? "mana" : "credits");
  const [phase, setPhase] = useState<Phase>(view.options.length > 1 || !!mixed ? "choose" : "confirm");
  const [reentry, setReentry] = useState("");
  const [stage, setStage] = useState<NameRegistrationStage | "approving">("preparing");
  const [error, setError] = useState<{ text: string; retryUnsafe: boolean; creditsClosed?: boolean } | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);
  const [closedCredits, setClosedCredits] = useState(false);
  const started = useRef(false);
  const busy = phase === "completing";

  useEffect(() => {
    if (view.options.length > 1 || mixed) setPhase((p) => (p === "confirm" && !reentry ? "choose" : p));
  }, [view.options.length, mixed, reentry]);

  const creditsQuote = offerQuote(offer, "credits");
  const priceCredits = creditsQuote?.method === "credits" ? creditsCeil(creditsQuote.quoted) : null;
  const manaQuote = manaOpt?.quote?.method === "mana" ? manaOpt.quote.amount : null;
  const matches = reentry.trim().toLowerCase() === name.toLowerCase();
  const creditsState = closedCredits ? ({ kind: "unavailable", missing: null } as const) : creditsOpt?.state;
  const shortBy = creditsState?.kind === "topUpRequired" ? creditsCeil(creditsState.short) : 0;
  const wrongNetwork = rail === "mana" && !!wallet.provider && !wallet.onChain(ethChain);

  const chosenReady =
    rail === "credits"
      ? creditsState?.kind === "ready"
      : rail === "mana"
        ? manaOpt?.state.kind === "ready" && !wrongNetwork
        : !!mixed && !!wallet.provider && !!wallet.signer;

  async function buy() {
    if (started.current || !matches || !chosenReady || !wallet.account) return;
    started.current = true;
    setPhase("completing");
    setError(null);
    setStage("preparing");
    try {
      if (rail === "mana") {
        if (!wallet.provider || !wallet.signer) throw new Error("Connect your wallet to register with MANA.");
        const out = await registerNameWithEthereumMana({
          name,
          buyer: wallet.signer,
          provider: wallet.provider,
          sessionKind: wallet.session.kind,
          chainId: ethChain,
          wait: timing?.wait,
          onProgress: setStage,
        });
        setTxHash(out.originTxHash);
        setPhase("success");
      } else {
        const managed = wallet.session.kind !== "web3";
        const signerProvider = wallet.provider ?? wallet.host?.provider?.() ?? null;
        const provider = signerProvider && managed ? metaTxProviderShim(signerProvider, creditsChainFor(landChainId())) : signerProvider;
        const submit =
          submitUseCredits ?? (signerProvider ? (managed ? relayedUseCreditsSubmitter(signerProvider) : walletUseCreditsSubmitter(signerProvider)) : null);
        if (!provider || !submit) {
          throw new Error(managed ? "Your sign-in can't confirm purchases right now. Sign in again to continue." : "This purchase needs a wallet to confirm it. Connect your wallet and try again.");
        }
        const quoted = creditsQuote?.method === "credits" ? creditsQuote.quoted : null;
        if (quoted === null) throw new Error("The Credits price is unavailable right now.");
        const out = await registerNameWithUsdCredits({
          name,
          buyer: wallet.account,
          creditsCents: rail === "mixed" && mixed ? mixed.creditsCents : quoted,
          quotedCents: quoted,
          maxManaWei: rail === "mixed" && mixed ? mixed.manaWei : undefined,
          provider,
          client,
          submit,
          wait: timing?.wait,
          acrossPoll: timing?.acrossPoll,
          onProgress: setStage,
        });
        setTxHash(out.originTxHash);
        setPhase(out.status === "registered" ? "success" : "pending");
        credits.reload();
      }
    } catch (e) {
      if (e instanceof TxPendingError) {
        setTxHash(e.txHash);
        setPhase("pending");
      } else {
        const closed = e instanceof NameCreditsClosedError;
        if (closed) setClosedCredits(true);
        const text =
          e instanceof NameRouteCostTooHighError
            ? "Registering a NAME with Credits is temporarily unavailable due to high network costs. Please try again later."
            : closed
              ? "Registering a NAME with Credits is not available right now. Nothing was charged."
              : e instanceof NameNotRegisteredError
                ? "The NAME could not be registered on Ethereum. The payment was recovered by Decentraland; contact support if it does not show in your history."
                : e instanceof NameSettlementUnknownError
                  ? "We couldn't confirm whether the purchase went through. Check your purchase history before trying again."
                  : e instanceof NameTakenError
                    ? "This NAME has just been registered by someone else. Nothing was charged."
                    : e instanceof InsufficientManaError
                      ? `Your wallet does not hold enough MANA on ${chainLabel(ethChain)} to register this NAME.`
                      : (e as { code?: number } | null)?.code === 4001
                        ? "You declined the request in your wallet. Nothing was charged."
                        : isShopApiError(e) && e.status === 402
                          ? "Your Credits balance does not cover this NAME. Top up to continue."
                          : (e as { message?: string } | null)?.message || "Couldn't register the name. Please try again.";
        setError({ text, retryUnsafe: e instanceof NameNotRegisteredError || e instanceof NameSettlementUnknownError, creditsClosed: closed });
        setPhase("error");
      }
    } finally {
      started.current = false;
    }
  }

  const title =
    phase === "success" ? "Purchase complete!" : phase === "pending" ? "Your NAME is being registered" : phase === "error" ? "Something went wrong" : `Buy ${name}.dcl.eth`;

  const priceRow = (
    <div className="shop-namebuy__row">
      <span className="shop-namebuy__thumb" aria-hidden="true">
        @
      </span>
      <span className="shop-namebuy__meta">
        <strong>
          {name}
          <span>.dcl.eth</span>
        </strong>
        <span>Decentraland NAME</span>
      </span>
      <span>
        {rail === "mana" && manaQuote !== null ? (
          `${formatMana(manaQuote, { group: true })} MANA`
        ) : rail === "mixed" && mixed ? (
          `${mixed.credits.toLocaleString("en-US")} Credits + ${formatMana(mixed.manaWei, { group: true })} MANA`
        ) : priceCredits !== null ? (
          `${priceCredits.toLocaleString("en-US")} Credits`
        ) : (
          "—"
        )}
      </span>
    </div>
  );

  return (
    <Modal onClose={busy ? undefined : onClose} closeOnBackdrop={!busy} closeOnEsc={!busy} showClose={!busy} width={500} portal={portal} ariaLabel={title}>
      <div className="shop-namebuy">
        <h2>{title}</h2>
        {wallet.account && phase !== "success" && phase !== "pending" ? (
          <p className="shop-namebuy__balance">
            My Credits balance: {credits.status === "ready" ? `${creditsFloor(credits.data).toLocaleString("en-US")} Credits` : credits.status === "error" ? "unavailable" : "…"}
          </p>
        ) : null}

        {phase === "choose" ? (
          <>
            {priceRow}
            <fieldset className="shop-namebuy__options">
              <legend>Pay with</legend>
              {creditsOpt ? (
                <label className="shop-namebuy__option">
                  <input type="radio" name="shop-namebuy-rail" checked={rail === "credits"} onChange={() => setRail("credits")} />
                  <span>Credits</span>
                  <span>{priceCredits !== null ? `${priceCredits.toLocaleString("en-US")} Credits` : "—"}</span>
                  {creditsState && creditsState.kind !== "ready" ? (
                    <small data-tone="warn">{closedCredits ? "Registering a NAME with Credits is not available right now." : optionStateText("credits", creditsState)}</small>
                  ) : null}
                </label>
              ) : null}
              {manaOpt ? (
                <label className="shop-namebuy__option">
                  <input type="radio" name="shop-namebuy-rail" checked={rail === "mana"} onChange={() => setRail("mana")} />
                  <span>MANA on {chainLabel(ethChain)}</span>
                  <span>{manaQuote !== null ? `${formatMana(manaQuote, { group: true })} MANA` : "—"}</span>
                  <small>Registered directly with your wallet. Your wallet pays the {chainLabel(ethChain)} network fee.</small>
                  {manaOpt.state.kind !== "ready" ? <small data-tone="warn">{optionStateText("mana", manaOpt.state)}</small> : null}
                </label>
              ) : null}
              {manaOpt && mixed ? (
                <label className="shop-namebuy__option">
                  <input type="radio" name="shop-namebuy-rail" checked={rail === "mixed"} onChange={() => setRail("mixed")} />
                  <span>Credits and MANA</span>
                  <span>
                    {mixed.credits.toLocaleString("en-US")} Credits + {formatMana(mixed.manaWei, { group: true })} MANA
                  </span>
                  <small>Your Credits first, the rest in MANA on {chainLabel(polygonChain)}.</small>
                </label>
              ) : null}
            </fieldset>
            <RailNotes inChoose rail={rail} creditsState={creditsState} creditsClosed={closedCredits} shortBy={shortBy} iapMode={iapMode} name={name} client={client} wallet={wallet} navigate={navigate} wrongNetwork={wrongNetwork} ethChain={ethChain} />
            <div className="shop-land-actions">
              <Button onClick={() => setPhase("confirm")} disabled={!chosenReady}>
                Continue
              </Button>
              <Button variant="secondary" onClick={onClose}>
                Cancel
              </Button>
            </div>
          </>
        ) : null}

        {phase === "confirm" ? (
          <>
            {priceRow}
            {rail === "credits" && creditsState && creditsState.kind !== "ready" ? (
              <RailNotes rail={rail} creditsState={creditsState} creditsClosed={closedCredits} shortBy={shortBy} iapMode={iapMode} name={name} client={client} wallet={wallet} navigate={navigate} wrongNetwork={false} ethChain={ethChain} />
            ) : (
              <form
                className="shop-land-form"
                onSubmit={(e) => {
                  e.preventDefault();
                  void buy();
                }}
              >
                <p>
                  You have chosen <b>@{escapeName(name)}</b>. Re-enter your NAME to confirm your selection. The capitalization of NAMEs cannot be changed once claimed.
                </p>
                <label className="shop-namebuy__reenter">
                  <span aria-hidden="true">@</span>
                  <input
                    value={reentry}
                    onChange={(e) => setReentry(e.target.value.replace(/[^a-zA-Z0-9]/g, ""))}
                    placeholder="yourname"
                    aria-label="Re-enter the NAME"
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <span aria-hidden="true">.dcl.eth</span>
                </label>
                {reentry && !matches ? <small role="alert">Names do not match</small> : null}
                {rail === "mana" && wrongNetwork ? (
                  <RailNotes rail={rail} creditsState={creditsState} creditsClosed={closedCredits} shortBy={0} iapMode={iapMode} name={name} client={client} wallet={wallet} navigate={navigate} wrongNetwork ethChain={ethChain} />
                ) : null}
                <div className="shop-land-actions">
                  <Button type="submit" disabled={!matches || !chosenReady}>
                    Buy
                  </Button>
                  {view.options.length > 1 || mixed ? (
                    <Button variant="secondary" onClick={() => setPhase("choose")}>
                      Back
                    </Button>
                  ) : (
                    <Button variant="secondary" onClick={onClose}>
                      Cancel
                    </Button>
                  )}
                </div>
              </form>
            )}
          </>
        ) : null}

        {phase === "completing" ? (
          <div className="shop-namebuy__progress" role="status" aria-live="polite">
            <p>
              {stage === "awaiting-confirmation"
                ? "Confirm in your wallet to continue."
                : stage === "approving"
                  ? "Approving MANA…"
                  : stage === "registering"
                    ? "Registering your NAME…"
                    : "Completing purchase…"}
            </p>
            <progress aria-label="Purchase progress" />
            {stage === "registering" ? <p>Registering a NAME crosses to Ethereum and can take a few minutes. You can close this window; the NAME will appear in My items.</p> : null}
          </div>
        ) : null}

        {phase === "pending" ? (
          <>
            <div className="shop-land-note" role="status">
              <p>
                <b>Your payment went through.</b> The NAME is still being registered on Ethereum and will appear in My items when it lands.
              </p>
            </div>
            {priceRow}
            {txHash ? (
              <a href={txHref(rail === "mana" ? ethChain : polygonChain, txHash)} target="_blank" rel="noreferrer">
                View transaction
              </a>
            ) : null}
            <div className="shop-land-actions">
              <Button onClick={() => navigate("/shop/my-items")}>My items</Button>
            </div>
          </>
        ) : null}

        {phase === "success" ? (
          <>
            <div className="shop-land-note" role="status">
              <p>
                <b>{name}.dcl.eth is yours.</b> You can find your NAME in My items.
              </p>
            </div>
            {priceRow}
            {txHash ? (
              <a href={txHref(rail === "mana" ? ethChain : polygonChain, txHash)} target="_blank" rel="noreferrer">
                View transaction
              </a>
            ) : null}
            <div className="shop-land-actions">
              <Button variant="secondary" onClick={() => navigate("/shop/my-items")}>
                My items
              </Button>
              <Button as="a" href={use ? use.href(name) : "/create/names"} onClick={() => use?.onUse?.(name)}>
                {use ? use.label : "Use it for your World or avatar"}
              </Button>
            </div>
          </>
        ) : null}

        {phase === "error" && error ? (
          <>
            {priceRow}
            <div className="shop-land-note shop-land-note--error" role="alert">
              <p>{error.text}</p>
            </div>
            <div className="shop-land-actions">
              {error.retryUnsafe ? (
                <Button onClick={onClose}>Close</Button>
              ) : (
                <Button onClick={() => setPhase(view.options.length > 1 || mixed ? "choose" : "confirm")}>Try again</Button>
              )}
            </div>
          </>
        ) : null}
      </div>
    </Modal>
  );
}

type NotesProps = {
  inChoose?: boolean;
  rail: Rail;
  creditsClosed: boolean;
  creditsState: ReturnType<typeof presentPayment>["options"][number]["state"] | { kind: "unavailable"; missing: null } | undefined;
  shortBy: number;
  iapMode: boolean;
  name: string;
  client: ShopClient;
  wallet: ReturnType<typeof useLandWallet>;
  navigate: ReturnType<typeof useNavigate>;
  wrongNetwork: boolean;
  ethChain: number;
};

function RailNotes({ inChoose = false, rail, creditsState, creditsClosed, shortBy, iapMode, name, client, wallet, navigate, wrongNetwork, ethChain }: NotesProps) {
  if (rail === "mana" && wrongNetwork) {
    return (
      <div className="shop-land-note shop-land-note--warn" role="status">
        <p>
          Your wallet is on {wallet.chainId ? chainLabel(wallet.chainId) : "another network"}. Registering with MANA runs on {chainLabel(ethChain)}.
        </p>
        <Button size="sm" variant="secondary" disabled={wallet.pendingChainId !== null} onClick={() => void wallet.switchTo(ethChain)}>
          {wallet.pendingChainId !== null ? "Confirm in your wallet…" : `Switch to ${chainLabel(ethChain)}`}
        </Button>
      </div>
    );
  }
  if (rail !== "credits" || !creditsState || creditsState.kind === "ready") return null;
  if (creditsState.kind === "signInRequired") {
    return (
      <div className="shop-land-note" role="status">
        <p>Sign in to pay with Credits.</p>
        {wallet.host?.onSignIn ? (
          <Button size="sm" onClick={() => wallet.host?.onSignIn?.()}>
            Sign in
          </Button>
        ) : null}
      </div>
    );
  }
  if (creditsState.kind === "topUpRequired") return <TopUp shortBy={shortBy} iapMode={iapMode} name={name} client={client} navigate={navigate} account={wallet.account} />;
  if (creditsState.kind === "unavailable") {
    if (inChoose && (!creditsClosed || wallet.session.kind !== "web3")) return null;
    if (inChoose) {
      return (
        <div className="shop-land-note" role="status">
          <p>You can still register it with MANA from your wallet.</p>
        </div>
      );
    }
    return (
      <div className="shop-land-note shop-land-note--warn" role="status">
        <p>
          {creditsClosed
            ? `Registering a NAME with Credits is not available right now.${wallet.session.kind === "web3" ? " You can still register it with MANA from your wallet." : ""}`
            : outageText("credits", creditsState.missing)}
        </p>
      </div>
    );
  }
  return null;
}

function TopUp({ shortBy, iapMode, name, client, navigate, account }: { shortBy: number; iapMode: boolean; name: string; client: ShopClient; navigate: ReturnType<typeof useNavigate>; account: string | null }) {
  const packs = useResource<UnityCreditPack[]>(iapMode ? null : (signal) => client.packs.list({ signal }).then((r) => [...(r.packs ?? [])].sort((a, b) => a.order - b.order)), [iapMode, client]);
  const closing = (packs.data ?? []).filter((p) => p.credits >= shortBy);
  const recommended = closing[0] ?? null;
  const [picked, setPicked] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const active = (packs.data ?? []).find((p) => p.id === picked) ?? recommended;
  useEffect(() => {
    const onShow = (e: PageTransitionEvent) => e.persisted && setBusy(false);
    window.addEventListener("pageshow", onShow);
    return () => window.removeEventListener("pageshow", onShow);
  }, []);
  const buyCredits = async () => {
    if (!active || busy || !account) return;
    setBusy(true);
    setError(null);
    stashNameResume(name);
    try {
      const order = await client.packs.order({ packId: active.id, source: "website" });
      if (order.url) {
        window.location.assign(order.url);
        return;
      }
      if (!order.orderId) throw new Error("The checkout returned neither a redirect nor an order.");
      navigate(`/shop/credits?order=${encodeURIComponent(order.orderId)}`);
    } catch {
      clearNameResume();
      setError("We couldn't start the Credits checkout. Nothing was charged; please try again.");
      setBusy(false);
    }
  };
  return (
    <div className="shop-land-note shop-land-note--warn" role="status">
      <p>
        <b>Insufficient Credits.</b> You need <b>{shortBy.toLocaleString("en-US")} more Credits</b> to purchase this NAME.
      </p>
      {iapMode ? null : packs.status === "ready" && packs.data.length ? (
        <>
          <fieldset className="shop-namebuy__packs">
            <legend>Buy Credits</legend>
            {packs.data.map((p) => (
              <label key={p.id}>
                <input type="radio" name="shop-namebuy-pack" checked={active?.id === p.id} onChange={() => setPicked(p.id)} />
                <span>{p.credits.toLocaleString("en-US")} Credits</span>
                <span>${p.usd.toLocaleString("en-US")}</span>
                {p.id === recommended?.id ? <em>Covers this NAME</em> : null}
              </label>
            ))}
          </fieldset>
          <Button size="sm" onClick={() => void buyCredits()} disabled={!active || busy || !account}>
            {busy ? "Opening checkout…" : "Buy Credits"}
          </Button>
        </>
      ) : packs.status === "error" ? (
        <a href="/shop/credits">Get Credits</a>
      ) : null}
      {error ? <p role="alert">{error}</p> : null}
    </div>
  );
}
