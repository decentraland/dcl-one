// from: auth@e081cc6:src/components/Pages/LoginPage/LoginPage.tsx (the page: rotating backdrop, the email box, the
//       wallet options and their modals), src/components/Connection/{Connection,ConnectionPrimaryButton,
//       ConnectionOption/ConnectionOption,EmailInput/EmailInput}.tsx, src/shared/email.ts (isEmailValid)
// decentraland/auth is Apache-2.0.
// decentraland.org's sign-in, on this site: Google, Apple, Discord and X through Magic, email codes through thirdweb,
// browser and phone wallets directly; every session lasts a month, as upstream's. Upstream's Fortmatic option goes
// through decentraland-connect, which this site does not carry, so it is not wired yet.

import logo from "@ui/atoms/assets/decentraland-logo.svg?url";
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type FormEvent } from "react";

import { useAuth } from "@/data/lib/auth/context";
import { isIdentityExpired } from "@/data/lib/auth/expiry";
import { getIdentity, setIdentity } from "@/data/lib/auth/session";

import appleIcon from "./assets/apple.svg?url";
import coinbaseIcon from "./assets/coinbase.svg?url";
import discordIcon from "./assets/discord.svg?url";
import fortmaticIcon from "./assets/fortmatic.png?url";
import googleIcon from "./assets/google.svg?url";
import background1 from "./assets/image-new1.webp?url";
import background2 from "./assets/image-new2.webp?url";
import background3 from "./assets/image-new3.webp?url";
import background4 from "./assets/image-new4.webp?url";
import background5 from "./assets/image-new5.webp?url";
import background6 from "./assets/image-new6.webp?url";
import metamaskIcon from "./assets/metamask.svg?url";
import walletConnectIcon from "./assets/wallet-connect.png?url";
import xIcon from "./assets/x.svg?url";
import { t } from "./copy";
import { failureState, hasMetaMask, signInWithBrowserWallet, walletFor, type ConnectionState } from "./flow";
import { ConnectionModal, EmailLoginModal, type PairingCode } from "./LoginModals";
import { rememberEmail, SESSION_MS, startMagicSignIn } from "./magic";
import styles from "./LoginPage.module.css";
import {
  layoutOptions,
  OPTION_TITLES,
  optionSetFor,
  signInOptionsMode,
  SOCIAL_OPTIONS,
  type ConnectionOption,
} from "./options";
import { safeRedirectTarget } from "./redirect";

const BACKGROUNDS = [background1, background2, background3, background4, background5, background6];
const ROTATE_MS = 5_000;

const ICONS: Record<ConnectionOption, string | null> = {
  metamask: metamaskIcon,
  fortmatic: fortmaticIcon,
  coinbase: coinbaseIcon,
  "wallet-connect": walletConnectIcon,
  google: googleIcon,
  apple: appleIcon,
  discord: discordIcon,
  x: xIcon,
  email: null,
};

const EMAIL_PATTERN =
  /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+$/;

export function isEmailValid(email: string): boolean {
  return EMAIL_PATTERN.test(email);
}

const FADE_MS = 1_000;

/**
 * Two stacked layers cross-fade every five seconds. The hidden one already holds the next picture, so it has loaded by
 * its turn; the layer that faded out takes the picture after that once its fade is over.
 */
function Backdrop() {
  const [layers, setLayers] = useState<{ front: 0 | 1; images: [number, number] }>({ front: 0, images: [0, 1] });
  useEffect(() => {
    let refill: ReturnType<typeof setTimeout> | undefined;
    const timer = setInterval(() => {
      setLayers(({ front, images }) => ({ front: front === 0 ? 1 : 0, images }));
      refill = setTimeout(() => {
        setLayers(({ front, images }) => {
          const behind: 0 | 1 = front === 0 ? 1 : 0;
          const next: [number, number] = [...images];
          next[behind] = (images[front] + 1) % BACKGROUNDS.length;
          return { front, images: next };
        });
      }, FADE_MS);
    }, ROTATE_MS);
    return () => {
      clearInterval(timer);
      clearTimeout(refill);
    };
  }, []);
  return (
    <div className={styles.backgrounds} aria-hidden="true">
      {layers.images.map((image, layer) => (
        <div
          key={layer}
          className={styles.background}
          data-visible={layer === layers.front}
          style={{ backgroundImage: `url("${BACKGROUNDS[image]}")` } as CSSProperties}
        />
      ))}
    </div>
  );
}

