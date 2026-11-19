import { z } from "zod";

import { catalystBase, postJSON } from "../client";
import { signedFetch } from "../../auth/signer";
import type { AuthIdentity } from "../../auth/types";

export const REPORT_REASONS = [
  "scam_phishing",
  "illegal_content",
  "harassment",
  "cheating",
  "impersonation",
] as const;
export type ReportReason = (typeof REPORT_REASONS)[number];

export const REPORT_LIMITS = {
  descriptionMax: 500,
  commentsMax: 500,
  maxFiles: 5,
  maxFileSizeMb: 5,
} as const;

/** The evidence types the node accepts; `report.limits.test.ts` holds this list and the limits to the node's. */
export const REPORT_EVIDENCE_TYPES: readonly string[] = ["image/png", "image/jpeg", "image/gif", "image/webp", "video/mp4", "video/webm", "application/pdf"];

export type EvidenceUpload = {
  id: string;
  name: string;
  size: number;
  blob?: Blob;
  contentType?: string;
};

export type ReportDraft = {
  playerAddress: string;
  reportedAddress: string;
  reason: ReportReason | "";
  description: string;
  evidence: EvidenceUpload[];
  additionalComments: string;
  confirmAccuracy: boolean;
};

export function isEthAddress(v: string | undefined | null): boolean {
  return /^0x[a-fA-F0-9]{40}$/.test((v ?? "").trim());
}

type ReportFieldErrors = {
  reportedAddress: string;
  reason: string;
  description: string;
  evidence: string;
  confirmAccuracy: string;
};

const E = {
  invalidAddress: "Please enter a valid wallet address",
  missingReason: "Please choose a reason",
  missingDescription: "Please include a description of your report",
  missingEvidence: "Please upload the evidence of your issue",
  mustConfirm: "You must confirm this information is accurate",
} as const;

/** The field error of a wallet address input: empty when the value is an address. */
export function validateAddress(address: string): string {
  return isEthAddress(address) ? "" : E.invalidAddress;
}

export function validateDraft(draft: ReportDraft): ReportFieldErrors {
  return {
    reportedAddress: validateAddress(draft.reportedAddress),
    reason: draft.reason ? "" : E.missingReason,
    description: draft.description.trim() ? "" : E.missingDescription,
    evidence: draft.evidence.length === 0 ? E.missingEvidence : "",
    confirmAccuracy: draft.confirmAccuracy ? "" : E.mustConfirm,
  };
}

export function invalidFields(errors: ReportFieldErrors): string[] {
  return Object.entries(errors)
    .filter(([, msg]) => msg !== "")
    .map(([field]) => field);
}

export type SubmitReportResult = {
  reportId: string;
  evidenceKeys: string[];
};

export type SubmitReportFn = (args: {
  draft: ReportDraft;
  signal?: AbortSignal;
}) => Promise<SubmitReportResult>;

// The node's report-user area (services/catalyst/comms/src/gatekeeper/reports.rs):
// each request is signed over the path it is sent to (the mount included);
// the upload paths the presign answers are relative to the mount.
const REPORT_USER_PREFIX = "/report-user";
const REPORT_PRESIGN_PATH = "/api/reports/players/presign";
const REPORT_CREATE_PATH = "/api/reports/players";

const NOT_CONNECTED_MESSAGE = "Connect your wallet to submit a report.";
const MISSING_BYTES_MESSAGE =
  "Re-attach your evidence files before submitting.";

const PresignSlotSchema = z.object({
  key: z.string().min(1),
  uploadPath: z.string().min(1),
});

const PresignResponseSchema = z.object({
  reportId: z.string().min(1),
  files: z.array(PresignSlotSchema),
});

const CreatedReportSchema = z.object({
  data: z.object({
    id: z.string().min(1),
    evidenceKeys: z.array(z.string()),
  }),
});

/** Where a submission stopped. Telemetry sends this code, never the message: the message can name a file. */
export type ReportFailureStep = "not_connected" | "invalid_draft" | "missing_bytes" | "upload_plan" | "upload" | "response";

export class ReportSubmitError extends Error {
  readonly step: ReportFailureStep;
  constructor(step: ReportFailureStep, message: string) {
    super(message);
    this.name = "ReportSubmitError";
    this.step = step;
  }
}

function contentTypeOf(file: EvidenceUpload): string {
  return file.contentType || file.blob?.type || "";
}

export function buildSubmitReport(
  identity: AuthIdentity | null,
): SubmitReportFn {
  return async ({ draft, signal }): Promise<SubmitReportResult> => {
    if (!identity) throw new ReportSubmitError("not_connected", NOT_CONNECTED_MESSAGE);

    const errors = validateDraft(draft);
    const invalid = invalidFields(errors);
    if (invalid.length > 0) {
      throw new ReportSubmitError("invalid_draft", errors[invalid[0] as keyof ReportFieldErrors]);
    }

    const uploads = draft.evidence.map((file) => {
      const blob = file.blob;
      const contentType = contentTypeOf(file);
      if (!blob || !contentType) throw new ReportSubmitError("missing_bytes", MISSING_BYTES_MESSAGE);
      return { name: file.name, blob, contentType };
    });

    const presignRaw = await postJSON<unknown>(
      `${REPORT_USER_PREFIX}${REPORT_PRESIGN_PATH}`,
      {
        files: uploads.map((u) => ({
          filename: u.name,
          contentType: u.contentType,
          fileSize: u.blob.size,
        })),
      },
      { identity, signal },
    );
    const presign = PresignResponseSchema.safeParse(presignRaw);
    if (!presign.success || presign.data.files.length !== uploads.length) {
      throw new ReportSubmitError("upload_plan", "Report service returned an unexpected upload plan.");
    }

    const base = catalystBase();
    for (const [index, slot] of presign.data.files.entries()) {
      const upload = uploads[index];
      const res = await signedFetch(
        identity,
        `${base}${REPORT_USER_PREFIX}${slot.uploadPath}`,
        {
          method: "PUT",
          metadata: {},
          headers: { "content-type": upload.contentType },
          body: upload.blob,
          signal,
        },
      );
      // Neither answer of the upload is read: drop its body, so the connection is free at once.
      await res.body?.cancel().catch(() => undefined);
      if (!res.ok) {
        throw new ReportSubmitError("upload", `Evidence upload failed for ${upload.name} (${res.status}).`);
      }
    }

    const comments = draft.additionalComments.trim();
    const createdRaw = await postJSON<unknown>(
      `${REPORT_USER_PREFIX}${REPORT_CREATE_PATH}`,
      {
        reportId: presign.data.reportId,
        reportedAddress: draft.reportedAddress.trim().toLowerCase(),
        reason: draft.reason,
        description: draft.description.trim(),
        additionalComments: comments || undefined,
        confirmAccuracy: draft.confirmAccuracy,
        evidenceKeys: presign.data.files.map((f) => f.key),
      },
      { identity, signal },
    );
    const created = CreatedReportSchema.safeParse(createdRaw);
    if (!created.success) {
      throw new ReportSubmitError("response", "Report service returned an unexpected response.");
    }

    return {
      reportId: created.data.data.id,
      evidenceKeys: created.data.data.evidenceKeys,
    };
  };
}
