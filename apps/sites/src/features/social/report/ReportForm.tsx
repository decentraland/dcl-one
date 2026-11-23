// The report form: one form with decentraland.org's fields, copy and order; the submission goes through the node (data/lib/catalyst/landings/report.ts).
import { useEffect, useEffectEvent, useMemo, useRef, useState, type ChangeEvent, type ReactNode } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";

import { Button, CheckboxLabel, InputWebCreatorTools, WebSelect, WebTextarea } from "@dcl/one-ui";
import { AnimatedBackground } from "@dcl/one-ui/components/AnimatedBackground/AnimatedBackground";
import { GeneralClose, GeneralDecentralandColor } from "@dcl/one-ui/foundations/icons";
import SitesLayout from "../../components/chrome/SitesLayout";

import { track, type TrackContext } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/context";
import {
  buildSubmitReport,
  REPORT_EVIDENCE_TYPES,
  ReportSubmitError,
  validateAddress,
  validateDraft,
  type ReportDraft,
  isEthAddress,
  REPORT_LIMITS,
  type EvidenceUpload,
  type ReportReason,
} from "@/data/lib/catalyst/landings/report";
import { onBeforeBrowserSignIn, openSignIn } from "../../components/auth/signin-store";

import styles from "./ReportForm.module.css";
import { saveSignInDraft, takeSignInDraft } from "./sign-in-draft";

const REASONS: { value: ReportReason; label: string }[] = [
  { value: "scam_phishing", label: "Scam/Phishing" },
  { value: "illegal_content", label: "Illegal Content" },
  { value: "harassment", label: "Harassment" },
  { value: "cheating", label: "Cheating" },
  { value: "impersonation", label: "Impersonation" },
];

const MAX_FILE_BYTES = REPORT_LIMITS.maxFileSizeMb * 1024 * 1024;
const ACCEPTED_TYPES = REPORT_EVIDENCE_TYPES;
const SUBMIT_FAILED = "We couldn't submit your report. Please try again.";
// The failures the visitor can act on show their own message; the others show the general one.
const SHOWN_STEPS: readonly string[] = ["not_connected", "missing_bytes", "upload"];

type FieldProps = {
  number: number;
  label: string;
  helper: string;
  optional?: boolean;
  error?: string;
  children: ReactNode;
};

/** The name and the description of a field's control: the field's label, and its helper and error lines. */
function named(number: number, error?: string) {
  const id = `report-field-${number}`;
  return {
    "aria-labelledby": `${id}-label`,
    "aria-describedby": error ? `${id}-help ${id}-error` : `${id}-help`,
  };
}

function Field({ number, label, helper, optional, error, children }: FieldProps) {
  const id = `report-field-${number}`;
  return (
    <div className={styles.field}>
      <p className={styles.label}>
        <span className={styles.bullet}>{number}</span>
        <span id={`${id}-label`}>{label}</span>
        {optional ? <span className={styles.optional}>(Optional)</span> : <span className={styles.required}>*</span>}
      </p>
      <p id={`${id}-help`} className={styles.helper}>
        {helper}
      </p>
      {children}
      {error && (
        <p id={`${id}-error`} className={styles.error}>
          {error}
        </p>
      )}
    </div>
  );
}

