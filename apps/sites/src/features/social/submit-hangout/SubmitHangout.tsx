import { useEventsCopy, type EventsLocale } from "@dcl/one-ui/apps/sites/events/locale";
import { useEffect, useRef, useState } from "react";
import { useMachine } from "@xstate/react";
import { Link, useSearchParams } from "react-router";


import type { TrackContext } from "@/core/lib/telemetry/track";
import { isStepValid, type HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import styles from "./SubmitHangout.module.css";
import HangoutAssociations from "./HangoutAssociations";
import {
  hangoutMachine,
  HANGOUT_EVENTS,
  emitStepCompleted,
  resolveHangoutSnapshot,
  slugToState,
  stateToSlug,
  FORM_ORDER,
  type HangoutStateId,
  type SubmitFn,
  type TrackFn,
} from "./machine";

export type CategoryOption = { name: string; label: string; i18n?: Partial<Record<EventsLocale, string | null>> };
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
  options?: HangoutOptions | null;
  canEditAny?: boolean;
  privateFieldsEditable?: boolean;
  canChangeCommunity?: boolean;
  editor?: boolean;
  reviewNotice?: string;
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
  options = null,
  canEditAny = false,
  privateFieldsEditable = mode === "create",
  canChangeCommunity = mode === "create",
  editor = false,
  reviewNotice,
}: SubmitHangoutProps) {
  const [searchParams] = useSearchParams();

  const urlStep = (searchParams.get("step")?.trim() || initialStep) ?? undefined;
  const inline = editor && !urlStep;
  const stateId = inline && signedIn ? "review" : slugToState(urlStep);

  return (
    <SubmitHangoutInner
      stateId={stateId}
      inline={inline}
      reviewNotice={reviewNotice}
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
      options={options}
      canEditAny={canEditAny}
      privateFieldsEditable={privateFieldsEditable}
      canChangeCommunity={canChangeCommunity}
    />
  );
}

type InnerProps = {
  stateId: HangoutStateId;
  inline: boolean;
  reviewNotice?: string;
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
  options: HangoutOptions | null;
  canEditAny: boolean;
  privateFieldsEditable: boolean;
  canChangeCommunity: boolean;
};

function uiStateFor(value: HangoutStateId): "signin" | "success" | "form" {
  if (value === "signinGate") return "signin";
  if (value === "submitted") return "success";
  return "form";
}

