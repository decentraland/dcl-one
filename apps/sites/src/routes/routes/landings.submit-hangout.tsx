import { useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router";
import LdSubmitHangoutView from "@ui/landings/pages/LdSubmitHangoutView";

import { fetchEventCategories, type EventCategory } from "@/data/lib/catalyst/places/events";
import { buildHangoutSubmit, emptyDraft, eventToDraft, loadEditableHangout, uploadHangoutPoster, type EditableHangout } from "@/data/lib/catalyst/landings/submit-hangout";
import { loadHangoutOptions, type HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import { postJSON } from "@/data/lib/catalyst/client";
import { openSignIn } from "@/features/components/auth/signin-store";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import SubmitHangout, {
  type CategoryOption,
} from "@/features/social/submit-hangout/SubmitHangout";

import type { Route } from "./+types/landings.submit-hangout";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/submit-hangout";

function toOption(c: EventCategory): CategoryOption {
  return { name: c.name, label: c.i18n?.en ?? c.name };
}

const FALLBACK: PageDefaults = {
  variant: "wizard",
};

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const mode = url.searchParams.get("mode") === "edit" ? "edit" : "create";

  const { sid, assignment, wrap } = await pageLoader(
    request,
    
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
    eventId: mode === "edit" ? url.searchParams.get("id")?.trim() || null : null,
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

export default function SubmitHangoutRoute({ loaderData }: Route.ComponentProps) {
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
  }, [d.mode, d.eventId, auth.identity]);
  const current = loaded?.account === auth.address && loaded?.id === d.eventId ? loaded : null;
  const original = current?.event;
  const owner = Boolean(original && original.user?.toLowerCase() === auth.address?.toLowerCase());
  const canEditAny = options?.profile?.permissions.includes("edit_any_event") ?? false;
  const accessPending = Boolean(original) && !owner && !optionsResult;
  const canEdit = d.mode === "create" || Boolean(original && (owner || canEditAny));
  const waiting = (d.mode === "edit" && Boolean(d.eventId) && auth.isConnected && !current) || accessPending;
  const accessError = original && !owner && optionsResult
    ? optionsResult.error || options?.profileError || (!canEditAny ? "You don't have permission to edit this hangout." : null)
    : null;
  const error = d.mode === "edit" && !d.eventId ? "Choose a hangout to edit using its event link." : current?.error || accessError;
  const canChangeCommunity = d.mode === "create" || (owner && (!original?.community_id || Boolean(options?.communities.some((community) => community.id === original.community_id))));

  async function removeHangout() {
    const identity = getIdentity();
    if (!identity || !original || !canEdit || identity.signer.toLowerCase() !== auth.address?.toLowerCase() || !window.confirm(`Delete ${original.name}?`)) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await postJSON(`/events/api/events/${encodeURIComponent(original.id)}`, undefined, { identity, method: "DELETE" });
      navigate("/whats-on");
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : "Could not delete this hangout.");
    } finally {
      setDeleting(false);
    }
  }

  return (
    <LdSubmitHangoutView>
      {waiting ? <p role="status">Loading your hangout…</p> : error ? <div><p role="alert">{error}</p><Link to="/whats-on?tab=my">Back to My Hangouts</Link><button type="button" onClick={() => setOptionsAttempt((previous) => previous + 1)}>Check access again</button></div> : <SubmitHangout
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
        upload={(file, vertical) => {
          const identity = getIdentity();
          if (!identity) return Promise.reject(new Error("Sign in to upload an image."));
          return uploadHangoutPoster(identity, file, vertical);
        }}
      />}
      {original && canEdit && !waiting && !error && <button type="button" disabled={deleting} onClick={() => { void removeHangout(); }}>{deleting ? "Deleting…" : "Delete hangout"}</button>}
      {deleteError && <p role="alert">{deleteError}</p>}
    </LdSubmitHangoutView>
  );
}
