// from: shop@2d3bf62:app/src/pages/NamesPage.tsx (+ NamesPage.styles.ts -> namespage.css, hooks/useTypedPlaceholder.ts,
// assets/names/*-icon.svg -> ./icons, en.json names.*), app/src/hooks/useNamesEnabled.ts (replaced: the server's
// credit route is the gate), marketplace@f3956826:webapp/src/components/NamesPage/ClaimNamePage/ClaimNamePage.tsx
// (the banned-names check, the taken-NAME link to the resale browse),
// catalyrst-v2 ui3/src/marketplace/pages/MkNamesPage.tsx and MkClaimNamePage.tsx (superseded)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace sources). Licensed under the Apache License, Version 2.0.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import Button from "../../atoms/Button";
import { useMediaQuery } from "../../data/hooks/useMediaQuery";
import { shopClient } from "../api/shop";
import Icon from "../discovery/components/Icon";
import { SHOP_BASE } from "../routes";
import type { ShopClient } from "../api/shop";
import { creditsCeil } from "../model/money";
import type { PurchaseOffer } from "../model/purchase";
import { presentPayment } from "../payment/present";
import { useLandWallet } from "../land/useLandWallet";
import { fetchBannedNames } from "./api";
import NameBuyModal, { type NameBuyModalProps } from "./NameBuyModal";
import { NAME_INVALID_TEXT, NAME_MAX_LENGTH, NAME_MIN_LENGTH, sanitizeNameInput, validateName } from "./names";
import { useOfferSource } from "../detail/offers";
import { interimNameOffer, NameAlreadyRegisteredError, nameLabel, quoteNameCredits, seamNameOffer, type NameOfferSource } from "./offer";
import { takeNameResume } from "./resume";
import "./names.css";
import "./namespage.css";
import identityIcon from "./icons/identity-icon.svg?url";
import worldIcon from "./icons/world-icon.svg?url";
import inviteIcon from "./icons/invite-icon.svg?url";

export type NamesNavState = { resumeName?: string };

type Status =
  | { kind: "idle" }
  | { kind: "invalid"; text: string }
  | { kind: "checking" }
  | { kind: "available"; offer: PurchaseOffer }
  | { kind: "taken" }
  | { kind: "banned" }
  | { kind: "error" };

export type NamesPageProps = {
  client?: ShopClient;
  offerSource?: NameOfferSource;
  loadBanned?: (signal: AbortSignal) => Promise<string[]>;
  debounceMs?: number;
  modal?: Omit<NameBuyModalProps, "name" | "offer" | "onClose">;
  resaleHref?: (name: string) => string;
};

const WORLDS_DOCS_URL = "https://docs.decentraland.org/creator/worlds/about/";

const EXAMPLES = ["nayeli", "kenji", "rowan", "mira"];
const TYPE_MS = 60;
const HOLD_MS = 700;
const ERASE_MS = 25;

function useTypedPlaceholder(words: readonly string[], enabled: boolean): string {
  const frames = useMemo(
    () =>
      words.flatMap((w) => {
        const f: { text: string; delay: number }[] = [];
        for (let i = 1; i <= w.length; i++) f.push({ text: w.slice(0, i), delay: TYPE_MS });
        if (f.length) f[f.length - 1] = { text: w, delay: HOLD_MS };
        for (let i = w.length - 1; i >= 1; i--) f.push({ text: w.slice(0, i), delay: ERASE_MS });
        return f;
      }),
    [words],
  );
  const [i, setI] = useState(0);
  useEffect(() => {
    if (!enabled || frames.length === 0) return;
    const id = setTimeout(() => setI((n) => (n + 1) % frames.length), frames[i % frames.length]!.delay);
    return () => clearTimeout(id);
  }, [i, enabled, frames]);
  return enabled && frames.length ? frames[i % frames.length]!.text : "";
}

