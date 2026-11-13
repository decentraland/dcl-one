// from: sites@17b3535a:src/components/Report/ReportForm (ReportForm, FormField, FileUpload, NumberBullet): one form,
// the same fields, copy and order; the submission goes through the node (data/lib/catalyst/landings/report.ts).
import { useEffect, useMemo, useRef, useState, type ChangeEvent, type ReactNode } from "react";
import { useNavigate, useSearchParams } from "react-router";

import { Button } from "@dcl/one-ui/components";
import AnimatedBackground from "@ui/shop/discovery/components/animated-background/AnimatedBackground";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { track, type TrackContext } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/context";
import {
  buildSubmitReport,
  isEthAddress,
  REPORT_LIMITS,
  type EvidenceUpload,
  type ReportReason,
} from "@/data/lib/catalyst/landings/report";
import { openSignIn } from "../../components/auth/signin-store";

import DclLogo from "../../components/DclLogo";
import styles from "./ReportForm.module.css";

const REASONS: { value: ReportReason; label: string }[] = [
  { value: "scam_phishing", label: "Scam/Phishing" },
  { value: "illegal_content", label: "Illegal Content" },
  { value: "harassment", label: "Harassment" },
  { value: "cheating", label: "Cheating" },
  { value: "impersonation", label: "Impersonation" },
];

const MAX_FILE_BYTES = REPORT_LIMITS.maxFileSizeMb * 1024 * 1024;
const ACCEPTED_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp", "video/mp4", "video/webm", "application/pdf"];

const ERRORS = {
  invalidAddress: "Please enter a valid wallet address",
  missingReason: "Please choose a reason",
  missingDescription: "Please include a description of your report",
  missingEvidence: "Please upload the evidence of your issue",
  missingConfirmation: "You must confirm this information is accurate",
};

type FieldProps = {
  number: number;
  label: string;
  helper: string;
  optional?: boolean;
  error?: string;
  children: ReactNode;
};

function Field({ number, label, helper, optional, error, children }: FieldProps) {
  return (
    <div className={styles.field}>
      <p className={styles.label}>
        <span className={styles.bullet}>{number}</span>
        {label}
        {optional ? <span className={styles.optional}>(Optional)</span> : <span className={styles.required}>*</span>}
      </p>
      <p className={styles.helper}>{helper}</p>
      {children}
      {error && <p className={styles.error}>{error}</p>}
    </div>
  );
}

const ZWSP = "\u200b";

/** MUI's outlined small Select markup: the placeholder is an InputLabel beside the combobox, which holds a zero-width
 * space until a reason is chosen. */