function SubmitHangoutInner({
  stateId,
  inline,
  reviewNotice,
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
  options,
  canEditAny,
  privateFieldsEditable,
  canChangeCommunity,
}: InnerProps) {
  const { text, errorText } = useEventsCopy();
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
  const startedEditor = useRef(false);
  useEffect(() => {
    if (!inline || !signedIn || startedEditor.current) return;
    startedEditor.current = true;
    emitTrack(HANGOUT_EVENTS.started, {}, trackCtx);
  }, [inline, signedIn, emitTrack, trackCtx]);
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
    if (inline || lastStep.current === step) return;
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
  }, [inline, step, setSearchParams]);

  const isForm = !needsSignIn && uiStateFor(value) === "form";
  const formStepIndex = FORM_ORDER.indexOf(value);
  const communityAllowed = !options || Boolean(options.communitiesError) || !d.communityId || (mode === "edit" && d.communityId === draft?.communityId) || Boolean(options?.communities.some((community) => community.id === d.communityId));
  const canAdvanceForm = !uploading && (value === "cover" || (isStepValid(value, d) && (value !== "details" || communityAllowed)));

  const editingInline = inline && value === "review";
  const canSubmit = !uploading && communityAllowed &&
    FORM_ORDER.every((section) => isStepValid(section, d));
  const onEdit = (patch: Partial<HangoutDraft>) => send({ type: "EDIT", patch });

  function fields(section: HangoutStateId) {
    return (
      <>
        <StepFields
          step={section}
          draft={d}
          categories={categories}
          categoriesError={categoriesError}
          onEdit={onEdit}
          onUpload={upload ? (file, vertical) => { void uploadImage(file, vertical); } : undefined}
          uploading={uploading}
          privateFieldsEditable={privateFieldsEditable}
          options={options}
        />
        {section === "details" && (
          <HangoutAssociations draft={d} options={options} canEditAny={canEditAny} canChangeCommunity={canChangeCommunity} onEdit={onEdit} />
        )}
        {section === "details" && !communityAllowed && <p role="alert">{text("Choose a community you manage, or select None.")}</p>}
      </>
    );
  }

  return (
    <div className="submit-hangout" data-editor={inline || undefined} data-step={inline ? undefined : step} data-state={value}>
      <header className={`submit-hangout__header ${inline ? styles.editorHeader : ""}`}>
        <Link to="/events">{text("Back to Events")}</Link>
        <h1>{mode === "edit" ? text("Edit Hangout") : text("Submit a Hangout")}</h1>
      </header>

      <div className={`submit-hangout__controls ${inline ? styles.editorControls : ""}`} role="group" aria-label={text(inline ? "Event editor" : "Hangout wizard")}>
        {(value === "signinGate" || needsSignIn) && (
          <button
            type="button"
            className="submit-hangout__btn submit-hangout__btn--primary"
            onClick={() => onSignIn ? onSignIn() : send({ type: "SIGN_IN" })}
          >
            {text("Sign In to continue")}
          </button>
        )}

        {isForm && (
          <>
            {reviewNotice && <p className={styles.reviewNotice}>{reviewNotice}</p>}
            {editingInline ? (
              <div className={styles.editorColumns}>
                <div>
                  <fieldset className={styles.editorSection}><legend>{text("Cover")}</legend>{fields("cover")}</fieldset>
                  <fieldset className={styles.editorSection}><legend>{text("Event details")}</legend>{fields("details")}</fieldset>
                </div>
                <div>
                  <fieldset className={styles.editorSection}><legend>{text("Location")}</legend>{fields("location")}</fieldset>
                  <fieldset className={styles.editorSection}><legend>{text("Schedule")}</legend>{fields("schedule")}</fieldset>
                </div>
              </div>
            ) : (
              <>
                {inline && value === "preview" && d.imagePreviewUrl && (
                  <img className={styles.previewCover} src={d.imagePreviewUrl} alt={text("Hangout cover")} />
                )}
                {fields(value)}
                {inline && value === "preview" && d.description && <p>{d.description}</p>}
              </>
            )}
            {uploadError && <p role="alert">{errorText(uploadError)}</p>}
            {uploading && <p role="status">{text("Uploading image…")}</p>}
            {editingInline && !canSubmit && !uploading && <p className="submit-hangout__hint">{text("Complete the hangout details, location and schedule before submitting.")}</p>}
            <div className="submit-hangout__nav">
              {editingInline ? <Link className="submit-hangout__btn" to={mode === "edit" ? "/events?tab=my" : "/events"}>{text("Cancel")}</Link>
                : value === "cover" ? <Link className="submit-hangout__btn" to="/events">{text("Back")}</Link> : <button
                type="button"
                className="submit-hangout__btn"
                onClick={() => send({ type: "BACK" })}
              >
                {text("Back")}
              </button>}

              {value === "review" ? (
                <>
                  <button
                    type="button"
                    className="submit-hangout__btn"
                    disabled={editingInline && !canSubmit}
                    onClick={() => send({ type: "PREVIEW" })}
                  >
                    {text("Preview")}
                  </button>
                  <button
                    type="button"
                    className="submit-hangout__btn submit-hangout__btn--primary"
                    disabled={editingInline && !canSubmit}
                    onClick={() => send({ type: "SUBMIT" })}
                  >
                    {text(mode === "edit" && inline ? "Save changes" : "Submit Hangout")}
                  </button>
                </>
              ) : value === "preview" ? (
                <button
                  type="button"
                  className="submit-hangout__btn submit-hangout__btn--primary"
                  onClick={() => send({ type: "SUBMIT" })}
                >
                  {text("Confirm & Submit")}
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
            {text("Submitting your hangout for review…")}
          </p>
        )}
        {value === "submitted" && !needsSignIn && (
          <div role="status">
            <p>{mode === "edit" ? text("Your hangout was updated.") : text("Your hangout was created.")}{state.context.result?.approved ? "" : ` ${text("It is pending review.")}`}</p>
            {state.context.result?.id && <Link to={`/events/edit-event/${encodeURIComponent(state.context.result.id)}`}>{text("Edit hangout")}</Link>}
          </div>
        )}

        {state.context.error && value === "review" && (
          <p className="submit-hangout__error" role="alert">
            {text("Submit failed: {error}. Please try again.", { error: errorText(state.context.error) })}
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
  const { text } = useEventsCopy();
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
      {text("Continue")}
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
  privateFieldsEditable,
  options,
}: {
  step: HangoutStateId;
  draft: HangoutDraft;
  categories: CategoryOption[];
  categoriesError: boolean;
  onEdit: (patch: Partial<HangoutDraft>) => void;
  onUpload?: (file: File, vertical: boolean) => void;
  uploading: boolean;
  privateFieldsEditable: boolean;
  options: HangoutOptions | null;
}) {
  const { locale, text } = useEventsCopy();
  if (step === "cover") {
    return (
      <div className="submit-hangout__fields">
        <p className="submit-hangout__hint">{text("Cover images are optional.")}</p>
        <CoverField
          image={draft.imagePreviewUrl}
          uploading={uploading}
          onUpload={onUpload}
          onRemove={() => onEdit({ imagePreviewUrl: null })}
        />
        <details className={styles.verticalCover} open={Boolean(draft.verticalImagePreviewUrl)}>
          <summary>{text("Optional vertical cover")}</summary>
          <CoverField
            vertical
            image={draft.verticalImagePreviewUrl}
            uploading={uploading}
            onUpload={onUpload}
            onRemove={() => onEdit({ verticalImagePreviewUrl: null })}
          />
        </details>
      </div>
    );
  }

  if (step === "details") {
    return (
      <div className="submit-hangout__fields">
        <label className="submit-hangout__fieldlabel">
          {text("Hangout Name")}
          <input
            className="submit-hangout__field"
            type="text"
            maxLength={150}
            value={draft.name}
            placeholder={text("Be as descriptive as you can")}
            onChange={(e) => onEdit({ name: e.target.value })}
          />
        </label>
        <label className="submit-hangout__fieldlabel">
          {text("Description")}
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
            {text("Category")}
            <select
              className="submit-hangout__field"
              value={draft.category}
              onChange={(e) => onEdit({ category: e.target.value })}
            >
              <option value="">{text("None")}</option>
              {categories.map((c) => (
                <option key={c.name} value={c.name}>
                  {c.i18n?.[locale] ?? c.label}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <p className="submit-hangout__hint" role="status">
            {categoriesError
              ? text("Categories couldn't be loaded right now — you can continue and pick one later.")
              : text("No categories are available right now — you can continue without one.")}
          </p>
        )}
        {privateFieldsEditable && <>
          <label className="submit-hangout__fieldlabel">{text("Contact email")}<input className="submit-hangout__field" type="email" value={draft.contact} onChange={(event) => onEdit({ contact: event.target.value })} /></label>
          <label className="submit-hangout__fieldlabel">{text("Details for moderators")}<textarea aria-label={text("Details for moderators")} className="submit-hangout__field" value={draft.details} maxLength={5000} onChange={(event) => onEdit({ details: event.target.value })} /></label>
        </>}
      </div>
    );
  }

  if (step === "location") {
    const isWorld = draft.location === "world";
    return (
      <div className="submit-hangout__fields">
        <label className="submit-hangout__fieldlabel">
          {text("Location Type")}
          <select
            className="submit-hangout__field"
            value={draft.location}
            onChange={(e) =>
              onEdit({ location: e.target.value === "world" ? "world" : "land" })
            }
          >
            <option value="land">{text("Land")}</option>
            <option value="world">{text("World")}</option>
          </select>
        </label>
        {isWorld ? (
          <label className="submit-hangout__fieldlabel">
            {text("World")}
            <input
              className="submit-hangout__field"
              type="text"
              list="hangout-worlds"
              value={draft.worldName}
              placeholder="my-world.dcl.eth"
              onChange={(e) => onEdit({ worldName: e.target.value })}
            />
            <datalist id="hangout-worlds">{options?.worlds.map((world) => <option value={world} key={world} />)}</datalist>
            {options?.worldsError && <span className="submit-hangout__hint">{text("World suggestions could not load. You can enter the world name.")}</span>}
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
          {text("Date")}
          <input
            className="submit-hangout__field"
            type="date"
            value={draft.startDate}
            onChange={(e) => onEdit({ startDate: e.target.value })}
          />
        </label>
        <label className="submit-hangout__fieldlabel">
          {text("Start (UTC)")}
          <input
            className="submit-hangout__field"
            type="time"
            value={draft.startTime}
            onChange={(e) => onEdit({ startTime: e.target.value })}
          />
        </label>
        <label className="submit-hangout__fieldcheck">
          {text("Duration (hours)")}
          <input className="submit-hangout__field" type="number" min="0.25" max="24" step="0.25" value={draft.durationHours} onChange={(event) => onEdit({ durationHours: Number(event.target.value) })} />
        </label>
        <label className="submit-hangout__fieldcheck">
          <input
            type="checkbox"
            checked={draft.recurrent}
            onChange={(e) => onEdit({ recurrent: e.target.checked })}
          />
          {text("Repeat Hangout")}
        </label>
        {draft.recurrent && (
          <label className="submit-hangout__fieldlabel">{text("Repeat")}<select className="submit-hangout__field" value={draft.recurrence} onChange={(event) => onEdit({ recurrence: event.target.value })}><option value="every_hour">{text("Every hour")}</option><option value="every_day">{text("Every day")}</option><option value="every_week">{text("Every week")}</option><option value="every_month">{text("Every month")}</option><option value="every_year">{text("Every year")}</option></select></label>
        )}
        {draft.recurrent && (
          <label className="submit-hangout__fieldlabel">
            {text("Ends")}
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
      <h3 className="submit-hangout__reviewtitle">{text("Review your hangout")}</h3>
      <dl className="submit-hangout__reviewlist">
        <dt>{text("Name")}</dt>
        <dd>{draft.name || "\u{2014}"}</dd>
        <dt>{text("When")}</dt>
        <dd>
          {draft.startDate || "\u{2014}"} {draft.startTime}
          {draft.recurrent ? ` · ${text("repeats until {date}", { date: draft.recurrentUntil || "\u{2014}" })}` : ""}
        </dd>
        <dt>{text("Where")}</dt>
        <dd>
          {draft.location === "world"
            ? draft.worldName || text("a world")
            : `LAND (${draft.coordX}, ${draft.coordY})`}
        </dd>
        {draft.category && (
          <>
            <dt>{text("Category")}</dt>
            <dd>{categories.find(c => c.name === draft.category)?.i18n?.[locale] ?? draft.category}</dd>
          </>
        )}
      </dl>
    </div>
  );
}

function CoverField({
  vertical = false,
  image,
  uploading,
  onUpload,
  onRemove,
}: {
  vertical?: boolean;
  image: string | null;
  uploading: boolean;
  onUpload?: (file: File, vertical: boolean) => void;
  onRemove: () => void;
}) {
  const { text } = useEventsCopy();
  const uploadLabel = text(vertical ? "Upload vertical cover" : "Upload cover image");
  return (
    <div className={styles.coverField}>
      {image && (
        <figure className={styles.cover}>
          <img className={styles.coverImage} src={image} alt={text(vertical ? "Vertical hangout cover" : "Hangout cover")} width={vertical ? 120 : 240} height={vertical ? 180 : 120} />
          <button type="button" className="submit-hangout__btn" disabled={uploading} onClick={onRemove}>
            {text(vertical ? "Remove vertical cover" : "Remove cover image")}
          </button>
        </figure>
      )}
      {onUpload && (
        <label
          className={styles.dropZone}
          onDragOver={(event) => event.preventDefault()}
          onDrop={(event) => {
            event.preventDefault();
            const file = event.dataTransfer.files[0];
            if (!uploading && file) onUpload(file, vertical);
          }}
        >
          <strong>{uploadLabel}</strong>
          <span>{text("Drop an image here or choose a file, up to 500 KiB.")}</span>
          <input
            type="file"
            aria-label={uploadLabel}
            accept={vertical ? "image/jpeg,image/png,image/webp" : "image/jpeg,image/png,image/webp,image/gif"}
            disabled={uploading}
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (file) onUpload(file, vertical);
            }}
          />
        </label>
      )}
    </div>
  );
}
