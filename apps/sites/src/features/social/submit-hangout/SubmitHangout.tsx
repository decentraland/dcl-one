import {
  useEventsCopy,
  type EventsLocale,
  EventDetailModal,
  SitesButton as Button,
  Busy,
  Notice,
  type NoticeTone,
} from "@dcl/one-ui";
import { GeneralAdd, GeneralCamera, GeneralDelete, GeneralInfoButton } from "@dcl/one-ui";
import { useEffect, useRef, useState, type DragEvent } from "react";
import { useMachine } from "@xstate/react";
import { Link, useSearchParams } from "react-router";


import type { TrackContext } from "@/core/lib/telemetry/track";
import { isStepValid, toEventSchedule, type EventScheduleSource, type HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import { heroWhen, scheduleLine } from "../../components/event-schedule";
import styles from "./SubmitHangout.module.css";
import HangoutAssociations from "./HangoutAssociations";
import SubmitSuccess from "./SubmitSuccess";
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
  originalSchedule?: EventScheduleSource;
  initialStep?: string;
  submit?: SubmitFn;
  track?: TrackFn;
  signedIn?: boolean;
  onSignIn?: () => void;
  upload?: UploadFn;
  options?: HangoutOptions | null;
  privateFieldsEditable?: boolean;
  canChangeCommunity?: boolean;
  editor?: boolean;
  reviewNotice?: string;
  /** How the notice reads: a rejection is an error, a note on an approved event a warning. */
  reviewTone?: NoticeTone;
  /** Edit mode: opens the delete confirmation from the form's actions. */
  onDelete?: () => void;
};

export default function SubmitHangout({
  trackCtx,
  categories,
  categoriesError = false,
  mode = "create",
  draft,
  originalSchedule,
  initialStep,
  submit,
  track,
  signedIn = false,
  onSignIn,
  upload,
  options = null,
  privateFieldsEditable = mode === "create",
  canChangeCommunity = mode === "create",
  editor = false,
  reviewNotice,
  reviewTone = "warning",
  onDelete,
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
      reviewTone={reviewTone}
      trackCtx={trackCtx}
      categories={categories}
      categoriesError={categoriesError}
      mode={mode}
      draft={draft}
      originalSchedule={originalSchedule}
      submit={submit}
      track={track}
      signedIn={signedIn}
      onSignIn={onSignIn}
      upload={upload}
      options={options}
      privateFieldsEditable={privateFieldsEditable}
      canChangeCommunity={canChangeCommunity}
      onDelete={onDelete}
    />
  );
}

