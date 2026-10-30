import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";

import Button from "../atoms/Button";
import Spinner from "../atoms/Spinner";
import {
  AppleIcon,
  DiscordIcon,
  EmailIcon,
  GoogleIcon,
  WalletIcon,
} from "../atoms/BrandIcons";
import { CameraIcon } from "../atoms/icons";
import Modal from "./Modal";

import "./signinmodalview.css";

type Step = "options" | "wallet" | "qr" | "code" | "shell-wait";

export type SignInSocialProvider = "google" | "apple" | "discord";

type SignInWallet = { rdns: string; name: string; icon?: string };

type SignInShellHandle = {
  identity: Promise<unknown>;
  cancel: () => void;
};

type WalletConnectSession = {
  qrDataUrl: string;
  uri: string;
  connected: Promise<unknown>;
  cancel: () => void;
};

const SOCIALS: {
  provider: SignInSocialProvider;
  label: string;
  Icon: (p: { size?: number }) => ReactNode;
}[] = [
  { provider: "google", label: "Google", Icon: GoogleIcon },
  { provider: "apple", label: "Apple", Icon: AppleIcon },
  { provider: "discord", label: "Discord", Icon: DiscordIcon },
];

type SignInModalViewProps = {
  onClose: () => void;
  onSignedIn?: () => void;
  inAppAvailable: boolean;
  walletAvailable: boolean;
  wallets?: SignInWallet[];
  desktopShell: boolean;
  authError: string | null;
  onStartEmailSignIn: (email: string) => Promise<void>;
  onVerifyEmailSignIn: (email: string, code: string) => Promise<unknown>;
  onSocialSignIn: (provider: SignInSocialProvider) => void;
  onConnectWallet: (walletRdns?: string) => Promise<unknown>;
  onWalletConnect?: (signal: AbortSignal) => Promise<WalletConnectSession | null>;
  onBeginShellSignIn: () => SignInShellHandle;
};