function ReasonSelect({ value, onChange, invalid }: { value: ReportReason | ""; onChange: (v: ReportReason) => void; invalid: boolean }) {
  const [open, setOpen] = useState(false);
  const selected = REASONS.find((r) => r.value === value);
  const choose = (v: ReportReason) => {
    onChange(v);
    setOpen(false);
  };
  return (
    <div className={styles.select}>
      {!selected && <label className={styles.selectLabel}>Choose a reason</label>}
      <div className={`${styles.control}${invalid ? ` ${styles.invalid}` : ""}${open ? ` ${styles.focused}` : ""}`}>
        <div
          tabIndex={0}
          role="combobox"
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-label="Reason for Report"
          className={styles.selectValue}
          onClick={() => setOpen((o) => !o)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setOpen(false);
            if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
              e.preventDefault();
              setOpen(true);
            }
          }}
        >
          <span>{selected?.label ?? ZWSP}</span>
        </div>
        <svg className={styles.arrow} viewBox="0 0 24 24" aria-hidden="true" data-testid="ArrowDropDownIcon">
          <path d="M7 10l5 5 5-5z" />
        </svg>
      </div>
      {open && (
        <ul className={styles.menu} role="listbox" aria-label="Reason for Report">
          {REASONS.map((r) => (
            <li
              key={r.value}
              role="option"
              aria-selected={r.value === value}
              tabIndex={0}
              className={styles.option}
              onClick={() => choose(r.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setOpen(false);
                if (e.key !== "Enter" && e.key !== " ") return;
                e.preventDefault();
                choose(r.value);
              }}
            >
              {r.label}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** MUI's Checkbox inside a FormControlLabel: a 42px round hit area around a 24px box icon. */
function ConfirmCheckbox({ checked, onChange, error }: { checked: boolean; onChange: (v: boolean) => void; error: boolean }) {
  const [testId, d] = checked
    ? ["CheckBoxIcon", "M19 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.11 0 2-.9 2-2V5c0-1.1-.89-2-2-2zm-9 14l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"]
    : ["CheckBoxOutlineBlankIcon", "M19 5v14H5V5h14m0-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z"];
  return (
    <label className={styles.confirm}>
      <span className={`${styles.checkbox}${checked ? ` ${styles.checked}` : ""}${error ? ` ${styles.checkboxError}` : ""}`}>
        <input type="checkbox" className={styles.checkboxInput} checked={checked} onChange={(e) => onChange(e.target.checked)} />
        <svg className={styles.checkboxIcon} viewBox="0 0 24 24" aria-hidden="true" data-testid={testId}>
          <path d={d} />
        </svg>
      </span>
      <span className={styles.confirmLabel}>I confirm this report is accurate.</span>
    </label>
  );
}

function FileUpload({ files, onChange }: { files: EvidenceUpload[]; onChange: (files: EvidenceUpload[]) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [problem, setProblem] = useState<string | null>(null);

  const select = (e: ChangeEvent<HTMLInputElement>) => {
    const picked = Array.from(e.target.files ?? []).slice(0, REPORT_LIMITS.maxFiles - files.length);
    const badType = picked.filter((f) => !ACCEPTED_TYPES.includes(f.type));
    const tooBig = picked.filter((f) => ACCEPTED_TYPES.includes(f.type) && f.size > MAX_FILE_BYTES);
    setProblem(
      badType.length
        ? `${badType.map((f) => f.name).join(", ")} is not a supported file type`
        : tooBig.length
          ? `${tooBig.map((f) => f.name).join(", ")} exceeded the 5MB limit`
          : null,
    );
    const next = picked
      .filter((f) => ACCEPTED_TYPES.includes(f.type) && f.size <= MAX_FILE_BYTES)
      .map((f) => ({ id: crypto.randomUUID(), name: f.name, size: f.size, blob: f, contentType: f.type }));
    if (next.length) onChange([...files, ...next]);
    if (input.current) input.current.value = "";
  };

  return (
    <div className={styles.upload}>
      <input ref={input} className={styles.fileInput} type="file" multiple accept={ACCEPTED_TYPES.join(",")} onChange={select} />
      {files.length > 0 && (
        <div className={styles.chips}>
          {files.map((f) => (
            <span key={f.id} className={styles.chip}>
              {f.name}
              <button type="button" className={styles.chipRemove} aria-label={`Remove ${f.name}`} onClick={() => onChange(files.filter((x) => x.id !== f.id))}>
                ✕
              </button>
            </span>
          ))}
        </div>
      )}
      <Button variant="secondary" size="md" className={styles.addFile} onClick={() => input.current?.click()} disabled={files.length >= REPORT_LIMITS.maxFiles}>
        Add File
      </Button>
      {problem && <p className={styles.error}>{problem}</p>}
    </div>
  );
}

export default function ReportForm({ trackCtx }: { trackCtx: TrackContext }) {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const { identity, address, isConnected } = useAuth();
  const submitReport = useMemo(() => buildSubmitReport(identity), [identity]);

  const playerParam = params.get("player_address") ?? "";
  const reportedParam = params.get("reported_address") ?? "";
  const wallet = address ?? "";

  const [reportedAddress, setReportedAddress] = useState(reportedParam);
  const [reason, setReason] = useState<ReportReason | "">("");
  const [description, setDescription] = useState("");
  const [evidence, setEvidence] = useState<EvidenceUpload[]>([]);
  const [comments, setComments] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (reportedParam) setReportedAddress(reportedParam);
  }, [reportedParam]);

  const [startCtx] = useState(trackCtx);
  useEffect(() => {
    track("report_started", {}, startCtx);
  }, [startCtx]);

  const mismatch = !!wallet && !!playerParam && wallet.toLowerCase() !== playerParam.toLowerCase();
  const errors = {
    player: isEthAddress(wallet) ? "" : ERRORS.invalidAddress,
    reported: isEthAddress(reportedAddress) ? "" : ERRORS.invalidAddress,
    reason: reason ? "" : ERRORS.missingReason,
    description: description.trim() ? "" : ERRORS.missingDescription,
    evidence: evidence.length ? "" : ERRORS.missingEvidence,
    confirm: confirmed ? "" : ERRORS.missingConfirmation,
  };
  const shown = (e: string) => (submitted ? e || undefined : undefined);

  const submit = async () => {
    setSubmitted(true);
    track("report_review_reached", {}, trackCtx);
    const invalid = Object.entries(errors).flatMap(([field, e]) => (e ? [field] : []));
    if (invalid.length || mismatch) {
      track("report_validation_failed", { fields: mismatch ? [...invalid, "player"] : invalid, step: "form" }, trackCtx);
      return;
    }
    setSubmitting(true);
    setFailed(false);
    track("report_submit_started", { reason, evidence_count: evidence.length }, trackCtx);
    try {
      const result = await submitReport({
        draft: { playerAddress: wallet, reportedAddress, reason, description, evidence, additionalComments: comments, confirmAccuracy: confirmed },
      });
      track("report_completed", { reason, report_id: result.reportId, evidence_count: evidence.length }, trackCtx);
      navigate("/report/success", { state: { submitted: true } });
    } catch (err) {
      track("report_failed", { reason: err instanceof Error ? err.message : undefined }, trackCtx);
      setFailed(true);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <SitesChrome active="legal" overlayNav>
      <main className={styles.page}>
        <AnimatedBackground />
        <div className={styles.content}>
          <div className={styles.card}>
            <div className={styles.logoWrap}>
              <DclLogo className={styles.logo} />
              <h3 className={styles.title}>Report User</h3>
            </div>

            {mismatch && (
              <div className={styles.mismatch}>
                The connected wallet does not match the player address. Please connect with the correct wallet to submit this report.
              </div>
            )}

            <Field
              number={1}
              label="Your Wallet Address"
              helper="Reports are tied to your wallet to prevent abuse. Your identity will not be publicly shared."
              error={shown(errors.player)}
            >
              <div className={`${styles.control} ${styles.disabled}`}>
                <input className={styles.input} placeholder="Write or paste your address here..." value={wallet} disabled readOnly />
              </div>
            </Field>

            {!isConnected && (
              <div className={styles.signIn}>
                Please sign in with your wallet to submit a report.
                <Button variant="text" size="sm" className={styles.signInButton} onClick={() => openSignIn()}>
                  Sign In
                </Button>
              </div>
            )}

            <Field
              number={2}
              label="Reported User Wallet"
              helper="This is the wallet address of the user you are reporting."
              error={shown(errors.reported)}
            >
              <div className={`${styles.control}${reportedParam ? ` ${styles.disabled}` : ""}`}>
                <input
                  className={styles.input}
                  placeholder="Write or paste an address here..."
                  value={reportedAddress}
                  onChange={(e) => setReportedAddress(e.target.value)}
                  onBlur={() => isEthAddress(reportedAddress) && track("report_target_set", { has_reporter: isEthAddress(wallet) }, trackCtx)}
                  disabled={!!reportedParam}
                />
              </div>
            </Field>

            <Field number={3} label="Reason for Report" helper="Select the option that best describes the issue." error={shown(errors.reason)}>
              <ReasonSelect
                value={reason}
                onChange={(v) => {
                  setReason(v);
                  track("report_category_set", { reason: v }, trackCtx);
                }}
                invalid={submitted && !!errors.reason} />
            </Field>

            <Field
              number={4}
              label="Description"
              helper="Describe what happened. Include when and where the incident occurred if possible."
              error={shown(errors.description)}
            >
              <div className={styles.inputGroup}>
                <div className={`${styles.control} ${styles.multiline}`}>
                  <textarea
                    className={`${styles.textarea} ${styles.rows4}`}
                    rows={4}
                    placeholder="Write your description here..."
                    value={description}
                    onChange={(e) => setDescription(e.target.value.slice(0, REPORT_LIMITS.descriptionMax))}
                    onBlur={() => description.trim() && track("report_details_set", { description_len: description.trim().length }, trackCtx)}
                  />
                </div>
                <p className={styles.counter}>{`${description.length} / ${REPORT_LIMITS.descriptionMax}`}</p>
                <p className={styles.hint}>The more detail you provide, the easier it is for moderators to review your report.</p>
              </div>
            </Field>

            <Field
              number={5}
              label="Evidence"
              helper="Upload up to 5 screenshots, videos, or links to support your report."
              error={shown(errors.evidence)}
            >
              <FileUpload
                files={evidence}
                onChange={(files) => {
                  if (files.length > evidence.length) track("report_evidence_added", { file_count: files.length }, trackCtx);
                  setEvidence(files);
                }}
              />
            </Field>

            <Field number={6} label="Additional Comments" optional helper="Provide any extra information that might help moderators investigate.">
              <div className={styles.inputGroup}>
                <div className={`${styles.control} ${styles.multiline}`}>
                  <textarea
                    className={`${styles.textarea} ${styles.rows3}`}
                    rows={3}
                    placeholder="Write your comments here..."
                    value={comments}
                    onChange={(e) => setComments(e.target.value.slice(0, REPORT_LIMITS.commentsMax))}
                  />
                </div>
                <p className={styles.counter}>{`${comments.length} / ${REPORT_LIMITS.commentsMax}`}</p>
              </div>
            </Field>

            <div className={styles.field}>
              <ConfirmCheckbox checked={confirmed} onChange={setConfirmed} error={submitted && !!errors.confirm} />
              {submitted && errors.confirm && <p className={styles.error}>{errors.confirm}</p>}
            </div>

            {failed && <p className={styles.submitError}>We couldn&apos;t submit your report. Please try again.</p>}

            <Button size="md" className={styles.submit} onClick={submit} disabled={mismatch || !isConnected || !confirmed || submitting}>
              {submitting ? "Submitting..." : "Submit Report"}
            </Button>
          </div>

          <p className={styles.footer}>
            Need help instead?
            <br />
            For urgent issues or security concerns, please{" "}
            <a className={styles.footerLink} href="/help" target="_blank" rel="noopener noreferrer">
              contact the Support team
            </a>
            .
          </p>
        </div>
      </main>
    </SitesChrome>
  );
}