type InnerProps = {
  stateId: HangoutStateId;
  inline: boolean;
  reviewNotice?: string;
  /** How the notice reads: a rejection is an error, a note on an approved event a warning. */
  reviewTone?: NoticeTone;
  trackCtx: TrackContext;
  categories: CategoryOption[];
  categoriesError: boolean;
  mode: "create" | "edit";
  draft?: HangoutDraft;
  originalSchedule?: EventScheduleSource;
  submit?: SubmitFn;
  track?: TrackFn;
  signedIn: boolean;
  onSignIn?: () => void;
  upload?: UploadFn;
  options: HangoutOptions | null;
  privateFieldsEditable: boolean;
  canChangeCommunity: boolean;
  onDelete?: () => void;
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
  reviewTone = "warning",
  trackCtx,
  categories,
  categoriesError,
  mode,
  draft,
  originalSchedule,
  submit,
  track,
  signedIn,
  onSignIn,
  upload,
  options,
  privateFieldsEditable,
  canChangeCommunity,
  onDelete,
}: InnerProps) {
  const { locale, text, errorText } = useEventsCopy();
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
  const trackDirectEntry = useRef(inline && stateId === "review");
  useEffect(() => {
    if (!trackDirectEntry.current) return;
    trackDirectEntry.current = false;
    emitTrack(HANGOUT_EVENTS.started, {}, trackCtx);
  }, [emitTrack, trackCtx]);
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
    if (signedIn && state.matches("signinGate")) send({ type: "SIGN_IN", inline });
  }, [inline, signedIn, state, send]);

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

  const editingInline = inline && (value === "review" || value === "preview");
  const previewSchedule = inline && value === "preview"
    ? { ...toEventSchedule(d, originalSchedule), finish_at: null, next_start_at: null, next_finish_at: null }
    : null;
  const canSubmit = !uploading && communityAllowed &&
    FORM_ORDER.every((section) => isStepValid(section, d));
  const onEdit = (patch: Partial<HangoutDraft>) => send({ type: "EDIT", patch });
  const uploadFeedback = uploadError ? <Notice tone="error">{errorText(uploadError)}</Notice>
    : uploading ? <Busy>{text("Uploading image…")}</Busy> : null;

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
          inline={editingInline}
        />
        {section === "details" && !editingInline && (
          <HangoutAssociations draft={d} options={options} canChangeCommunity={canChangeCommunity} onEdit={onEdit} />
        )}
        {section === "details" && !communityAllowed && <Notice tone="error">{text("Choose a community you manage, or select None.")}</Notice>}
      </>
    );
  }

  return (
    <div className={inline ? `submit-hangout ${styles.authoring}` : "submit-hangout"} data-editor={inline || undefined} data-step={inline ? undefined : step} data-state={value}>
      {inline ? <EditorHeader mode={mode} /> : (
        <header className="submit-hangout__header">
          <Link to="/events">{text("Back to Events")}</Link>
          <h1>{text(mode === "edit" ? "Edit Hangout" : "Submit a Hangout")}</h1>
        </header>
      )}

      <div className={`submit-hangout__controls ${inline ? styles.editorControls : ""}`} role="group" aria-label={text(inline ? "Event editor" : "Hangout wizard")}>
        {(value === "signinGate" || needsSignIn) && (
          <button
            type="button"
            className="submit-hangout__btn submit-hangout__btn--primary"
            onClick={() => onSignIn ? onSignIn() : send({ type: "SIGN_IN", inline })}
          >
            {text("Sign In to continue")}
          </button>
        )}

        {isForm && (
          <>
            {editingInline ? (
              <div className={styles.editorColumns}>
                <div className={styles.coverPanel}>
                  <fieldset className={styles.editorSection}><legend>{text("Cover")}</legend>{fields("cover")}</fieldset>
                  {uploadFeedback}
                  <fieldset className={styles.editorSection}><legend>{text("Event details")}</legend>{fields("details")}</fieldset>
                </div>
                <div className={styles.schedulePanel}>
                  {reviewNotice && <Notice tone={reviewTone}>{reviewNotice}</Notice>}
                  <div className={styles.sections}>
                    <h2>{text("Event details")}</h2>
                    <fieldset className={styles.editorSection}><legend>{text("Schedule")}</legend>{fields("schedule")}</fieldset>
                    <fieldset className={styles.editorSection}><legend>{text("Location")}</legend>{fields("location")}</fieldset>
                    <HangoutAssociations draft={d} options={options} canChangeCommunity={canChangeCommunity} onEdit={onEdit} />
                  </div>
                  <div className={styles.editorActions}>
                    <div className={styles.reviewBar}>
                      <p><GeneralInfoButton size={16} aria-hidden="true" />{text("The event submission will be reviewed by our team, you'll be notified by email")}</p>
                      <Button variant="tertiary" disabled={!canSubmit} onClick={() => send({ type: "PREVIEW" })}>{text("Preview")}</Button>
                    </div>
                    <div className={styles.formActions}>
                      {mode === "edit" && onDelete && (
                        <button type="button" className={styles.delete} onClick={onDelete}>
                          {text("Delete")}<GeneralDelete size={20} aria-hidden="true" />
                        </button>
                      )}
                      <Link className={styles.cancel} to={mode === "edit" ? "/events?tab=my" : "/events"}>{text("Cancel")}</Link>
                      <Button disabled={!canSubmit} onClick={() => send({ type: "SUBMIT" })}>{text(mode === "edit" ? "Save changes" : "Submit Event")}</Button>
                    </div>
                  </div>
                </div>
              </div>
            ) : fields(value)}
            {previewSchedule && (
              <EventDetailModal
                title={d.name}
                image={d.imagePreviewUrl}
                live={false}
                when={heroWhen(previewSchedule, undefined, locale)}
                schedule={scheduleLine(previewSchedule, undefined, locale)}
                location={{ world: d.location === "world", label: d.location === "world" ? d.worldName : `${d.coordX},${d.coordY}` }}
                description={d.description}
                onClose={() => send({ type: "BACK" })}
              />
            )}
            {!editingInline && uploadFeedback}
            {!editingInline && <div className="submit-hangout__nav">
              {value === "cover" ? <Link className="submit-hangout__btn" to="/events">{text("Back")}</Link> : <button
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
                    onClick={() => send({ type: "PREVIEW" })}
                  >
                    {text("Preview")}
                  </button>
                  <button
                    type="button"
                    className="submit-hangout__btn submit-hangout__btn--primary"
                    onClick={() => send({ type: "SUBMIT" })}
                  >
                    {text("Submit Hangout")}
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
            </div>}
          </>
        )}

        {value === "submitting" && (
          <Busy>{text("Submitting your hangout for review…")}</Busy>
        )}
        {value === "submitted" && !needsSignIn && <SubmitSuccess mode={mode} approved={state.context.result?.approved ?? false} stillRejected={state.context.result?.rejected ?? false} />}

        {state.context.error && value === "review" && (
          <Notice tone="error">
            {text("Submit failed: {error}. Please try again.", { error: errorText(state.context.error) })}
          </Notice>
        )}
      </div>
    </div>
  );
}