export default function NamesPage({ client, offerSource, loadBanned, debounceMs = 400, modal, resaleHref = (n) => `/shop/names?section=resale&search=${encodeURIComponent(n)}` }: NamesPageProps) {
  const wallet = useLandWallet();
  const location = useLocation();
  const navigate = useNavigate();
  const navResume = (location.state as NamesNavState | null)?.resumeName;
  const [value, setValue] = useState("");
  const [status, setStatus] = useState<Status>({ kind: "idle" });
  const [modalOpen, setModalOpen] = useState(false);
  const [pendingResume, setPendingResume] = useState("");
  const [banned, setBanned] = useState<Set<string> | null>(null);
  const [retry, setRetry] = useState(0);
  const seam = useOfferSource();
  const source = useMemo(() => offerSource ?? seamNameOffer(seam) ?? interimNameOffer({ client }), [offerSource, seam, client]);
  const sizer = useRef<HTMLSpanElement | null>(null);
  const [width, setWidth] = useState<number | undefined>(undefined);

  const [touched, setTouched] = useState(false);
  const reducedMotion = useMediaQuery("(prefers-reduced-motion: reduce)", true);
  const typed = useTypedPlaceholder(EXAMPLES, !touched && !value && !reducedMotion);
  const placeholder = typed || "yourname";
  const [basePrice, setBasePrice] = useState<number | null>(null);
  useEffect(() => {
    const ctrl = new AbortController();
    quoteNameCredits(client ?? shopClient(), ctrl.signal).then(
      (q) => {
        if (!ctrl.signal.aborted && "quoted" in q) setBasePrice(creditsCeil(q.quoted));
      },
      () => {},
    );
    return () => ctrl.abort();
  }, [client]);

  useLayoutEffect(() => {
    if (sizer.current) setWidth(sizer.current.getBoundingClientRect().width || undefined);
  }, [value, placeholder]);

  useEffect(() => {
    const resume = typeof navResume === "string" && nameLabel(navResume) ? navResume : takeNameResume();
    if (navResume) navigate(`${location.pathname}${location.search}`, { replace: true, state: null });
    if (!resume) return;
    setValue(resume);
    setPendingResume(resume);
    setTouched(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const ctrl = new AbortController();
    (loadBanned ? loadBanned(ctrl.signal) : fetchBannedNames({ signal: ctrl.signal })).then(
      (list) => setBanned(new Set(list)),
      () => setBanned(new Set()),
    );
    return () => ctrl.abort();
  }, [loadBanned]);

  useEffect(() => {
    const v = validateName(value);
    if (!v.ok) {
      setStatus(v.reason === "empty" ? { kind: "idle" } : { kind: "invalid", text: NAME_INVALID_TEXT[v.reason] });
      return;
    }
    if (banned?.has(value.toLowerCase())) {
      setStatus({ kind: "banned" });
      return;
    }
    const label = nameLabel(value)!;
    setStatus({ kind: "checking" });
    const ctrl = new AbortController();
    const id = setTimeout(() => {
      source(label, wallet.session, ctrl.signal).then(
        (offer) => {
          if (ctrl.signal.aborted) return;
          setStatus(presentPayment(offer, wallet.session).manaOnly ? { kind: "taken" } : { kind: "available", offer });
        },
        (e: unknown) => {
          if (ctrl.signal.aborted || (e as { name?: string } | null)?.name === "AbortError") return;
          setStatus(e instanceof NameAlreadyRegisteredError ? { kind: "taken" } : { kind: "error" });
        },
      );
    }, debounceMs);
    return () => {
      clearTimeout(id);
      ctrl.abort();
    };
  }, [value, banned, source, wallet.session, debounceMs, retry]);

  const view = status.kind === "available" ? presentPayment(status.offer, wallet.session, { report: () => {} }) : null;
  const creditsOption = view && !view.manaOnly ? view.options.find((o) => o.method === "credits") : undefined;
  const priceCredits = creditsOption?.quote?.method === "credits" ? creditsCeil(creditsOption.quote.quoted) : null;
  const bothMethods = !!creditsOption && !!view?.options.some((o) => o.method === "mana");
  const canClaim = status.kind === "available" && !!view && !view.manaOnly && view.options.length > 0;

  useEffect(() => {
    if (!pendingResume) return;
    if (value.toLowerCase() !== pendingResume.toLowerCase()) {
      setPendingResume("");
      return;
    }
    if (status.kind === "idle" || status.kind === "checking") return;
    if (!canClaim) {
      setPendingResume("");
      return;
    }
    if (wallet.session.kind === "signedOut") return;
    setPendingResume("");
    setModalOpen(true);
  }, [pendingResume, value, status, canClaim, wallet.session.kind]);

  const claim = () => {
    if (!canClaim) return;
    if (wallet.session.kind === "signedOut") {
      wallet.host?.onSignIn?.();
      return;
    }
    setModalOpen(true);
  };

  const takenish = status.kind === "taken" || status.kind === "banned";
  const shownPrice = takenish ? null : (priceCredits ?? basePrice);
  const claimLabel = priceCredits !== null ? `Claim name for ${priceCredits} Credits` : "Claim name";
  const shortIdle = status.kind === "idle" && value.length > 0 && value.length < NAME_MIN_LENGTH;

  return (
    <div className="shop-names-claim" data-testid="names-page">
      <nav className="shop-names-claim__crumbs" aria-label="Breadcrumb">
        <Link to={`${SHOP_BASE}/items`}>Collectibles</Link>
        <span aria-hidden="true">{">"}</span>
        <span aria-current="page">NAMEs</span>
      </nav>

      <div className="shop-names-claim__panel">
        <section className="shop-names-claim__hero" aria-labelledby="shop-names-title">
          <div className="shop-names-claim__copy">
            <h1 id="shop-names-title">Get your unique NAME!</h1>
            <p>When you claim a NAME, you are not just choosing an alias. You are making your presence official.</p>
          </div>
          <form
            className="shop-names-claim__search"
            role="search"
            onSubmit={(e) => {
              e.preventDefault();
              claim();
            }}
          >
            <div className="shop-names-claim__wrap">
              <div className="shop-names-claim__row" data-invalid={takenish || undefined}>
                <div className="shop-names-claim__box">
                  <label className="shop-names-claim__field">
                    <span className="shop-names-claim__at" aria-hidden="true">
                      @
                    </span>
                    <input
                      value={value}
                      onChange={(e) => setValue(sanitizeNameInput(e.target.value))}
                      onFocus={() => setTouched(true)}
                      placeholder={placeholder}
                      aria-label="Search for a NAME"
                      autoComplete="off"
                      spellCheck={false}
                      maxLength={NAME_MAX_LENGTH}
                      style={{ width: width ? `${Math.ceil(width) + 1}px` : undefined }}
                    />
                    <span ref={sizer} className="shop-names-claim__sizer" aria-hidden="true" data-placeholder={!value || undefined}>
                      {value || placeholder}
                    </span>
                    <span className="shop-names-claim__suffix" aria-hidden="true">
                      .dcl.eth
                    </span>
                  </label>
                  {value ? (
                    <span className="shop-names-claim__counter" aria-hidden="true">
                      {value.length}/{NAME_MAX_LENGTH}
                    </span>
                  ) : null}
                </div>
                <button type="submit" className="shop-names-claim__btn" disabled={!canClaim} aria-label={claimLabel}>
                  Claim name
                  {shownPrice !== null ? (
                    <span className="shop-names-claim__price">
                      <Icon name="credits" />
                      {shownPrice.toLocaleString("en-US")}
                    </span>
                  ) : null}
                </button>
              </div>
              {takenish ? (
                <div className="shop-names-claim__taken" role="status">
                  <Icon name="info" size={16} />
                  {status.kind === "banned" ? (
                    <span>This NAME is not available.</span>
                  ) : (
                    <>
                      <span>This NAME is taken. Choose another NAME or make an offer to the owner.</span>
                      <Link className="shop-names-claim__action" to={resaleHref(value)}>
                        Make offer
                      </Link>
                    </>
                  )}
                </div>
              ) : status.kind === "checking" || status.kind === "invalid" || status.kind === "error" || shortIdle ? (
                <div className="shop-names-claim__status" role="status">
                  {status.kind === "checking" ? "Checking availability…" : null}
                  {status.kind === "invalid" ? status.text : null}
                  {shortIdle ? `Names must be at least ${NAME_MIN_LENGTH} characters.` : null}
                  {status.kind === "error" ? (
                    <>
                      <span>We couldn’t check that NAME right now.</span>
                      <Button size="sm" variant="ghost" onClick={() => setRetry((r) => r + 1)}>
                        Try again
                      </Button>
                    </>
                  ) : null}
                </div>
              ) : null}
            </div>
            <span className="shop-names-claim__sr" role="status" aria-live="polite">
              {status.kind === "available" ? `${value} is available!` : ""}
            </span>
            <span className="shop-names-claim__sr">
              {status.kind === "available" && bothMethods ? "Pay with Credits, or with MANA from your wallet." : ""}
            </span>
          </form>
        </section>

        <section className="shop-names-claim__why" aria-labelledby="shop-names-why">
          <div className="shop-names-claim__whyhead">
            <h2 id="shop-names-why">Why buy a NAME?</h2>
            <p>
              Your NAME is your unique identity in Decentraland. It appears above your avatar, in chat, and in photos, helping people recognize and remember you wherever you go.
            </p>
          </div>
          <div className="shop-names-claim__cards">
            <article>
              <img src={identityIcon} alt="" />
              <div>
                <h3>Own your identity</h3>
                <p>Stand out with a unique NAME that is unmistakably yours.</p>
              </div>
            </article>
            <article>
              <img src={worldIcon} alt="" />
              <div>
                <h3>Unlock your own World</h3>
                <p>
                  Create and customize a personal 3D space with an address like <b>yourname.dcl.eth.</b>
                </p>
                <a href={WORLDS_DOCS_URL} target="_blank" rel="noopener noreferrer">
                  Learn More
                  <Icon name="external-link" size={13} />
                </a>
              </div>
            </article>
            <article>
              <img src={inviteIcon} alt="" />
              <div>
                <h3>Invite friends and earn</h3>
                <p>Share your referral link and earn rewards when your friends join.</p>
              </div>
            </article>
          </div>
        </section>
      </div>

      {modalOpen && status.kind === "available" ? <NameBuyModal {...modal} client={modal?.client ?? client} name={value} offer={status.offer} onClose={() => setModalOpen(false)} /> : null}
    </div>
  );
}
