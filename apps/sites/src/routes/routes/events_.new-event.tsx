import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";
import { useEventsCopy, Notice, SubmitHangoutView as LdSubmitHangoutView, type SitesLocaleHandle } from "@dcl/one-ui/apps/sites";
import { rejectedNotice } from "@/features/social/submit-hangout/review-notice";
import { EventsLocaleProvider, useEventsFooterCopy } from "@/features/events/locale";

import { fetchEventCategories, type EventCategory } from "@/data/lib/catalyst/places/events";
import { invalidateEventsAfterWrite } from "@/data/lib/catalyst/places/events-page";
import { ownedEventsKeys } from "@/data/lib/catalyst/places/event-management";
import { buildHangoutSubmit, deleteHangout, emptyDraft, eventToDraft, loadEditableHangout, uploadHangoutPoster, type EditableHangout } from "@/data/lib/catalyst/landings/submit-hangout";
import { loadHangoutOptions, type HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import { useAuth } from "@/data/lib/auth/context";
import { CatalystError } from "@/data/lib/catalyst/client";
import { getIdentity } from "@/data/lib/auth/session";
import { SitesLayoutMaybe } from "@/features/components/chrome/SitesLayout";
import { openSignIn } from "@/features/components/auth/signin-store";
import { useSignInRequired } from "@/features/components/auth/useSignInRequired";
import { parseUuidParam } from "@/data/lib/creators/ids";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import SubmitHangout, {
  SubmitHangoutLoading,
  type CategoryOption,
} from "@/features/social/submit-hangout/SubmitHangout";
import DeleteEventDialog, { type DeleteEventError } from "@/features/social/submit-hangout/DeleteEventDialog";
import { useHangoutSignInDraft } from "@/features/social/submit-hangout/sign-in-draft";

import type { Route } from "./+types/events_.new-event";
import type { StoryId } from "@/core/lib/telemetry/story-id";

export const handle = { sitesLocalized: true } satisfies SitesLocaleHandle;

const STORY: StoryId = "landings/submit-hangout";

function toOption(c: EventCategory): CategoryOption {
  return { name: c.name, label: c.i18n?.en ?? c.name, i18n: c.i18n };
}

const FALLBACK: PageDefaults = {
  variant: "wizard",
};

// /events/new-event creates a hangout; /events/edit-event/:id serves this module with the event id.
export async function loader({ request, context, params }: Route.LoaderArgs & { params: { id?: string } }) {
  const url = new URL(request.url);
  const mode = params.id === undefined ? "create" : "edit";

  const { sid, assignment, wrap } = await pageLoader(
    request,
    context,
    FALLBACK,
  );

  let categoriesError = false;
  const categories: CategoryOption[] = await fetchEventCategories({
    signal: request.signal,
  })
    .then((cats) => cats.map(toOption))
    .catch(() => {
      categoriesError = true;
      return [];
    });

  const payload = {
    sid,
    mode,
    categories,
    categoriesError,
    // Event ids are UUIDs and the API refuses anything else, so a malformed id is no event at all.
    eventId: mode === "edit" ? parseUuidParam(params.id?.trim()) ?? null : null,
    communityId: mode === "create" ? url.searchParams.get("community_id")?.trim() || "" : "",
    assignment,
  };

  return wrap(payload);
}

type LoaderData = {
  sid: string;
  mode: "create" | "edit";
  categories: CategoryOption[];
  categoriesError: boolean;
  eventId: string | null;
  communityId: string;
  assignment: PageDefaults;
};

export function headers() {
  return { "Cache-Control": "private, no-cache" };
}

const noSubscription = () => () => {};
// Statuses that answer "this account cannot edit this event" (400: the API refused the id itself);
// anything else may succeed on a retry.
const DENIED_STATUSES = new Set([400, 401, 403, 404]);
// Layout effects warn during server rendering; the editor only scrolls in the browser.
const useBrowserLayoutEffect = typeof window === "undefined" ? useEffect : useLayoutEffect;

export default function SubmitHangoutRoute(props: Route.ComponentProps) {
  return <EventsLocaleProvider><SubmitHangoutForm {...props} /></EventsLocaleProvider>;
}

function SubmitHangoutForm({ loaderData }: Route.ComponentProps) {
  const footerCopy = useEventsFooterCopy();
  const { text, locale } = useEventsCopy();
  const queryClient = useQueryClient();
  const d = loaderData as LoaderData;
  const auth = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const [params] = useSearchParams();
  // The server and the hydrating render have no identity; only a browser render can tell signed out.
  const hydrated = useSyncExternalStore(noSubscription, () => true, () => false);
  const inline = !params.get("step")?.trim();
  const [loaded, setLoaded] = useState<{ account: string; id: string; event?: EditableHangout; error?: string; status?: number } | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleteError, setDeleteError] = useState<DeleteEventError | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [optionsRead, setOptionsRead] = useState<{ account: string; options?: HangoutOptions; error?: string } | null>(null);
  // The account the editor was opened for. A sign-out or an expired session keeps the editor, its draft
  // and the event read for that account behind the editor's own sign-in gate, until another account signs in.
  const [editorAccount, setEditorAccount] = useState<string | null>(null);
  useEffect(() => {
    if (auth.address) setEditorAccount(auth.address.toLowerCase());
  }, [auth.address]);
  const account = auth.address?.toLowerCase() ?? editorAccount;
  const held = !auth.isConnected && editorAccount !== null;
  const returnTo = location.pathname + location.search + location.hash;
  const mounted = useRef(true);
  useEffect(() => {
    // StrictMode runs this setup again after a cleanup, so the flag is restored here, not only at creation.
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  useEffect(() => {
    if (!auth.identity) return;
    const controller = new AbortController();
    const account = auth.identity.signer;
    loadHangoutOptions(auth.identity, controller.signal).then(
      (options) => { if (!controller.signal.aborted) setOptionsRead({ account, options }); },
      (error: unknown) => { if (!controller.signal.aborted) setOptionsRead({ account, error: error instanceof Error ? error.message : "Could not load authoring options." }); },
    );
    return () => controller.abort();
  }, [auth.identity, attempt]);
  const optionsResult = optionsRead?.account.toLowerCase() === account ? optionsRead : null;
  const options = optionsResult?.options ?? null;
  const optionsRef = useRef(options);
  optionsRef.current = options;
  useEffect(() => {
    if (d.mode !== "edit" || !d.eventId || !auth.identity) return;
    const controller = new AbortController();
    const account = auth.identity.signer;
    const id = d.eventId;
    loadEditableHangout(auth.identity, id, controller.signal).then(
      (event) => { if (!controller.signal.aborted) setLoaded({ account, id, event }); },
      (error: unknown) => { if (!controller.signal.aborted) setLoaded({ account, id, error: error instanceof Error ? error.message : "Could not load this hangout.", status: error instanceof CatalystError ? error.status : undefined }); },
    );
    return () => controller.abort();
  }, [d.mode, d.eventId, auth.identity, attempt]);
  const current = loaded?.account.toLowerCase() === account && loaded?.id === d.eventId ? loaded : null;
  const original = current?.event;
  const baseDraft = original ? eventToDraft(original) : { ...emptyDraft(), communityId: d.communityId };
  const signInDraft = useHangoutSignInDraft(returnTo, account, baseDraft, held);
  const owner = Boolean(original && account && original.user?.toLowerCase() === account);
  const canEditAny = options?.profile?.permissions.includes("edit_any_event") ?? false;
  const canEdit = d.mode === "create" || Boolean(original && (owner || canEditAny));
  const canChangeCommunity = d.mode === "create" || owner;
  // Another creator's event needs the account's permissions; a failed read of them is not an answer.
  const permissionsError = original && !owner && optionsResult ? optionsResult.error || options?.profileError || null : null;
  // A signed-out visitor goes straight to sign-in and comes back to this same editor: creating needs an
  // account, and an edit link (a rejection email, say) names an event only its owner or a moderator edits.
  const signedOut = !auth.isConnected && (d.mode === "create" || Boolean(d.eventId));
  useSignInRequired(signedOut && hydrated && !held, auth.isConnected);
  // As decentraland.org: an edit link that is missing its event, refused or not found, or definitely not
  // this account's to edit goes back to the Events page instead of showing an error.
  const editDenied = d.mode === "edit" && (
    !d.eventId
    || Boolean(current?.error && DENIED_STATUSES.has(current.status ?? 0))
    || Boolean(original && !owner && optionsResult && !permissionsError && !canEditAny)
  );
  // A failure that says nothing about access (timeout, server error) stays on the page with a retry.
  const editFailed = d.mode === "edit" && !editDenied ? (current?.error ?? permissionsError) : null;
  const editReady = d.mode === "edit" && Boolean(original) && canEdit && !editDenied && !editFailed;
  useEffect(() => {
    if (editDenied) void navigate("/events", { replace: true });
  }, [editDenied, navigate]);
  // The editor replaces a short loading page. A scroll offset kept or restored for that short page
  // (history restoration, a navigation that kept the previous page's offset) would otherwise land
  // the tall form part-way down, so the editor always opens at its top unless the URL names a target.
  useBrowserLayoutEffect(() => {
    if (editReady && !location.hash) window.scrollTo(0, 0);
  }, [editReady, d.eventId, location.hash]);

  async function removeHangout() {
    if (!original || !canEdit) return;
    const identity = getIdentity();
    // The session ended or another account signed in elsewhere: deleting with it would act for someone else.
    if (!identity || !auth.isConnected || identity.signer.toLowerCase() !== account) {
      setDeleteError("session");
      return;
    }
    setDeleting(true);
    setDeleteError(null);
    try {
      await deleteHangout(identity, original.id);
    } catch {
      setDeleteError("failed");
      setDeleting(false);
      return;
    }
    // My Events opens on a fresh read, never on a cached list that still holds the deleted event: it is
    // not mounted here, so an invalidation alone would only mark it stale.
    if (account) queryClient.removeQueries({ queryKey: ownedEventsKeys.mine(account) });
    await invalidateEventsAfterWrite(queryClient);
    if (!mounted.current) return;
    setDeleting(false);
    setConfirmingDelete(false);
    void navigate("/events?tab=my");
  }

  const submit = buildHangoutSubmit(getIdentity, original, () => optionsRef.current?.profile?.permissions.includes("edit_any_event") ?? false);

  return (
    <SitesLayoutMaybe active="whatson" headerSurface={inline ? "overlay" : "solid"} overlayNav={inline} footerCopy={footerCopy}>
    <LdSubmitHangoutView>
      {signInDraft.saveFailed && <Notice tone="error">{text("Your browser couldn't keep your draft for sign-in. Copy what you wrote, then choose Sign In again to continue without it.")}</Notice>}
      {/* Signed out (and on the server, which renders anonymously) the page only waits: sign-in opens on its own. */}
      {(signedOut && !held) || (d.mode === "edit" && !editReady) ? <SubmitHangoutLoading
        mode={d.mode}
        error={signedOut ? null : editFailed}
        onRetry={() => { setLoaded(null); setOptionsRead(null); setAttempt((previous) => previous + 1); }}
      /> : <SubmitHangout
        key={`${account ?? "signed-out"}:${d.eventId ?? "new"}:${signInDraft.restored ? "restored" : "fresh"}`}
        trackCtx={{
          sid: d.sid,
          story: STORY,
          variant: d.assignment.variant,
        }}
        editor
        reviewNotice={original?.rejected
          ? rejectedNotice(original.rejection_reason, locale)
          : original?.approved
            ? text("Changes to approved events may require another review.")
            : undefined}
        reviewTone={original?.rejected ? "error" : "warning"}
        categories={d.categories}
        categoriesError={d.categoriesError}
        mode={d.mode}
        draft={signInDraft.restored ?? baseDraft}
        onDraftChange={signInDraft.remember}
        originalSchedule={original}
        submit={async (input) => {
          const result = await submit(input);
          signInDraft.forget();
          void invalidateEventsAfterWrite(queryClient);
          return result;
        }}
        signedIn={auth.isConnected}
        onSignIn={openSignIn}
        options={options}
        privateFieldsEditable={d.mode === "create" || owner}
        canChangeCommunity={canChangeCommunity}
        onDelete={editReady ? () => { setDeleteError(null); setConfirmingDelete(true); } : undefined}
        upload={(file, vertical, signal) => {
          const identity = getIdentity();
          if (!identity) return Promise.reject(new Error(text("Sign in to upload an image.")));
          return uploadHangoutPoster(identity, file, vertical, signal);
        }}
      />}
      {confirmingDelete && editReady && (
        <DeleteEventDialog
          busy={deleting}
          error={deleteError}
          onCancel={() => setConfirmingDelete(false)}
          onConfirm={() => { void removeHangout(); }}
        />
      )}
    </LdSubmitHangoutView>
    </SitesLayoutMaybe>
  );
}
