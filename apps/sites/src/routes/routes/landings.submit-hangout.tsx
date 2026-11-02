import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import LdSubmitHangoutView from "@ui/landings/pages/LdSubmitHangoutView";

import { fetchEventCategories, type EventCategory } from "@/data/lib/catalyst/places/events";
import { buildHangoutSubmit, deleteHangout, emptyDraft, eventToDraft, loadEditableHangout, uploadHangoutPoster, type EditableHangout } from "@/data/lib/catalyst/landings/submit-hangout";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import { SitesChromeMaybe } from "@ui/web/frames/SitesChrome";
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
  const waiting = d.mode === "edit" && Boolean(d.eventId) && auth.isConnected && !current;
  const error = d.mode === "edit" && !d.eventId ? "Choose a hangout to edit using its event link." : current?.error;

  async function removeHangout() {
    const identity = getIdentity();
    if (!identity || !original || !window.confirm(`Delete ${original.name}?`)) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await deleteHangout(identity, original.id);
      navigate("/whats-on");
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : "Could not delete this hangout.");
    } finally {
      setDeleting(false);
    }
  }

  return (
    <SitesChromeMaybe active="whatson">
    <LdSubmitHangoutView>
      {waiting ? <p role="status">Loading your hangout…</p> : error ? <p role="alert">{error}</p> : <SubmitHangout
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
        submit={buildHangoutSubmit(getIdentity, original)}
        signedIn={auth.isConnected}
        onSignIn={openSignIn}
        upload={(file, vertical, signal) => {
          const identity = getIdentity();
          if (!identity) return Promise.reject(new Error("Sign in to upload an image."));
          return uploadHangoutPoster(identity, file, vertical, signal);
        }}
      />}
      {original && <button type="button" disabled={deleting} onClick={() => { void removeHangout(); }}>{deleting ? "Deleting…" : "Delete hangout"}</button>}
      {deleteError && <p role="alert">{deleteError}</p>}
    </LdSubmitHangoutView>
    </SitesChromeMaybe>
  );
}