function FileUpload({
  files,
  onChange,
  invalid,
}: {
  files: EvidenceUpload[];
  onChange: (files: EvidenceUpload[]) => void;
  invalid: boolean;
}) {
  const input = useRef<HTMLInputElement>(null);
  const nextAttachmentId = useRef(0);
  const [problem, setProblem] = useState<string | null>(null);

  const select = (e: ChangeEvent<HTMLInputElement>) => {
    const chosen = Array.from(e.target.files ?? []);
    const picked = chosen.slice(0, REPORT_LIMITS.maxFiles - files.length);
    const next: EvidenceUpload[] = [];
    const problems: string[] = [];
    for (const file of picked) {
      if (!ACCEPTED_TYPES.includes(file.type)) {
        problems.push(`${file.name} is not a supported file type.`);
      } else if (file.size === 0) {
        problems.push(`${file.name} is empty.`);
      } else if (file.size > MAX_FILE_BYTES) {
        problems.push(`${file.name} exceeded the ${REPORT_LIMITS.maxFileSizeMb}MB limit.`);
      } else {
        next.push({
          id: String(nextAttachmentId.current++),
          name: file.name,
          size: file.size,
          blob: file,
          contentType: file.type,
        });
      }
    }
    if (chosen.length > picked.length) {
      problems.push(`Add up to ${REPORT_LIMITS.maxFiles} files per report.`);
    }
    setProblem(problems.join(" ") || null);
    if (next.length) onChange([...files, ...next]);
    if (input.current) input.current.value = "";
  };

  return (
    <div className={styles.upload}>
      <input
        ref={input}
        className={styles.fileInput}
        type="file"
        aria-label="Evidence"
        multiple
        accept={ACCEPTED_TYPES.join(",")}
        onChange={select}
      />
      {files.length > 0 && (
        <ul className={styles.chips} aria-label="Attached evidence">
          {files.map((f) => (
            <li key={f.id} className={styles.chip}>
              <span className={styles.fileName}>{f.name}</span>
              <Button
                type="button"
                variant="text"
                size="sm"
                iconOnly
                aria-label={`Remove ${f.name}`}
                onClick={() => onChange(files.filter((x) => x.id !== f.id))}
              >
                <GeneralClose size={16} aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}
      <Button
        variant="secondary"
        size="md"
        className={styles.addFile}
        data-missing-evidence={invalid || undefined}
        aria-describedby={invalid ? "report-field-5-help report-field-5-error" : "report-field-5-help"}
        onClick={() => input.current?.click()}
        disabled={files.length >= REPORT_LIMITS.maxFiles}
      >
        Add File
      </Button>
      {problem && (
        <p className={styles.error} role="alert">
          {problem}
        </p>
      )}
    </div>
  );
}

export default function ReportForm({ trackCtx }: { trackCtx: TrackContext }) {
  const navigate = useNavigate();
  const { pathname, search, hash } = useLocation();
  const returnTo = pathname + search + hash;
  const [params] = useSearchParams();
  const { identity, address, isConnected } = useAuth();
  const submitReport = useMemo(() => buildSubmitReport(identity), [identity]);

  const playerParam = params.get("player_address") ?? "";
  const reportedParam = params.get("reported_address") ?? "";
  const reportedAddressLocked = isEthAddress(reportedParam);
  const wallet = address ?? "";

  const [reportedAddress, setReportedAddress] = useState(reportedParam);
  const [reason, setReason] = useState<ReportReason | "">("");
  const [description, setDescription] = useState("");
  const [evidence, setEvidence] = useState<EvidenceUpload[]>([]);
  const [reattachEvidence, setReattachEvidence] = useState(false);
  const [comments, setComments] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [submitAttempts, setSubmitAttempts] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const [draftSaveFailed, setDraftSaveFailed] = useState(false);
  const [confirmingSignIn, setConfirmingSignIn] = useState(false);
  const signInTrigger = useRef<HTMLElement | null>(null);
  const resumeSignIn = useRef<(() => void) | null>(null);
  const cancelSignIn = useRef<HTMLButtonElement>(null);
  const discardConfirmed = useRef(false);
  useEffect(() => {
    if (confirmingSignIn) cancelSignIn.current?.focus();
  }, [confirmingSignIn]);

  const lastReportedParam = useRef(reportedParam);
  useEffect(() => {
    if (lastReportedParam.current === reportedParam) return;
    lastReportedParam.current = reportedParam;
    if (reportedParam) setReportedAddress(reportedParam);
  }, [reportedParam]);

  const restoreSignInDraft = useEffectEvent(() => {
    const saved = takeSignInDraft(returnTo);
    if (!saved) return;
    setReportedAddress(reportedAddressLocked ? reportedParam : saved.reportedAddress);
    setReason(saved.reason);
    setDescription(saved.description);
    setComments(saved.comments);
    setReattachEvidence(saved.hadEvidence);
  });
  useEffect(() => {
    restoreSignInDraft();
  }, []);

  const saveBrowserDraft = useEffectEvent((destination: string, resume: () => void) => {
    if (destination !== returnTo) return;
    if (evidence.length && !discardConfirmed.current) {
      signInTrigger.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      resumeSignIn.current = resume;
      setConfirmingSignIn(true);
      return false;
    }
    const hasText = Boolean(reportedAddress !== reportedParam || reason || description || comments);
    const hadEvidence = reattachEvidence || evidence.length > 0;
    if (!hasText && !hadEvidence) return;
    if (!saveSignInDraft(returnTo, { reportedAddress, reason, description, comments, hadEvidence }) && hasText) {
      setDraftSaveFailed(true);
      return false;
    }
  });
  useEffect(() => onBeforeBrowserSignIn((destination, resume) => saveBrowserDraft(destination, resume)), []);

  function keepEditing() {
    resumeSignIn.current = null;
    setConfirmingSignIn(false);
    signInTrigger.current?.focus();
  }

  function continueSignIn() {
    const resume = resumeSignIn.current;
    resumeSignIn.current = null;
    setConfirmingSignIn(false);
    discardConfirmed.current = true;
    try {
      resume?.();
    } finally {
      discardConfirmed.current = false;
    }
  }

  const [startCtx] = useState(trackCtx);
  useEffect(() => {
    track("report_started", {}, startCtx);
  }, [startCtx]);

  const mismatch = !!wallet && !!playerParam && wallet.toLowerCase() !== playerParam.toLowerCase();
  const draft: ReportDraft = {
    playerAddress: wallet,
    reportedAddress,
    reason,
    description,
    evidence,
    additionalComments: comments,
    confirmAccuracy: confirmed,
  };
  // The report builder's own checks, under the names the form's fields and its telemetry use.
  const checked = validateDraft(draft);
  const errors = {
    player: validateAddress(wallet),
    reported: checked.reportedAddress,
    reason: checked.reason,
    description: checked.description,
    evidence: checked.evidence,
    confirm: checked.confirmAccuracy,
  };
  const shown = (e: string) => (submitAttempts > 0 ? e || undefined : undefined);

  const formContent = useRef<HTMLDivElement>(null);
  const focusInvalid = useRef(false);
  useEffect(() => {
    if (!focusInvalid.current) return;
    focusInvalid.current = false;
    formContent.current?.querySelector<HTMLElement>('[aria-invalid="true"]:not(:disabled), button[data-missing-evidence]')?.focus();
  }, [submitAttempts]);

  const busy = useRef(false);
  const submit = async () => {
    if (busy.current) return;
    setSubmitAttempts((attempts) => attempts + 1);
    track("report_review_reached", {}, trackCtx);
    const invalid = Object.entries(errors).flatMap(([field, e]) => (e ? [field] : []));
    if (invalid.length || mismatch) {
      focusInvalid.current = !mismatch;
      track("report_validation_failed", { fields: mismatch ? [...invalid, "player"] : invalid, step: "form" }, trackCtx);
      return;
    }
    busy.current = true;
    setSubmitting(true);
    setFailure(null);
    track("report_submit_started", { reason, evidence_count: evidence.length }, trackCtx);
    try {
      const result = await submitReport({
        draft,
      });
      track("report_completed", { reason, report_id: result.reportId, evidence_count: evidence.length }, trackCtx);
      navigate("/report/success", { state: { submitted: true } });
    } catch (err) {
      track("report_failed", { reason: err instanceof ReportSubmitError ? err.step : "request" }, trackCtx);
      setFailure(err instanceof ReportSubmitError && SHOWN_STEPS.includes(err.step) ? err.message : SUBMIT_FAILED);
    } finally {
      busy.current = false;
      setSubmitting(false);
    }
  };

  return (
    <SitesLayout active="legal" overlayNav>
      <main className={styles.page}>
        <AnimatedBackground />
        <div className={styles.content}>
          <div className={styles.card} ref={formContent}>
            <div className={styles.logoWrap}>
              <GeneralDecentralandColor className={styles.logo} aria-hidden="true" />
              <h1 className={styles.title}>Report User</h1>
            </div>

            {mismatch && (
              <div className={styles.mismatch} role="alert">
                The connected wallet does not match the player address. Please connect with the correct wallet to submit this report.
              </div>
            )}

            <Field
              number={1}
              label="Your Wallet Address"
              helper="Reports are tied to your wallet to prevent abuse. Your identity will not be publicly shared."
            >
              <InputWebCreatorTools
                {...named(1)}
                label="Your Wallet Address"
                labelHidden
                error={shown(errors.player)}
                placeholder="Write or paste your address here..."
                value={wallet}
                disabled
                readOnly
              />
            </Field>

            {!isConnected && (
              <div className={styles.signIn}>
                Please sign in with your wallet to submit a report.
                <Button variant="text" size="sm" className={styles.signInButton} onClick={() => openSignIn()}>
                  Sign In
                </Button>
              </div>
            )}

            {confirmingSignIn && (
              <div className={`${styles.signIn} ${styles.signInNotice}`}>
                <span id="report-leave-evidence" role="status">Opening the sign-in page clears the attached files. You’ll need to attach them again after signing in.</span>
                <div className={styles.signInActions}>
                  <Button ref={cancelSignIn} aria-describedby="report-leave-evidence" variant="text" size="sm" className={styles.signInButton} onClick={keepEditing}>
                    Cancel
                  </Button>
                  <Button variant="secondary" size="sm" onClick={continueSignIn}>Continue to sign in</Button>
                </div>
              </div>
            )}
            {draftSaveFailed && (
              <p className={styles.error} role="alert">
                Your browser couldn’t save your draft. Copy your text and clear the form before signing in.
              </p>
            )}

            <Field number={2} label="Reported User Wallet" helper="This is the wallet address of the user you are reporting.">
              <InputWebCreatorTools
                {...named(2)}
                label="Reported User Wallet"
                labelHidden
                aria-required="true"
                error={shown(errors.reported)}
                placeholder="Write or paste an address here..."
                value={reportedAddress}
                onChange={(e) => setReportedAddress(e.target.value)}
                onBlur={() => isEthAddress(reportedAddress) && track("report_target_set", { has_reporter: isEthAddress(wallet) }, trackCtx)}
                disabled={reportedAddressLocked}
              />
            </Field>

            <Field
              number={3}
              label="Reason for Report"
              helper="Select the option that best describes the issue."
              error={shown(errors.reason)}
            >
              <WebSelect
                {...named(3, shown(errors.reason))}
                aria-required="true"
                aria-invalid={!!shown(errors.reason)}
                value={reason}
                onChange={(e) => {
                  const selected = e.currentTarget.value as ReportReason;
                  setReason(selected);
                  track("report_category_set", { reason: selected }, trackCtx);
                }}
              >
                <option value="" disabled>
                  Choose a reason
                </option>
                {REASONS.map(({ value, label }) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </WebSelect>
            </Field>

            <Field
              number={4}
              label="Description"
              helper="Describe what happened. Include when and where the incident occurred if possible."
              error={shown(errors.description)}
            >
              <div className={styles.inputGroup}>
                <WebTextarea
                  {...named(4, shown(errors.description))}
                  aria-required="true"
                  aria-invalid={!!shown(errors.description)}
                  rows={4}
                  placeholder="Write your description here..."
                  value={description}
                  onChange={(e) => setDescription(e.target.value.slice(0, REPORT_LIMITS.descriptionMax))}
                  onBlur={() => description.trim() && track("report_details_set", { description_len: description.trim().length }, trackCtx)}
                />
                <p className={styles.counter}>{`${description.length} / ${REPORT_LIMITS.descriptionMax}`}</p>
                <p className={styles.hint}>The more detail you provide, the easier it is for moderators to review your report.</p>
              </div>
            </Field>

            <Field
              number={5}
              label="Evidence"
              helper={`Upload up to ${REPORT_LIMITS.maxFiles} screenshots, videos or PDF files to support your report.`}
              error={shown(errors.evidence)}
            >
              <FileUpload
                files={evidence}
                invalid={!!shown(errors.evidence)}
                onChange={(files) => {
                  if (files.length > evidence.length) track("report_evidence_added", { file_count: files.length }, trackCtx);
                  if (files.length) setReattachEvidence(false);
                  setEvidence(files);
                }}
              />
              {reattachEvidence && <p className={styles.helper}>Add your evidence files again after signing in.</p>}
            </Field>

            <Field
              number={6}
              label="Additional Comments"
              optional
              helper="Provide any extra information that might help moderators investigate."
            >
              <div className={styles.inputGroup}>
                <WebTextarea
                  {...named(6)}
                  rows={3}
                  placeholder="Write your comments here..."
                  value={comments}
                  onChange={(e) => setComments(e.target.value.slice(0, REPORT_LIMITS.commentsMax))}
                />
                <p className={styles.counter}>{`${comments.length} / ${REPORT_LIMITS.commentsMax}`}</p>
              </div>
            </Field>

            <div className={styles.field}>
              <CheckboxLabel
                size="lg"
                label="I confirm this report is accurate."
                checked={confirmed}
                onCheckedChange={setConfirmed}
                aria-required="true"
                aria-invalid={!!shown(errors.confirm)}
                aria-describedby={shown(errors.confirm) ? "report-confirm-error" : undefined}
              />
              {shown(errors.confirm) && (
                <p id="report-confirm-error" className={styles.error}>
                  {errors.confirm}
                </p>
              )}
            </div>

            {failure && (
              <p className={styles.submitError} role="alert">
                {failure}
              </p>
            )}

            <Button size="md" className={styles.submit} onClick={submit} disabled={mismatch || !isConnected || submitting}>
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
    </SitesLayout>
  );
}