function Tip({ text, large, arrow, children }: { text: string; large?: boolean; arrow?: boolean; children: React.ReactNode }) {
  return (
    <span className={styles.tip}>
      {children}
      <span className={styles.tipText} role="tooltip" data-size={large ? "large" : undefined} data-arrow={arrow}>
        {text}
      </span>
    </span>
  );
}

type OptionProps = { option: ConnectionOption; busy: boolean; metamask: boolean; onConnect: (o: ConnectionOption) => void };

function PrimaryOption({ option, busy, metamask, onConnect }: OptionProps) {
  const missingMetaMask = option === "metamask" && !metamask;
  const button = (
    <button
      type="button"
      className={`${styles.button} ${styles.primary}`}
      disabled={busy || missingMetaMask}
      onClick={() => onConnect(option)}
      data-testid={`primary-${option}-button`}
    >
      <span className={styles.primaryIcon} aria-hidden="true">
        <svg viewBox="0 0 32 32" width="35" height="35">
          <image href={ICONS[option] ?? ""} width="32" height="32" />
        </svg>
      </span>
      <span>{t("connection.continue_with", { provider: OPTION_TITLES[option] })}</span>
    </button>
  );
  return missingMetaMask ? (
    <Tip text={t("connection.metamask_not_installed")} large arrow>
      {button}
    </Tip>
  ) : (
    button
  );
}

function GridOption({ option, busy, metamask, onConnect }: OptionProps) {
  const missingMetaMask = option === "metamask" && !metamask;
  return (
    <Tip text={missingMetaMask ? t("connection.metamask_not_installed") : OPTION_TITLES[option]}>
      <button
        type="button"
        className={`${styles.button} ${styles.option}`}
        disabled={busy || missingMetaMask}
        aria-label={OPTION_TITLES[option]}
        onClick={() => onConnect(option)}
        data-testid={`more-${option}-button`}
      >
        <span className={styles.optionIcon} style={{ backgroundImage: `url("${ICONS[option] ?? ""}")` }} aria-hidden="true" />
      </button>
    </Tip>
  );
}

function EmailInput({ busy, error, onSubmit, onChange }: { busy: boolean; error: string | null; onSubmit: (email: string) => void; onChange: () => void }) {
  const [email, setEmail] = useState("");
  const valid = isEmailValid(email);
  const showInvalid = email.length > 0 && !valid;
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (valid) onSubmit(email);
  };
  return (
    <form className={styles.email} onSubmit={submit} noValidate>
      <label className={styles.emailLabel} htmlFor="dcl-email-input">
        {t("email_input.recommended")}
      </label>
      <div className={styles.emailRow}>
        <input
          id="dcl-email-input"
          data-testid="email-input"
          className={styles.emailInput}
          type="email"
          placeholder={t("email_input.placeholder")}
          value={email}
          onChange={(e) => {
            setEmail(e.target.value);
            onChange();
          }}
          aria-invalid={showInvalid || !!error}
          disabled={busy}
          autoComplete="email"
        />
        <button type="submit" className={styles.next} data-busy={busy} disabled={busy || !valid} data-testid="email-submit-button">
          <span>{t("email_input.next")}</span>
          {busy ? <span className={styles.nextSpinner} aria-hidden="true" /> : null}
        </button>
      </div>
      {showInvalid ? <p className={styles.emailError}>{t("email_input.validation_error")}</p> : null}
      {error && !showInvalid ? (
        <p className={styles.emailError} role="alert">
          {error}
        </p>
      ) : null}
    </form>
  );
}

type Modal = { state: ConnectionState; detail: string | null; pairing: PairingCode | null };

export type LoginPageProps = {
  /** The query's `redirectTo`, as given; it is checked before any use. */
  redirectTo: string | null;
  /** The sign-in layout rollout's variant (`dapps-sign-in-primary-option`), null when off or unread. */
  layoutVariant: string | null;
  /** Magic's publishable key for the social options; null leaves them unavailable. */
  magicApiKey: string | null;
};

