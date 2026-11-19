import { useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router";
import { useEventsCopy } from "@dcl/one-ui/apps/sites/events/locale";
import { EventsLocaleProvider, useEventsFooterCopy } from "@/features/events/locale";
import LdSubmitHangoutView from "@ui/landings/pages/LdSubmitHangoutView";

import { fetchEventCategories, type EventCategory } from "@/data/lib/catalyst/places/events";
import { buildHangoutSubmit, deleteHangout, emptyDraft, eventToDraft, loadEditableHangout, uploadHangoutPoster, type EditableHangout } from "@/data/lib/catalyst/landings/submit-hangout";
import { loadHangoutOptions, type HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import { SitesChromeMaybe } from "@ui/web/frames/SitesChrome";
import type { SitesLocaleHandle } from "@ui/web/frames/sites-locale";
import { openSignIn } from "@/features/components/auth/signin-store";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import SubmitHangout, {
  type CategoryOption,
} from "@/features/social/submit-hangout/SubmitHangout";

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
    eventId: mode === "edit" ? params.id?.trim() || null : null,
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

export default function SubmitHangoutRoute(props: Route.ComponentProps) {
  return <EventsLocaleProvider><SubmitHangoutForm {...props} /></EventsLocaleProvider>;
}

function SubmitHangoutForm({ loaderData }: Route.ComponentProps) {
  const footerCopy = useEventsFooterCopy();
  const { text, errorText } = useEventsCopy();
  const d = loaderData as LoaderData;
  const auth = useAuth();
  const navigate = useNavigate();
  const [loaded, setLoaded] = useState<{ account: string; id: string; event?: EditableHangout; error?: string } | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [optionsRead, setOptionsRead] = useState<{ account: string; options?: HangoutOptions; error?: string } | null>(null);
  const [optionsAttempt, setOptionsAttempt] = useState(0);
  useEffect(() => {
    if (!auth.identity) return;
    const controller = new AbortController();
    const account = auth.identity.signer;
    loadHangoutOptions(auth.identity, controller.signal).then(
      (options) => { if (!controller.signal.aborted) setOptionsRead({ account, options }); },
      (error: unknown) => { if (!controller.signal.aborted) setOptionsRead({ account, error: error instanceof Error ? error.message : "Could not load authoring options." }); },
    );
    return () => controller.abort();
  }, [auth.identity, optionsAttempt]);
  const optionsResult = optionsRead?.account === auth.address ? optionsRead : null;
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
      (error: unknown) => { if (!controller.signal.aborted) setLoaded({ account, id, error: error instanceof Error ? error.message : "Could not load this hangout." }); },
    );
    return () => controller.abort();
  }, [d.mode, d.eventId, auth.identity, optionsAttempt]);
  const current = loaded?.account === auth.address && loaded?.id === d.eventId ? loaded : null;
  const original = current?.event;
  const owner = Boolean(original && original.user?.toLowerCase() === auth.address?.toLowerCase());
  const canEditAny = options?.profile?.permissions.includes("edit_any_event") ?? false;
  const accessPending = Boolean(original) && !owner && !optionsResult;
  const canEdit = d.mode === "create" || Boolean(original && (owner || canEditAny));
  const waiting = (d.mode === "edit" && Boolean(d.eventId) && auth.isConnected && !current) || accessPending;
  const accessError = original && !owner && optionsResult
    ? optionsResult.error || options?.profileError || (!canEditAny ? text("You don't have permission to edit this hangout.") : null)
    : null;
  const error = d.mode === "edit" && !d.eventId ? text("Choose a hangout to edit using its event link.") : current?.error || accessError;
  const canChangeCommunity = d.mode === "create" || owner;

  async function removeHangout() {
    const identity = getIdentity();
    if (!identity || !original || !canEdit || identity.signer.toLowerCase() !== auth.address?.toLowerCase() || !window.confirm(text("Delete {name}?", { name: original.name }))) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await deleteHangout(identity, original.id);
      navigate("/events");
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : "Could not delete this hangout.");
    } finally {
      setDeleting(false);
    }
  }

  return (
    <SitesChromeMaybe active="whatson" footerCopy={footerCopy}>
    <LdSubmitHangoutView>
      {waiting ? <p role="status">{text("Loading your hangout…")}</p> : error ? <div><p role="alert">{errorText(error)}</p><Link to="/events?tab=my">{text("Back to My Hangouts")}</Link><button type="button" onClick={() => { setLoaded(null); setOptionsRead(null); setOptionsAttempt((previous) => previous + 1); }}>{text("Check access again")}</button></div> : <SubmitHangout
        key={`${auth.address ?? "signed-out"}:${d.eventId ?? "new"}`}
        trackCtx={{
          sid: d.sid,
          story: STORY,
          variant: d.assignment.variant,
        }}
        categories={d.categories}
        categoriesError={d.categoriesError}
        mode={d.mode}
        draft={original ? eventToDraft(original) : { ...emptyDraft(), communityId: d.communityId }}
        submit={buildHangoutSubmit(getIdentity, original, () => optionsRef.current?.profile?.permissions.includes("edit_any_event") ?? false)}
        signedIn={auth.isConnected}
        onSignIn={openSignIn}
        options={options}
        canEditAny={canEditAny}
        privateFieldsEditable={d.mode === "create" || owner}
        canChangeCommunity={canChangeCommunity}
        upload={(file, vertical, signal) => {
          const identity = getIdentity();
          if (!identity) return Promise.reject(new Error(text("Sign in to upload an image.")));
          return uploadHangoutPoster(identity, file, vertical, signal);
        }}
      />}
      {original && canEdit && !waiting && !error && <button type="button" disabled={deleting} onClick={() => { void removeHangout(); }}>{deleting ? text("Deleting…") : text("Delete hangout")}</button>}
      {deleteError && <p role="alert">{errorText(deleteError)}</p>}
    </LdSubmitHangoutView>
    </SitesChromeMaybe>
  );
}