export default function SignInModalView({
  onClose,
  onSignedIn,
  inAppAvailable,
  walletAvailable,
  wallets = [],
  desktopShell,
  authError,
  onStartEmailSignIn,
  onVerifyEmailSignIn,
  onSocialSignIn,
  onConnectWallet,
  onWalletConnect,
  onBeginShellSignIn,
}: SignInModalViewProps) {
  const [step, setStep] = useState<Step>("options");
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [pendingWallet, setPendingWallet] = useState<string | null>(null);
  const [qr, setQr] = useState<WalletConnectSession | null>(null);
  const [qrError, setQrError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const qrRequest = useRef<AbortController | null>(null);

  const inApp = inAppAvailable;
  const shell = desktopShell;
  const shellRef = useRef<SignInShellHandle | null>(null);
  const [shellError, setShellError] = useState<string | null>(null);

  const injectedPath = walletAvailable || wallets.length > 0;
  const showWallet = !shell && (injectedPath || !!onWalletConnect);

  useEffect(() => () => shellRef.current?.cancel(), []);
  useEffect(() => () => qrRequest.current?.abort(), []);

  async function onShellBrowser() {
    if (busy || step === "shell-wait") return;
    setShellError(null);
    setStep("shell-wait");
    const handle = onBeginShellSignIn();
    shellRef.current = handle;
    const identity = await handle.identity;
    shellRef.current = null;
    if (identity) {
      onSignedIn?.();
      onClose();
    } else {
      setShellError("The browser didn't confirm the sign-in. Try again.");
      setStep("options");
    }
  }

  function onShellCancel() {
    shellRef.current?.cancel();
    shellRef.current = null;
    setStep("options");
  }

  async function onEmailContinue(e: FormEvent) {
    e.preventDefault();
    const value = email.trim();
    if (!value || busy) return;
    setBusy(true);
    try {
      await onStartEmailSignIn(value);
      setStep("code");
    } catch {
    } finally {
      setBusy(false);
    }
  }

  async function onVerify(e: FormEvent) {
    e.preventDefault();
    const value = code.trim();
    if (!value || busy) return;
    setBusy(true);
    try {
      const identity = await onVerifyEmailSignIn(email.trim(), value);
      if (identity) {
        onSignedIn?.();
        onClose();
      }
    } finally {
      setBusy(false);
    }
  }

  function onSocial(provider: SignInSocialProvider) {
    if (busy) return;
    onSocialSignIn(provider);
  }

  async function onWalletEntry() {
    if (busy) return;
    if (!injectedPath) {
      await startQr();
      return;
    }
    setStep("wallet");
  }

  async function onWallet(rdns?: string) {
    if (busy) return;
    setBusy(true);
    setPendingWallet(rdns ?? "injected");
    try {
      const identity = await onConnectWallet(rdns);
      if (identity) {
        onSignedIn?.();
        onClose();
      }
    } finally {
      setBusy(false);
      setPendingWallet(null);
    }
  }

  async function startQr() {
    qrRequest.current?.abort();
    const request = new AbortController();
    qrRequest.current = request;
    setQr(null);
    setQrError(null);
    setCopied(false);
    setStep("qr");
    if (!onWalletConnect) {
      setQrError("WalletConnect isn't available right now.");
      return;
    }
    try {
      const session = await onWalletConnect(request.signal);
      if (request.signal.aborted) { session?.cancel(); return; }
      if (!session) throw new Error("WalletConnect isn't available right now.");
      request.signal.addEventListener("abort", session.cancel, { once: true });
      setQr(session);
      const identity = await session.connected;
      if (request.signal.aborted) return;
      if (identity) {
        onSignedIn?.();
        onClose();
      } else {
        setQr(null);
        setQrError("That code expired. Go back and try again.");
      }
    } catch (error) {
      if (!request.signal.aborted) {
        setQr(null);
        setQrError(error instanceof Error ? error.message : "Couldn't connect your wallet. Try again.");
      }
    }
  }

  function leaveQr() {
    qrRequest.current?.abort();
    qrRequest.current = null;
    setQr(null);
    setQrError(null);
    setStep(injectedPath ? "wallet" : "options");
  }

  const emailValid = /.+@.+\..+/.test(email.trim());

  return (
    <Modal
      onClose={onClose}
      width={400}
      ariaLabel="Sign in to Decentraland"
      closeOnBackdrop={false}
    >
      <div className="signin">
        <h2 className="signin__title">Sign in or sign up</h2>

        {step === "options" && (
          <div className="signin__options">
            {shell ? (
              <>
                <Button onClick={onShellBrowser} disabled={busy}>
                  Continue in your browser
                </Button>
                <p className="signin__note">
                  Uses your existing Decentraland session &#x2014; no need to sign in
                  again.
                </p>
                {shellError && (
                  <p className="signin__note" role="alert">
                    {shellError}
                  </p>
                )}
              </>
            ) : null}

            {inApp &&
              SOCIALS.map(({ provider, label, Icon }) => (
                <button
                  key={provider}
                  type="button"
                  className="signin__choice"
                  onClick={() => onSocial(provider)}
                  disabled={busy}
                >
                  <span className="signin__choiceicon" aria-hidden="true">
                    <Icon size={22} />
                  </span>
                  <span className="signin__choicelabel">
                    Continue with {label}
                  </span>
                </button>
              ))}

            {showWallet && (
              <button
                type="button"
                className="signin__choice"
                onClick={onWalletEntry}
                disabled={busy}
              >
                <span className="signin__choiceicon" aria-hidden="true">
                  <WalletIcon size={22} />
                </span>
                <span className="signin__choicelabel">Continue with wallet</span>
              </button>
            )}

            {!shell && !inApp && !showWallet && (
              <p className="signin__note">
                Sign-in is temporarily unavailable (no sign-in provider is
                configured). Please try again shortly.
              </p>
            )}

            {inApp && (
              <>
                <div className="signin__or">
                  <span>or with email</span>
                </div>
                <form className="signin__emailrow" onSubmit={onEmailContinue}>
                  <span className="signin__emailicon" aria-hidden="true">
                    <EmailIcon size={18} />
                  </span>
                  <input
                    className="signin__emailinput"
                    type="email"
                    autoComplete="email"
                    aria-label="Email"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    placeholder="you@example.com"
                    disabled={busy}
                  />
                  <button
                    type="submit"
                    className="signin__emailgo"
                    disabled={busy || !emailValid}
                    aria-label="Continue with email"
                  >
                    {busy && step === "options" ? (
                      <Spinner size={16} />
                    ) : (
                      "Continue"
                    )}
                  </button>
                </form>
              </>
            )}

          </div>
        )}

        {step === "wallet" && (
          <div className="signin__options">
            {wallets.length > 0 ? (
              wallets.map((w) => (
                <button
                  key={w.rdns}
                  type="button"
                  className="signin__choice"
                  onClick={() => onWallet(w.rdns)}
                  disabled={busy}
                >
                  <span className="signin__choiceicon" aria-hidden="true">
                    {w.icon ? (
                      <img src={w.icon} alt="" width={22} height={22} />
                    ) : (
                      <WalletIcon size={22} />
                    )}
                  </span>
                  <span className="signin__choicelabel">
                    Continue with {w.name}
                  </span>
                  {busy && pendingWallet === w.rdns ? <Spinner size={16} /> : null}
                </button>
              ))
            ) : walletAvailable ? (
              <button
                type="button"
                className="signin__choice"
                onClick={() => onWallet()}
                disabled={busy}
              >
                <span className="signin__choiceicon" aria-hidden="true">
                  <WalletIcon size={22} />
                </span>
                <span className="signin__choicelabel">Use a browser wallet</span>
                {busy && pendingWallet ? <Spinner size={16} /> : null}
              </button>
            ) : null}

            {onWalletConnect && <>
            <div className="signin__or">
              <span>or scan with a phone</span>
            </div>
            <button
              type="button"
              className="signin__choice"
              onClick={startQr}
              disabled={busy}
            >
              <span className="signin__choiceicon" aria-hidden="true">
                <CameraIcon size={20} />
              </span>
              <span className="signin__choicelabel">
                Continue with WalletConnect
              </span>
            </button>
            </>}

            <button
              type="button"
              className="signin__back"
              onClick={() => setStep("options")}
            >
              &#x2190; Back
            </button>
          </div>
        )}

        {step === "qr" && (
          <div className="signin__form">
            {qrError ? (
              <p className="signin__note" role="alert">
                {qrError}
              </p>
            ) : qr ? (
              <>
                <p className="signin__brand">
                  <span className="signin__brandname">WalletConnect</span>
                </p>
                <p className="signin__sent">
                  Scan with your wallet app, then approve the sign-in request.
                </p>
                <img
                  className="signin__qr"
                  src={qr.qrDataUrl}
                  alt="WalletConnect QR code"
                />
                <button
                  type="button"
                  className="signin__copylink"
                  onClick={() => {
                    void navigator.clipboard?.writeText(qr.uri).then(
                      () => setCopied(true),
                      () => setCopied(false),
                    );
                  }}
                >
                  {copied ? "Copied \u{2713}" : "Copy connection link"}
                </button>

              </>
            ) : (
              <>
                <Spinner size={24} />
                <p className="signin__sent">Starting phone sign-in&#x2026;</p>
              </>
            )}
            <button type="button" className="signin__back" onClick={leaveQr}>
              &#x2190; Back
            </button>
          </div>
        )}

        {step === "shell-wait" && (
          <div className="signin__form">
            <Spinner size={24} />
            <p className="signin__sent">
              Approve the request in your browser &#x2014; we opened a Decentraland
              tab for you.
            </p>
            <button
              type="button"
              className="signin__back"
              onClick={onShellCancel}
            >
              &#x2190; Cancel
            </button>
          </div>
        )}

        {step === "code" && (
          <form className="signin__form" onSubmit={onVerify}>
            <p className="signin__sent">
              We sent a code to <strong>{email}</strong>.
            </p>
            <label className="signin__label" htmlFor="signin-code">
              Verification code
            </label>
            <input
              id="signin-code"
              className="signin__input"
              inputMode="numeric"
              autoComplete="one-time-code"
              autoFocus
              value={code}
              onChange={(e) => setCode(e.target.value)}
              placeholder="123456"
              disabled={busy}
            />
            <Button type="submit" disabled={busy || !code.trim()}>
              {busy ? <Spinner size={16} /> : "Verify & sign in"}
            </Button>
            <button
              type="button"
              className="signin__back"
              onClick={() => {
                setCode("");
                setStep("options");
              }}
              disabled={busy}
            >
              &#x2190; Use a different email
            </button>
          </form>
        )}

        {authError && (
          <p className="signin__error" role="alert">
            {authError}
          </p>
        )}
      </div>
    </Modal>
  );
}