export default function LoginPage({ redirectTo, layoutVariant, magicApiKey }: LoginPageProps) {
  const auth = useAuth();
  const [client, setClient] = useState<{ userAgent: string; touch: number; injected: boolean; metamask: boolean } | null>(null);
  const [modal, setModal] = useState<Modal | null>(null);
  const [pending, setPending] = useState<ConnectionOption | null>(null);
  const [showMore, setShowMore] = useState(false);
  const [emailBusy, setEmailBusy] = useState(false);
  const [emailError, setEmailError] = useState<string | null>(null);
  const [otpEmail, setOtpEmail] = useState<string | null>(null);
  const walletConnect = useRef<AbortController | null>(null);
  // Each sign-in attempt's number; closing the modal or starting another makes an older attempt's answers stale.
  const attempt = useRef(0);

  // The phone's own option set and the browser's wallet are the browser's to report: they settle after hydration, so
  // the server's markup and the first client render agree.
  useEffect(() => {
    setClient({
      userAgent: navigator.userAgent,
      touch: navigator.maxTouchPoints ?? 0,
      injected: !!window.ethereum,
      metamask: hasMetaMask(),
    });
  }, []);
  useEffect(() => () => walletConnect.current?.abort(), []);
  // A reader who is already signed in goes straight on, as upstream sends them.
  // The store is read directly: during hydration `useAuth` still reports the server's empty session.
  useEffect(() => {
    const identity = getIdentity();
    if (identity && !isIdentityExpired(identity)) {
      window.location.replace(safeRedirectTarget(redirectTo, new URL(window.location.href)));
    }
    // Only the session the page opens with counts: a sign-in made here navigates on its own.
  }, []);

  const layout = useMemo(
    () =>
      layoutOptions(
        optionSetFor(client?.userAgent ?? "", client?.touch ?? 0),
        signInOptionsMode(layoutVariant),
        client?.injected ?? false,
      ),
    [client, layoutVariant],
  );

  const target = useCallback(() => safeRedirectTarget(redirectTo, new URL(window.location.href)), [redirectTo]);
  const metamask = client?.metamask ?? false;
  const finish = useCallback(() => {
    setModal({ state: "redirecting", detail: null, pairing: null });
    window.location.assign(target());
  }, [target]);

  const connect = useCallback(
    async (option: ConnectionOption) => {
      if (option === "email") {
        const input = document.getElementById("dcl-email-input");
        input?.scrollIntoView({ behavior: "smooth", block: "center" });
        input?.focus();
        return;
      }
      const mine = ++attempt.current;
      const live = () => attempt.current === mine;
      setPending(option);
      walletConnect.current?.abort();

      if (SOCIAL_OPTIONS.includes(option)) {
        if (!magicApiKey) {
          setModal({ state: "error-generic", detail: "Social sign-in is not configured on this site.", pairing: null });
          return;
        }
        setModal({ state: "redirecting", detail: null, pairing: null });
        try {
          await startMagicSignIn(magicApiKey, option, target());
        } catch (err) {
          if (live()) setModal({ state: "error-generic", detail: err instanceof Error ? err.message : null, pairing: null });
        }
        return;
      }

      if (option === "fortmatic") {
        setModal({ state: "error-connection-failed", detail: "Fortmatic sign-in is not available on this site yet.", pairing: null });
        return;
      }

      const browserWallet = option === "metamask" ? walletFor("metamask") ?? "injected" : option === "coinbase" ? walletFor("coinbase") : null;
      if (browserWallet) {
        let askedToSign = false;
        try {
          const identity = await signInWithBrowserWallet(browserWallet, SESSION_MS, (phase) => {
            askedToSign = phase === "signing";
            if (live()) setModal({ state: phase, detail: null, pairing: null });
          });
          if (!live()) return;
          setIdentity(identity);
          rememberEmail(null);
          finish();
        } catch (err) {
          if (live()) setModal({ ...failureState(err, askedToSign), pairing: null });
        }
        return;
      }

      // WalletConnect, and Coinbase without its extension: a phone wallet pairs through the code.
      const request = new AbortController();
      walletConnect.current = request;
      setModal({ state: "connecting", detail: null, pairing: null });
      try {
        const session = await auth.connectWalletConnect({ signal: request.signal, expirationMs: SESSION_MS });
        if (request.signal.aborted) {
          session.cancel();
          return;
        }
        request.signal.addEventListener("abort", session.cancel, { once: true });
        setModal({ state: "connecting", detail: null, pairing: { qrDataUrl: session.qrDataUrl, uri: session.uri } });
        const identity = await session.connected;
        if (request.signal.aborted) return;
        if (identity) {
          rememberEmail(null);
          finish();
        } else setModal({ state: "error-connection-failed", detail: null, pairing: null });
      } catch (err) {
        if (!request.signal.aborted) setModal({ ...failureState(err, false), pairing: null });
      }
    },
    [auth, finish, magicApiKey, target],
  );

  const closeModal = useCallback(() => {
    attempt.current++;
    walletConnect.current?.abort();
    walletConnect.current = null;
    setModal(null);
    setPending(null);
  }, []);

  const sendCode = useCallback(
    async (email: string) => {
      setEmailBusy(true);
      setEmailError(null);
      try {
        await auth.startEmailSignIn(email);
        setOtpEmail(email);
      } catch (err) {
        const message = err instanceof Error ? err.message : "";
        if (message === "Failed to fetch" || message.toLowerCase().includes("network")) setEmailError(t("login.errors.network_error"));
        else if (message.toLowerCase().includes("invalid email")) setEmailError(t("login.errors.invalid_email"));
        else setEmailError(message || t("login.errors.failed_send_code"));
      } finally {
        setEmailBusy(false);
      }
    },
    [auth],
  );

  const verifyCode = useCallback(
    async (code: string) => {
      if (!otpEmail) return;
      // The failure itself reaches the modal, which words a network drop or a refused code apart.
      const identity = await auth.verifyEmailSignIn(otpEmail, code, { expirationMs: SESSION_MS, rethrow: true });
      if (!identity) throw new Error(t("email_login_modal.invalid_or_expired"));
      rememberEmail(otpEmail);
      setOtpEmail(null);
      finish();
    },
    [auth, finish, otpEmail],
  );

  const busy = pending !== null && modal !== null;

  return (
    <main className={styles.main} data-testid="login-page">
      <Backdrop />
      <div className={styles.left}>
        <div className={styles.leftInfo}>
          <div className={styles.mainContainer}>
            <div className={styles.connection}>
              <div className={styles.logoRow}>
                <img className={styles.logo} src={logo} alt="Decentraland" width={48} height={48} />
              </div>
              <div className={styles.content}>
                <h1 className={styles.title}>{t("connection.title")}</h1>
                <EmailInput busy={emailBusy} error={emailError} onSubmit={(email) => void sendCode(email)} onChange={() => setEmailError(null)} />
                {layout.first || layout.second ? <div className={styles.divider}>{t("connection.or_continue_with")}</div> : null}
                <div className={styles.primaryList}>
                  {layout.first ? <PrimaryOption option={layout.first} busy={busy} metamask={metamask} onConnect={(o) => void connect(o)} /> : null}
                  {layout.second ? <PrimaryOption option={layout.second} busy={busy} metamask={metamask} onConnect={(o) => void connect(o)} /> : null}
                </div>
              </div>
              <div className={styles.more}>
                {layout.more.length > 0 ? (
                  <button type="button" className={styles.moreButton} aria-expanded={showMore} onClick={() => setShowMore((v) => !v)} data-testid="show-more-button">
                    {t("connection.more_options")}
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                      <path d={showMore ? "m12 8-6 6 1.41 1.41L12 10.83l4.59 4.58L18 14z" : "M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z"} />
                    </svg>
                  </button>
                ) : null}
                {showMore && layout.more.length > 0 ? (
                  <div className={styles.grid} data-testid="more-options">
                    {layout.more.map((option) => (
                      <GridOption key={option} option={option} busy={busy} metamask={metamask} onConnect={(o) => void connect(o)} />
                    ))}
                  </div>
                ) : null}
              </div>
            </div>
          </div>
        </div>
      </div>
      {modal ? (
        <ConnectionModal
          state={modal.state}
          detail={modal.detail}
          pairing={modal.pairing}
          onClose={closeModal}
          onTryAgain={() => {
            const option = pending;
            closeModal();
            if (option) void connect(option);
          }}
        />
      ) : null}
      {otpEmail ? (
        <EmailLoginModal
          email={otpEmail}
          verify={verifyCode}
          resend={() => auth.startEmailSignIn(otpEmail)}
          onBack={() => setOtpEmail(null)}
          onClose={() => setOtpEmail(null)}
        />
      ) : null}
    </main>
  );
}
