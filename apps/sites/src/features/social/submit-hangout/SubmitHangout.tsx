import { useEffect, useRef, useState } from "react";
import { useMachine } from "@xstate/react";
import { Link, useSearchParams } from "react-router";


import type { TrackContext } from "@/core/lib/telemetry/track";
import { isStepValid, type HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import {
  hangoutMachine,
  emitStepCompleted,
  resolveHangoutSnapshot,
  slugToState,
  stateToSlug,
  FORM_ORDER,
  type HangoutStateId,
  type SubmitFn,
  type TrackFn,
} from "./machine";

export type CategoryOption = { name: string; label: string };
type UploadFn = (file: File, vertical: boolean, signal?: AbortSignal) => Promise<string>;

type SubmitHangoutProps = {
  trackCtx: TrackContext;
  categories: CategoryOption[];
  categoriesError?: boolean;
  mode?: "create" | "edit";
  draft?: HangoutDraft;
  initialStep?: string;
  submit?: SubmitFn;
  track?: TrackFn;
  signedIn?: boolean;
  onSignIn?: () => void;
  upload?: UploadFn;
};

export default function SubmitHangout({
  trackCtx,
  categories,
  categoriesError = false,
  mode = "create",
  draft,
  initialStep,
  submit,
  track,
  signedIn = false,
  onSignIn,
  upload,
}: SubmitHangoutProps) {
  const [searchParams] = useSearchParams();

  const urlStep = (searchParams.get("step")?.trim() || initialStep) ?? undefined;
  const stateId = slugToState(urlStep);

  return (
    <SubmitHangoutInner
      stateId={stateId}
      trackCtx={trackCtx}
      categories={categories}
      categoriesError={categoriesError}
      mode={mode}
      draft={draft}
      submit={submit}
      track={track}
      signedIn={signedIn}
      onSignIn={onSignIn}
      upload={upload}
    />
  );
}

type InnerProps = {
  stateId: HangoutStateId;
  trackCtx: TrackContext;
  categories: CategoryOption[];
  categoriesError: boolean;
  mode: "create" | "edit";
  draft?: HangoutDraft;
  submit?: SubmitFn;
  track?: TrackFn;
  signedIn: boolean;
  onSignIn?: () => void;
  upload?: UploadFn;
};

function uiStateFor(value: HangoutStateId): "signin" | "success" | "form" {
  if (value === "signinGate") return "signin";
  if (value === "submitted") return "success";
  return "form";
}

function SubmitHangoutInner({
  stateId,
  trackCtx,
  categories,
  categoriesError,
  mode,
  draft,
  submit,
  track,
  signedIn,
  onSignIn,
  upload,
}: InnerProps) {
  const [, setSearchParams] = useSearchParams();

  const snapshot = useRef(
    resolveHangoutSnapshot({ step: stateId, trackCtx, draft, submit, track }),
  ).current;

  const [state, send] = useMachine(hangoutMachine, {
    input: { trackCtx, draft, submit, track },
    snapshot,
  });

  const value = state.value as HangoutStateId;
  const step = stateToSlug(value);
  const d = state.context.draft;
  const emitTrack: TrackFn = track ?? state.context.track;
  const needsSignIn = Boolean(onSignIn) && !signedIn;
  const [uploading, setUploading] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const uploadController = useRef<AbortController | null>(null);
  useEffect(() => () => uploadController.current?.abort(), []);
  async function uploadImage(file: File, vertical: boolean) {
    if (!upload) return;
    uploadController.current?.abort();
    const controller = new AbortController();
    uploadController.current = controller;
    setUploading(true);
    setUploadError(null);
    try {
      const url = await upload(file, vertical, controller.signal);
      if (!controller.signal.aborted) send({ type: "EDIT", patch: vertical ? { verticalImagePreviewUrl: url } : { imagePreviewUrl: url } });
    } catch (error) {
      if (!controller.signal.aborted) setUploadError(error instanceof Error ? error.message : "Could not upload the image.");
    } finally {
      if (!controller.signal.aborted) setUploading(false);
    }
  }
  useEffect(() => {
    if (signedIn && state.matches("signinGate")) send({ type: "SIGN_IN" });
  }, [signedIn, state, send]);

  const lastStep = useRef<string | null>(null);
  useEffect(() => {
    if (lastStep.current === step) return;
    lastStep.current = step;
    setSearchParams(
      (prev) => {
        const params = new URLSearchParams(prev);
        if (params.get("step") === step) return params;
        params.set("step", step);
        return params;
      },
      { replace: true, preventScrollReset: true },
    );
  }, [step, setSearchParams]);

  const isForm = !needsSignIn && uiStateFor(value) === "form";
  const formStepIndex = FORM_ORDER.indexOf(value);
  const canAdvanceForm = !uploading && (value === "cover" || isStepValid(value, d));

  return (
    <div className="submit-hangout" data-step={step} data-state={value}>
      <header className="submit-hangout__header">
        <Link to="/whats-on">Back to What&apos;s On</Link>
        <h1>{mode === "edit" ? "Edit Hangout" : "Submit a Hangout"}</h1>
      </header>

      <div className="submit-hangout__controls" role="group" aria-label="Hangout wizard">
        {(value === "signinGate" || needsSignIn) && (
          <button
            type="button"
            className="submit-hangout__btn submit-hangout__btn--primary"
            onClick={() => onSignIn ? onSignIn() : send({ type: "SIGN_IN" })}
          >
            Sign In to continue
          </button>
        )}

        {isForm && (
          <>
            <StepFields
              step={value}
              draft={d}
              categories={categories}
              categoriesError={categoriesError}
              onEdit={(patch) => send({ type: "EDIT", patch })}
              onUpload={upload ? (file, vertical) => { void uploadImage(file, vertical); } : undefined}
              uploading={uploading}
              editing={mode === "edit"}
            />
            {uploadError && <p role="alert">{uploadError}</p>}
            {uploading && <p role="status">Uploading image…</p>}
            <div className="submit-hangout__nav">
              {value === "cover" ? <Link className="submit-hangout__btn" to="/whats-on">Back</Link> : <button
                type="button"
                className="submit-hangout__btn"
                onClick={() => send({ type: "BACK" })}
              >
                Back
              </button>}

              {value === "review" ? (
                <>
                  <button
                    type="button"
                    className="submit-hangout__btn"
                    onClick={() => send({ type: "PREVIEW" })}
                  >
                    Preview
                  </button>
                  <button
                    type="button"
                    className="submit-hangout__btn submit-hangout__btn--primary"
                    onClick={() => send({ type: "SUBMIT" })}
                  >
                    Submit Hangout
                  </button>
                </>
              ) : value === "preview" ? (
                <button
                  type="button"
                  className="submit-hangout__btn submit-hangout__btn--primary"
                  onClick={() => send({ type: "SUBMIT" })}
                >
                  Confirm &amp; Submit
                </button>
              ) : (
                <ForwardButton
                  from={value}
                  enabled={canAdvanceForm}
                  send={send}
                  emitTrack={emitTrack}
                  trackCtx={state.context.trackCtx}
                  nextStep={FORM_ORDER[formStepIndex + 1]}
                />
              )}
            </div>
          </>
        )}

        {value === "submitting" && (
          <p className="submit-hangout__status" role="status">
            Submitting your hangout for review&#x2026;
          </p>
        )}
        {value === "submitted" && !needsSignIn && (
          <div role="status">
            <p>{mode === "edit" ? "Your hangout was updated." : "Your hangout was created."}{state.context.result?.approved ? "" : " It is pending review."}</p>
            {state.context.result?.id && <Link to={`/landings/submit-hangout?mode=edit&id=${encodeURIComponent(state.context.result.id)}`}>Edit hangout</Link>}
          </div>
        )}

        {state.context.error && value === "review" && (
          <p className="submit-hangout__error" role="alert">
            Submit failed: {state.context.error}. Please try again.
          </p>
        )}
      </div>
    </div>
  );
}

function ForwardButton({
  from,
  enabled,
  send,
  emitTrack,
  trackCtx,
  nextStep,
}: {
  from: HangoutStateId;
  enabled: boolean;
  send: (e: { type: "NEXT" }) => void;
  emitTrack: TrackFn;
  trackCtx: TrackContext;
  nextStep?: HangoutStateId;
}) {
  return (
    <button
      type="button"
      className="submit-hangout__btn submit-hangout__btn--primary"
      disabled={!enabled}
      onClick={() => {
        if (!enabled) return;
        send({ type: "NEXT" });
        if (nextStep) emitStepCompleted(emitTrack, trackCtx, from, nextStep);
      }}
    >
      Continue
    </button>
  );
}

function StepFields({
  step,
  draft,
  categories,
  categoriesError,
  onEdit,
  onUpload,
  uploading,
  editing,
}: {
  step: HangoutStateId;
  draft: HangoutDraft;
  categories: CategoryOption[];
  categoriesError: boolean;
  onEdit: (patch: Partial<HangoutDraft>) => void;
  onUpload?: (file: File, vertical: boolean) => void;
  uploading: boolean;
  editing: boolean;
}) {
  if (step === "cover") {
    return (
      <div className="submit-hangout__fields">
        <p className="submit-hangout__hint">Cover images are optional.</p>
        {onUpload && <>
          <label className="submit-hangout__fieldlabel">Upload cover image<input type="file" accept="image/jpeg,image/png,image/webp,image/gif" disabled={uploading} onChange={(event) => { const file = event.target.files?.[0]; if (file) onUpload(file, false); }} /></label>
          <label className="submit-hangout__fieldlabel">Upload vertical cover<input type="file" accept="image/jpeg,image/png,image/webp" disabled={uploading} onChange={(event) => { const file = event.target.files?.[0]; if (file) onUpload(file, true); }} /></label>
        </>}
        <label className="submit-hangout__fieldlabel">Cover image URL<input className="submit-hangout__field" type="url" value={draft.imagePreviewUrl ?? ""} onChange={(event) => onEdit({ imagePreviewUrl: event.target.value || null })} /></label>
        <label className="submit-hangout__fieldlabel">Vertical cover image URL<input className="submit-hangout__field" type="url" value={draft.verticalImagePreviewUrl ?? ""} onChange={(event) => onEdit({ verticalImagePreviewUrl: event.target.value || null })} /></label>
      </div>
    );
  }

  if (step === "details") {
    return (
      <div className="submit-hangout__fields">
        <label className="submit-hangout__fieldlabel">
          Hangout Name
          <input
            className="submit-hangout__field"
            type="text"
            maxLength={150}
            value={draft.name}
            placeholder="Be as descriptive as you can"
            onChange={(e) => onEdit({ name: e.target.value })}
          />
        </label>
        <label className="submit-hangout__fieldlabel">
          Description
          <textarea
            className="submit-hangout__field"
            rows={2}
            maxLength={5000}
            value={draft.description}
            onChange={(e) => onEdit({ description: e.target.value })}
          />
        </label>
        {categories.length > 0 ? (
          <label className="submit-hangout__fieldlabel">
            Category
            <select
              className="submit-hangout__field"
              value={draft.category}
              onChange={(e) => onEdit({ category: e.target.value })}
            >
              <option value="">None</option>
              {categories.map((c) => (
                <option key={c.name} value={c.name}>
                  {c.label}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <p className="submit-hangout__hint" role="status">
            {categoriesError
              ? "Categories couldn't be loaded right now \u{2014} you can continue and pick one later."
              : "No categories are available right now \u{2014} you can continue without one."}
          </p>
        )}
        {!editing && <>
          <label className="submit-hangout__fieldlabel">Contact email<input className="submit-hangout__field" type="email" value={draft.contact} onChange={(event) => onEdit({ contact: event.target.value })} /></label>
          <label className="submit-hangout__fieldlabel">Details for moderators<textarea className="submit-hangout__field" value={draft.details} maxLength={5000} onChange={(event) => onEdit({ details: event.target.value })} /></label>
        </>}
      </div>
    );
  }

  if (step === "location") {
    const isWorld = draft.location === "world";
    return (
      <div className="submit-hangout__fields">
        <label className="submit-hangout__fieldlabel">
          Location Type
          <select
            className="submit-hangout__field"
            value={draft.location}
            onChange={(e) =>
              onEdit({ location: e.target.value === "world" ? "world" : "land" })
            }
          >
            <option value="land">Land</option>
            <option value="world">World</option>
          </select>
        </label>
        {isWorld ? (
          <label className="submit-hangout__fieldlabel">
            World
            <input
              className="submit-hangout__field"
              type="text"
              value={draft.worldName}
              placeholder="my-world.dcl.eth"
              onChange={(e) => onEdit({ worldName: e.target.value })}
            />
          </label>
        ) : (
          <div className="submit-hangout__coords">
            <label className="submit-hangout__fieldlabel">
              X
              <input
                className="submit-hangout__field"
                type="number"
                value={draft.coordX}
                onChange={(e) => onEdit({ coordX: Number(e.target.value) })}
              />
            </label>
            <label className="submit-hangout__fieldlabel">
              Y
              <input
                className="submit-hangout__field"
                type="number"
                value={draft.coordY}
                onChange={(e) => onEdit({ coordY: Number(e.target.value) })}
              />
            </label>
          </div>
        )}
      </div>
    );
  }

  if (step === "schedule") {
    return (
      <div className="submit-hangout__fields">
        <label className="submit-hangout__fieldlabel">
          Date
          <input
            className="submit-hangout__field"
            type="date"
            value={draft.startDate}
            onChange={(e) => onEdit({ startDate: e.target.value })}
          />
        </label>
        <label className="submit-hangout__fieldlabel">
          Start (UTC)
          <input
            className="submit-hangout__field"
            type="time"
            value={draft.startTime}
            onChange={(e) => onEdit({ startTime: e.target.value })}
          />
        </label>
        <label className="submit-hangout__fieldcheck">
          Duration (hours)
          <input className="submit-hangout__field" type="number" min="0.25" max="24" step="0.25" value={draft.durationHours} onChange={(event) => onEdit({ durationHours: Number(event.target.value) })} />
        </label>
        <label className="submit-hangout__fieldcheck">
          <input
            type="checkbox"
            checked={draft.recurrent}
            onChange={(e) => onEdit({ recurrent: e.target.checked })}
          />
          Repeat Hangout
        </label>
        {draft.recurrent && (
          <label className="submit-hangout__fieldlabel">Repeat<select className="submit-hangout__field" value={draft.recurrence} onChange={(event) => onEdit({ recurrence: event.target.value })}><option value="every_hour">Every hour</option><option value="every_day">Every day</option><option value="every_week">Every week</option><option value="every_month">Every month</option><option value="every_year">Every year</option></select></label>
        )}
        {draft.recurrent && (
          <label className="submit-hangout__fieldlabel">
            Ends
            <input
              className="submit-hangout__field"
              type="date"
              value={draft.recurrentUntil}
              onChange={(e) => onEdit({ recurrentUntil: e.target.value, recurrentCount: null })}
            />
          </label>
        )}
      </div>
    );
  }

  return (
    <div className="submit-hangout__review">
      <h3 className="submit-hangout__reviewtitle">Review your hangout</h3>
      <dl className="submit-hangout__reviewlist">
        <dt>Name</dt>
        <dd>{draft.name || "\u{2014}"}</dd>
        <dt>When</dt>
        <dd>
          {draft.startDate || "\u{2014}"} {draft.startTime}
          {draft.recurrent ? ` \u{B7} repeats until ${draft.recurrentUntil || "\u{2014}"}` : ""}
        </dd>
        <dt>Where</dt>
        <dd>
          {draft.location === "world"
            ? draft.worldName || "a world"
            : `LAND (${draft.coordX}, ${draft.coordY})`}
        </dd>
        {draft.category && (
          <>
            <dt>Category</dt>
            <dd>{draft.category}</dd>
          </>
        )}
      </dl>
    </div>
  );
}
