// from: auth@e081cc6:src/components/ConnectionModal/{ConnectionModal,ConnectionLayout}.tsx (the wallet's progress and
//       failure, with the support detail and Try Again), src/components/EmailLoginModal/{EmailLoginModal.tsx,
//       useEmailOtp.ts} (the six-digit code: auto-advance, paste, auto-submit, the 90-second resend cooldown)
// decentraland/auth is Apache-2.0.
// Upstream shows WalletConnect's own modal; this site has no such SDK, so its pairing code sits in the same modal.

import logo from "@ui/atoms/assets/decentraland-logo.svg?url";
import { useCallback, useEffect, useRef, useState, type ClipboardEvent, type KeyboardEvent, type ReactNode } from "react";

import emailIcon from "./assets/email.svg?url";
import welcomeBackground from "./assets/custom-welcome-background.webp?url";
import { t, type CopyKey } from "./copy";
import { LOADING_STATES, type ConnectionState } from "./flow";
import styles from "./LoginModals.module.css";

export function Spinner({ className }: { className?: string }) {
  return (
    <svg className={`${styles.spinner} ${className ?? ""}`} viewBox="22 22 44 44" aria-hidden="true">
      <circle cx="44" cy="44" r="20.2" fill="none" strokeWidth="3.6" />
    </svg>
  );
}

function useEscape(onEscape: () => void) {
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") onEscape();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onEscape]);
}

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])';

/**
 * The dimmed backdrop: a press outside the dialog, or Escape, dismisses it. The child is the dialog itself. Focus moves
 * into the dialog when it opens (unless something inside already took it), Tab cycles within it, and focus returns to
 * where it was when the dialog closes.
 */
function Overlay({ className, onDismiss, children }: { className?: string; onDismiss: () => void; children: ReactNode }) {
  useEscape(onDismiss);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    const box = root.current;
    if (box && !box.contains(document.activeElement)) box.querySelector<HTMLElement>(FOCUSABLE)?.focus();
    return () => opener?.focus?.();
  }, []);
  const trap = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Tab" || !root.current) return;
    const items = [...root.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
    if (items.length === 0) return;
    const first = items[0]!;
    const last = items[items.length - 1]!;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };
  return (
    <div
      ref={root}
      className={`${styles.overlay} ${className ?? ""}`}
      role="presentation"
      onKeyDown={trap}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onDismiss();
      }}
    >
      {children}
    </div>
  );
}

const STATE_COPY: Record<ConnectionState, CopyKey> = {
  connecting: "connection_layout.waiting_for_signature",
  signing: "connection_layout.waiting_for_signature",
  validating: "connection_layout.validating_sign_in",
  redirecting: "connection_layout.loading_magic",
  error: "connection_layout.error",
  "error-generic": "connection_layout.error_generic",
  "error-locked": "connection_layout.error_locked_wallet",
  "error-signature-unsupported": "connection_layout.error_wallet_signature_unsupported",
  "error-connection-failed": "connection_layout.error_connection_failed",
};

export type PairingCode = { qrDataUrl: string; uri: string };