function EditorHeader({ mode }: { mode: "create" | "edit" }) {
  const { text } = useEventsCopy();
  return (
    <header className={`submit-hangout__header ${styles.editorHeader}`}>
      <Link to="/events" className={styles.back} aria-label={text("Back to Events")} />
      <h1>{text(mode === "edit" ? "Edit Event" : "Submit an Event")}</h1>
    </header>
  );
}

/**
 * The editor's page while the event to edit is read, or when that read failed and can be retried:
 * the same header, so nothing jumps when the form arrives.
 */
export function SubmitHangoutLoading({ mode, error, onRetry }: { mode: "create" | "edit"; error?: string | null; onRetry?: () => void }) {
  const { text, errorText } = useEventsCopy();
  return (
    <div className={`submit-hangout ${styles.authoring}`} data-editor>
      <EditorHeader mode={mode} />
      {error ? (
        <div className={styles.loadState}>
          <Notice tone="error">{errorText(error)}</Notice>
          {onRetry && <Button variant="tertiary" onClick={onRetry}>{text("Retry")}</Button>}
        </div>
      ) : <Busy layout="page">{text("Loading your hangout…")}</Busy>}
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
  inline,
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
  inline: boolean;
}) {
  const { locale, text } = useEventsCopy();
  const [verticalCoverOpen, setVerticalCoverOpen] = useState(Boolean(draft.verticalImagePreviewUrl));
  if (step === "cover") {
    return (
      <div className={`submit-hangout__fields ${styles.fields}`}>
        {!inline && <p className="submit-hangout__hint">{text("Cover images are optional.")}</p>}
        <CoverField
          inline={inline}
          image={draft.imagePreviewUrl}
          uploading={uploading}
          onUpload={onUpload}
          onRemove={() => onEdit({ imagePreviewUrl: null })}
        />
        <details className={styles.verticalCover} open={verticalCoverOpen} onToggle={(event) => setVerticalCoverOpen(event.currentTarget.open)}>
          <summary>{inline ? <><GeneralAdd size={24} aria-hidden="true" /><strong>{text("Add Vertical Cover")}</strong> {text("(recommended)")}</> : text("Optional vertical cover")}</summary>
          <CoverField
            inline={inline}
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
      <div className={`submit-hangout__fields ${styles.fields} ${styles.descriptionFields}`}>
        <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
          <span>{text(inline ? "Event Name" : "Hangout Name")}</span>
          <input
            className={`submit-hangout__field ${styles.field}`}
            type="text"
            maxLength={150}
            value={draft.name}
            placeholder={text("Be as descriptive as you can")}
            onChange={(e) => onEdit({ name: e.target.value })}
          />
        </label>
        <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
          <span>{text(inline ? "Event Description" : "Description")}</span>
          <textarea
            className={`submit-hangout__field ${styles.field}`}
            rows={inline ? 3 : 2}
            placeholder={inline ? text("Be as descriptive as you can") : undefined}
            maxLength={5000}
            value={draft.description}
            onChange={(e) => onEdit({ description: e.target.value })}
          />
        </label>
        {categories.length > 0 ? (
          <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
            {text("Category")}
            <select
              className={`submit-hangout__field ${styles.field}`}
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
          <Notice tone={categoriesError ? "warning" : "info"}>
            {categoriesError
              ? text("Categories couldn't be loaded right now — you can continue and pick one later.")
              : text("No categories are available right now — you can continue without one.")}
          </Notice>
        )}
        {privateFieldsEditable && <>
          <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>{text("Contact email")}<input className={`submit-hangout__field ${styles.field}`} type="email" value={draft.contact} onChange={(event) => onEdit({ contact: event.target.value })} /></label>
          <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>{text("Details for moderators")}<textarea aria-label={text("Details for moderators")} className={`submit-hangout__field ${styles.field}`} value={draft.details} maxLength={5000} onChange={(event) => onEdit({ details: event.target.value })} /></label>
        </>}
      </div>
    );
  }

  if (step === "location") {
    const isWorld = draft.location === "world";
    return (
      <div className={`submit-hangout__fields ${styles.fields}`}>
        <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
          {text("Location Type")}
          <select
            className={`submit-hangout__field ${styles.field}`}
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
          <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
            {text("World")}
            <input
              className={`submit-hangout__field ${styles.field}`}
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
          <div className={`submit-hangout__coords ${styles.coords}`}>
            <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
              X
              <input
                className={`submit-hangout__field ${styles.field}`}
                type="number"
                value={draft.coordX}
                onChange={(e) => onEdit({ coordX: Number(e.target.value) })}
              />
            </label>
            <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
              Y
              <input
                className={`submit-hangout__field ${styles.field}`}
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
      <div className={`submit-hangout__fields ${styles.fields} ${styles.scheduleFields}`}>
        <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
          {text("Date")}
          <input
            className={`submit-hangout__field ${styles.field}`}
            type="date"
            value={draft.startDate}
            onChange={(e) => onEdit({ startDate: e.target.value })}
          />
        </label>
        <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
          {text("Start (UTC)")}
          <input
            className={`submit-hangout__field ${styles.field}`}
            type="time"
            value={draft.startTime}
            onChange={(e) => onEdit({ startTime: e.target.value })}
          />
        </label>
        <label className={`submit-hangout__fieldcheck ${styles.durationField}`}>
          {text("Duration (hours)")}
          <input className={`submit-hangout__field ${styles.field}`} type="number" min="0.25" max="24" step="0.25" value={draft.durationHours} onChange={(event) => onEdit({ durationHours: Number(event.target.value) })} />
        </label>
        <label className={`submit-hangout__fieldcheck ${styles.repeatField}`}>
          <input
            type="checkbox"
            checked={draft.recurrent}
            onChange={(e) => onEdit({ recurrent: e.target.checked })}
          />
          {text("Repeat Hangout")}
        </label>
        {draft.recurrent && (
          <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>{text("Repeat")}<select className={`submit-hangout__field ${styles.field}`} value={draft.recurrence} onChange={(event) => onEdit({ recurrence: event.target.value })}><option value="every_hour">{text("Every hour")}</option><option value="every_day">{text("Every day")}</option><option value="every_week">{text("Every week")}</option><option value="every_month">{text("Every month")}</option><option value="every_year">{text("Every year")}</option></select></label>
        )}
        {draft.recurrent && (
          <label className={`submit-hangout__fieldlabel ${styles.fieldLabel}`}>
            {text("Ends")}
            <input
              className={`submit-hangout__field ${styles.field}`}
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
  inline,
  vertical = false,
  image,
  uploading,
  onUpload,
  onRemove,
}: {
  inline: boolean;
  vertical?: boolean;
  image: string | null;
  uploading: boolean;
  onUpload?: (file: File, vertical: boolean) => void;
  onRemove: () => void;
}) {
  const { text } = useEventsCopy();
  const uploadLabel = text(vertical ? "Upload vertical cover" : "Upload cover image");
  function dropCover(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    const file = event.dataTransfer.files[0];
    if (!uploading && file) onUpload?.(file, vertical);
  }
  return (
    <div className={styles.coverField}>
      {image && (
        <figure className={styles.cover} onDragOver={(event) => event.preventDefault()} onDrop={dropCover}>
          <img className={styles.coverImage} src={image} alt={text(vertical ? "Vertical hangout cover" : "Hangout cover")} width={vertical ? 120 : 240} height={vertical ? 180 : 120} />
          <button type="button" className="submit-hangout__btn" disabled={uploading} onClick={onRemove}>
            {text(vertical ? "Remove vertical cover" : "Remove cover image")}
          </button>
        </figure>
      )}
      {onUpload && (!inline || !image) && (
        <label
          className={styles.dropZone}
          onDragOver={(event) => event.preventDefault()}
          onDrop={dropCover}
        >
          {inline && <GeneralCamera className={styles.camera} size={64} aria-hidden="true" />}
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