export function ConnectionModal({
  state,
  detail,
  pairing,
  onClose,
  onTryAgain,
}: {
  state: ConnectionState;
  detail: string | null;
  pairing: PairingCode | null;
  onClose: () => void;
  onTryAgain: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const loading = LOADING_STATES.includes(state);
  const message = t(STATE_COPY[state]);
  return (
    <Overlay onDismiss={onClose}>
      <div
        className={`${styles.paper} ${styles.elevation}`}
        style={{ backgroundImage: `url("${welcomeBackground}")` }}
        role="dialog"
        aria-modal="true"
        aria-label={message}
        data-testid="connection-modal"
      >
        <div className={styles.paperHeader}>
          <button type="button" className={styles.iconButton} aria-label="close" onClick={onClose}>
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 2C6.47 2 2 6.47 2 12s4.47 10 10 10 10-4.47 10-10S17.53 2 12 2m4.3 14.3c-.39.39-1.02.39-1.41 0L12 13.41 9.11 16.3c-.39.39-1.02.39-1.41 0a.996.996 0 0 1 0-1.41L10.59 12 7.7 9.11a.996.996 0 0 1 0-1.41c.39-.39 1.02-.39 1.41 0L12 10.59l2.89-2.89c.39-.39 1.02-.39 1.41 0s.39 1.02 0 1.41L13.41 12l2.89 2.89c.38.38.38 1.02 0 1.41" />
            </svg>
          </button>
        </div>
        <div className={styles.paperContent}>
          <div className={styles.layout}>
            <img className={styles.layoutLogo} src={logo} alt="" width={48} height={48} />
            <p className={styles.layoutTitle} data-testid="connection-status">{message}</p>
            {pairing && loading ? (
              <>
                <img className={styles.qr} src={pairing.qrDataUrl} alt="WalletConnect pairing code" width={240} height={240} />
                <button
                  type="button"
                  className={styles.copyLink}
                  onClick={() => void navigator.clipboard?.writeText(pairing.uri).then(() => setCopied(true), () => {})}
                >
                  {copied ? "Link copied" : "Copy link"}
                </button>
              </>
            ) : loading ? (
              <div className={styles.progress} role="progressbar" aria-label={message}>
                <Spinner />
              </div>
            ) : (
              <>
                {detail ? (
                  <div className={styles.errorDetail}>
                    <p className={styles.errorDescription}>{t("connection_layout.error_contact_support")}</p>
                    <p className={styles.errorCode} data-testid="connection-error-detail">{detail}</p>
                  </div>
                ) : null}
                <div className={styles.errorActions}>
                  <button type="button" className={styles.tryAgain} onClick={onTryAgain} data-testid="connection-try-again-button">
                    {t("common.try_again")}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </Overlay>
  );
}

const OTP_LENGTH = 6;
export const RESEND_COOLDOWN_SECONDS = 90;

function countdown(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

/** Upstream's wording for a failed call: the network notice, a known reason, or the service's own text. */
function apiError(err: unknown, fallback: CopyKey, known: [string, CopyKey][] = []): string {
  const raw = err instanceof Error ? err.message : "";
  if (raw === "Failed to fetch" || raw.toLowerCase().includes("network")) return t("email_login_modal.network_error");
  const hit = known.find(([pattern]) => raw.toLowerCase().includes(pattern));
  if (hit) return t(hit[1]);
  return raw || t(fallback);
}

export function EmailLoginModal({
  email,
  verify,
  resend,
  onBack,
  onClose,
}: {
  email: string;
  verify: (code: string) => Promise<void>;
  resend: () => Promise<void>;
  onBack: () => void;
  onClose: () => void;
}) {
  const [otp, setOtp] = useState<string[]>(() => Array<string>(OTP_LENGTH).fill(""));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [shake, setShake] = useState(false);
  const [cooldown, setCooldown] = useState(RESEND_COOLDOWN_SECONDS);
  const inputs = useRef<(HTMLInputElement | null)[]>([]);
  // The boxes are disabled while a code is checked, which drops the focus; a refused code hands it back.
  const refocus = useRef(false);
  useEffect(() => {
    if (!busy && refocus.current) {
      refocus.current = false;
      inputs.current[OTP_LENGTH - 1]?.focus();
    }
  }, [busy]);

  useEffect(() => {
    const timer = setTimeout(() => inputs.current[0]?.focus(), 100);
    return () => clearTimeout(timer);
  }, []);
  useEffect(() => {
    if (cooldown <= 0) return;
    const timer = setTimeout(() => setCooldown((s) => Math.max(s - 1, 0)), 1000);
    return () => clearTimeout(timer);
  }, [cooldown]);

  const submit = useCallback(
    async (code: string) => {
      if (code.length !== OTP_LENGTH) {
        setError(t("email_login_modal.enter_complete_code"));
        return;
      }
      setBusy(true);
      setError(null);
      setShake(false);
      try {
        await verify(code);
      } catch (err) {
        setError(apiError(err, "email_login_modal.invalid_or_expired", [["failed to verify", "email_login_modal.failed_verify"]]));
        setShake(true);
        refocus.current = true;
      } finally {
        setBusy(false);
      }
    },
    [verify],
  );

  /** Puts digits into the boxes from `start` on, and submits once all six are there. */
  const fill = (start: number, digits: string) => {
    const next = start === 0 && digits.length >= OTP_LENGTH ? Array<string>(OTP_LENGTH).fill("") : [...otp];
    for (let i = 0; i < digits.length && start + i < OTP_LENGTH; i++) next[start + i] = digits[i]!;
    setOtp(next);
    setError(null);
    setShake(false);
    inputs.current[Math.min(start + digits.length, OTP_LENGTH - 1)]?.focus();
    if (next.every(Boolean)) void submit(next.join(""));
  };

  const change = (index: number, value: string) => {
    let typed = value.replace(/\D/g, "");
    // A digit typed over the box's own replaces it, wherever the caret was.
    const own = otp[index];
    if (own && typed.length === 2 && typed.includes(own)) typed = typed.startsWith(own) ? typed[1]! : typed[0]!;
    // A phone's one-time-code autofill puts the whole code into one box.
    if (typed.length > 1) {
      fill(index, typed.length > OTP_LENGTH ? typed.slice(-OTP_LENGTH) : typed);
      return;
    }
    const digit = typed.slice(-1);
    const next = [...otp];
    next[index] = digit;
    setOtp(next);
    setError(null);
    setShake(false);
    if (digit && index < OTP_LENGTH - 1) inputs.current[index + 1]?.focus();
    // Upstream submits only from the last box; any box completing the code submits here, so a corrected digit counts.
    if (digit && next.every(Boolean)) void submit(next.join(""));
  };
  const keyDown = (index: number, e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Backspace" && !otp[index] && index > 0) inputs.current[index - 1]?.focus();
    if (e.key === "Enter") void submit(otp.join(""));
  };
  const paste = (e: ClipboardEvent<HTMLInputElement>) => {
    e.preventDefault();
    const digits = e.clipboardData.getData("text").replace(/\D/g, "").slice(0, OTP_LENGTH);
    if (!digits) return;
    const next = Array<string>(OTP_LENGTH).fill("");
    for (let i = 0; i < digits.length; i++) next[i] = digits[i]!;
    setOtp(next);
    setError(null);
    setShake(false);
    inputs.current[digits.length < OTP_LENGTH ? digits.length : OTP_LENGTH - 1]?.focus();
    if (digits.length === OTP_LENGTH) void submit(digits);
  };
  const resendCode = async () => {
    if (busy || cooldown > 0) return;
    setOtp(Array<string>(OTP_LENGTH).fill(""));
    setError(null);
    setShake(false);
    setBusy(true);
    try {
      await resend();
      setCooldown(RESEND_COOLDOWN_SECONDS);
      setTimeout(() => inputs.current[0]?.focus(), 100);
    } catch (err) {
      setError(apiError(err, "email_login_modal.failed_resend"));
      setShake(true);
    } finally {
      setBusy(false);
    }
  };

  const resendDisabled = busy || cooldown > 0;
  const resendText = `${t("email_login_modal.resend_code")}${cooldown > 0 ? ` (${countdown(cooldown)})` : ""}`;
  const dismiss = () => {
    if (!busy) onClose();
  };

  return (
    <Overlay className={styles.otpOverlay} onDismiss={dismiss}>
      <div
        className={`${styles.otpPaper} ${styles.elevation}`}
        role="dialog"
        aria-modal="true"
        aria-label={t("email_login_modal.title")}
        data-testid="email-login-modal"
      >
        <div className={styles.otpHeader}>
          <button type="button" className={styles.back} onClick={() => !busy && onBack()} disabled={busy} data-testid="email-login-back-button">
            <span>‹</span> {t("email_login_modal.back")}
          </button>
          <button type="button" className={styles.close} onClick={dismiss} disabled={busy} aria-label="Close" data-testid="email-login-close-button">
            ×
          </button>
        </div>
        <div className={styles.otpMain}>
          <div className={styles.otpContent}>
            <div className={styles.emailIcon}>
              <img src={emailIcon} alt="" />
            </div>
            <p className={styles.otpTitle}>{t("email_login_modal.title")}</p>
            <p className={styles.otpSubtitle}>
              {t("email_login_modal.subtitle_prefix")}
              <strong>{email}</strong>
              {t("email_login_modal.subtitle_suffix")}
            </p>
            <div className={styles.otpInputs} data-error={shake}>
              {otp.map((digit, index) => (
                <input
                  key={index}
                  ref={(el) => {
                    inputs.current[index] = el;
                  }}
                  className={styles.otpInput}
                  data-testid={`otp-input-${index}`}
                  aria-label={`Digit ${index + 1} of ${OTP_LENGTH}`}
                  type="text"
                  inputMode="numeric"
                  value={digit}
                  onChange={(e) => change(index, e.target.value)}
                  onKeyDown={(e) => keyDown(index, e)}
                  onPaste={paste}
                  disabled={busy}
                  autoComplete="one-time-code"
                />
              ))}
            </div>
            {busy && !error ? (
              <div className={styles.verifying} role="status">
                <Spinner />
                <span>{t("email_login_modal.verifying")}</span>
              </div>
            ) : null}
            {error ? (
              <>
                <p className={styles.otpError} role="alert">{error}</p>
                <button type="button" className={styles.resend} data-tone="plain" disabled={resendDisabled} onClick={() => void resendCode()}>
                  {resendText}
                </button>
              </>
            ) : (
              <p className={styles.resendText}>
                {t("email_login_modal.didnt_get_email")}
                <button type="button" className={styles.resend} disabled={resendDisabled} onClick={() => void resendCode()}>
                  {resendText}
                </button>
              </p>
            )}
          </div>
        </div>
      </div>
    </Overlay>
  );
}
