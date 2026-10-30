import { useEffect, useState } from "react";
import { Link } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { openSignIn } from "../../components/auth/signin-store";
import { EVENT_PERMISSIONS, loadEventProfile, loadEventProfiles, loadManagedEvents, moderateEvent, saveEventPermissions, type EventProfile, type ManagedEvent } from "@/data/lib/catalyst/places/event-management";
import "./event-management.css";

type Mode = "mine" | "pending" | "users";
const PAGE_SIZE = 25;
const TITLES: Record<Mode, string> = { mine: "My Hangouts", pending: "Hangout moderation", users: "User permissions" };
const message = (error: unknown) => error instanceof Error ? error.message : "The request could not be completed.";
const has = (profile: EventProfile | null, permission: string) => profile?.permissions.includes(permission) ?? false;
const canModerate = (profile: EventProfile | null) => ["approve_own_event", "approve_any_event", "edit_any_event"].some((permission) => has(profile, permission));

function useEventAccess() {
  const auth = useAuth();
  const identity = auth.isConnected ? auth.identity : null;
  const [version, setVersion] = useState(0);
  const [read, setRead] = useState<{ identity: AuthIdentity; profile?: EventProfile; error?: string } | null>(null);
  useEffect(() => {
    if (!identity) return;
    const controller = new AbortController();
    loadEventProfile(identity, controller.signal).then(
      (profile) => { if (!controller.signal.aborted) setRead({ identity, profile }); },
      (error: unknown) => { if (!controller.signal.aborted) setRead({ identity, error: message(error) }); },
    );
    return () => controller.abort();
  }, [identity, version]);
  const current = read?.identity === identity ? read : null;
  return { identity, profile: current?.profile ?? null, error: current?.error, loading: Boolean(identity) && !current, refresh: () => setVersion((previous) => previous + 1) };
}

export function EventNavigation({ profile = null }: { profile?: EventProfile | null }) {
  return <nav className="event-management__nav" aria-label="Hangout navigation">
    <Link to="/whats-on">All hangouts</Link>
    <Link to="/whats-on?tab=my">My Hangouts</Link>
    <Link to="/events/new-event">Create a Hangout</Link>
    {canModerate(profile) && <Link to="/events/admin/pending-events">Moderation</Link>}
    {has(profile, "edit_any_profile") && <Link to="/events/admin/users">User permissions</Link>}
    {has(profile, "edit_any_schedule") && <Link to="/landings/event-schedule">Schedules</Link>}
  </nav>;
}

export function EventDiscoveryNavigation() {
  const access = useEventAccess();
  return <EventNavigation profile={access.profile} />;
}

type Read = { key: string; events: ManagedEvent[]; profiles: EventProfile[]; hasMore: boolean; error?: string };

export default function EventManagement({ mode }: { mode: Mode }) {
  const access = useEventAccess();
  const [page, setPage] = useState(0);
  const [search, setSearch] = useState("");
  const [version, setVersion] = useState(0);
  const [read, setRead] = useState<Read | null>(null);
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [rejection, setRejection] = useState<{ id: string; reason: string } | null>(null);
  const [address, setAddress] = useState("");
  const [permissions, setPermissions] = useState<string[]>([]);
  const [editingProfile, setEditingProfile] = useState(false);
  const query = search.trim().length >= 3 ? search.trim() : "";
  const allowed = mode === "mine" || (mode === "pending" ? canModerate(access.profile) : has(access.profile, "edit_any_profile"));
  const identity = access.identity;
  const key = JSON.stringify([identity?.signer, mode, page, query, version]);

  useEffect(() => {
    setPage(0); setSearch(""); setFeedback(null); setRejection(null); setAddress(""); setPermissions([]); setEditingProfile(false);
  }, [identity, mode]);

  useEffect(() => {
    if (!identity || !allowed) return;
    const controller = new AbortController();
    const load = mode === "users"
      ? loadEventProfiles(identity, controller.signal).then((profiles) => ({ events: [], profiles, hasMore: false }))
      : loadManagedEvents(identity, { own: mode === "mine", offset: page * PAGE_SIZE, limit: PAGE_SIZE, search: query }, controller.signal).then((result) => ({ ...result, profiles: [] }));
    load.then(
      (data) => { if (!controller.signal.aborted) setRead({ key, ...data }); },
      (error: unknown) => { if (!controller.signal.aborted) setRead({ key, events: [], profiles: [], hasMore: false, error: message(error) }); },
    );
    return () => controller.abort();
  }, [identity, allowed, mode, page, query, key]);

  const current = read?.key === key ? read : null;
  const loading = Boolean(identity) && allowed && !current;
  const filteredProfiles = (current?.profiles ?? []).filter((profile) => !query || profile.user.toLowerCase().includes(query.toLowerCase()));
  const profiles = filteredProfiles.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  const events = (current?.events ?? []).filter((event) => mode === "mine" || (
    !event.approved && !event.rejected && !event.deleted_by_user && !event.deleted_by_admin && new Date(event.next_finish_at ?? event.finish_at ?? 0).getTime() > Date.now()
  ));
  const more = mode === "users" ? (page + 1) * PAGE_SIZE < filteredProfiles.length : current?.hasMore;

  async function applyModeration(id: string, action: "approve" | "reject", reason?: string) {
    const signer = getIdentity();
    if (!signer || signer.signer.toLowerCase() !== identity?.signer.toLowerCase()) { openSignIn(); return; }
    setBusy(true); setFeedback(null);
    try {
      await moderateEvent(signer, id, action, reason);
      setRejection(null);
      setVersion((previous) => previous + 1);
      setFeedback(action === "approve" ? "Hangout approved." : "Hangout rejected.");
    } catch (error) { setFeedback(message(error)); }
    finally { setBusy(false); }
  }

  async function savePermissions() {
    const signer = getIdentity();
    if (!signer || signer.signer.toLowerCase() !== identity?.signer.toLowerCase()) { openSignIn(); return; }
    setBusy(true); setFeedback(null);
    try {
      await saveEventPermissions(signer, address.trim(), permissions);
      setEditingProfile(false); setVersion((previous) => previous + 1); access.refresh(); setFeedback("Permissions saved.");
    } catch (error) { setFeedback(message(error)); }
    finally { setBusy(false); }
  }

  return <SitesChrome active="whatson"><main className="event-management">
    <EventNavigation profile={access.profile} />
    <h1>{TITLES[mode]}</h1>
    {!identity ? <div><p>Sign in to {mode === "mine" ? "manage your hangouts" : "continue"}.</p><button onClick={() => openSignIn()}>Sign in</button></div>
      : access.error && mode !== "mine" ? <p role="alert">{access.error}</p>
      : access.loading && mode !== "mine" ? <p role="status">Checking access…</p>
      : !allowed ? <p role="alert">Your account does not have permission to manage this area.</p>
      : <>
        <label>Search {mode === "users" ? "wallets" : "hangouts"}<input type="search" value={search} onChange={(event) => { setSearch(event.target.value); setPage(0); }} /></label>
        {search.trim().length > 0 && search.trim().length < 3 && <p>Enter at least three characters to search.</p>}
        {feedback && <p role="status">{feedback}</p>}
        {loading ? <p role="status">Loading…</p> : current?.error ? <div><p role="alert">{current.error}</p><button onClick={() => setVersion((previous) => previous + 1)}>Retry</button></div>
          : mode === "users" ? <>
            <button onClick={() => { setAddress(""); setPermissions([]); setEditingProfile(true); }}>Add permissions for a wallet</button>
            <ul>{profiles.map((profile) => <li key={profile.user}><span>{profile.user}</span><span>{profile.permissions.map((permission) => EVENT_PERMISSIONS.find((entry) => entry.id === permission)?.label ?? permission).join(", ") || "No permissions"}</span><button aria-label={`Edit permissions for ${profile.user}`} onClick={() => { setAddress(profile.user); setPermissions(profile.permissions); setEditingProfile(true); }}>Edit permissions</button></li>)}</ul>
            {profiles.length === 0 && <p>No matching users.</p>}
          </> : <>
            <ul>{events.map((event) => <li key={event.id}>
              {event.image && <img src={event.image} alt="" width={160} height={80} />}
              <div><h2>{event.name}</h2><p>{event.description}</p><p>{event.start_at ? new Date(event.start_at).toLocaleString() : "Date not published"} · {event.world ? event.server : `${event.x},${event.y}`}</p>
                <p>{event.rejected ? "Rejected" : event.approved ? "Approved" : "Pending review"}{event.rejection_reason ? `: ${event.rejection_reason}` : ""}</p>
                {(mode === "mine" || has(access.profile, "edit_any_event")) && <Link to={`/events/edit-event/${encodeURIComponent(event.id)}`}>Edit hangout</Link>}
                {event.approved && <Link to={`/whats-on/${encodeURIComponent(event.id)}`}>View hangout</Link>}
                {mode === "pending" && (has(access.profile, "approve_any_event") || (has(access.profile, "approve_own_event") && event.user?.toLowerCase() === identity.signer.toLowerCase())) && <div>
                  <button disabled={busy} onClick={() => { void applyModeration(event.id, "approve"); }}>Approve {event.name}</button>
                  <button disabled={busy} onClick={() => setRejection({ id: event.id, reason: "" })}>Reject {event.name}</button>
                </div>}
              </div>
            </li>)}</ul>
            {events.length === 0 && <p>{mode === "mine" ? "No hangouts found. Create your first hangout to get started." : "No pending hangouts on this page."}</p>}
          </>}
        <nav className="event-management__pager" aria-label="Hangout pages">
          <button disabled={page === 0 || loading} onClick={() => setPage((previous) => previous - 1)}>Previous page</button>
          <span>Page {page + 1}</span>
          <button disabled={!more || loading} onClick={() => setPage((previous) => previous + 1)}>Next page</button>
        </nav>
        {rejection && <section className="event-management__editor" aria-label="Reject hangout"><h2>Reject hangout</h2><label>Reason<textarea maxLength={500} value={rejection.reason} onChange={(event) => setRejection({ ...rejection, reason: event.target.value })} /></label><button disabled={busy || !rejection.reason.trim()} onClick={() => { void applyModeration(rejection.id, "reject", rejection.reason); }}>Confirm rejection</button><button disabled={busy} onClick={() => setRejection(null)}>Cancel</button></section>}
        {editingProfile && <section className="event-management__editor" aria-label="Edit user permissions"><h2>Edit user permissions</h2><label>Wallet address<input value={address} onChange={(event) => setAddress(event.target.value)} /></label>
          {EVENT_PERMISSIONS.map((permission) => <label key={permission.id}><input type="checkbox" checked={permissions.includes(permission.id)} onChange={(event) => setPermissions((previous) => event.target.checked ? [...previous, permission.id] : previous.filter((value) => value !== permission.id))} />{permission.label}</label>)}
          <button disabled={busy} onClick={() => { void savePermissions(); }}>Save permissions</button><button disabled={busy} onClick={() => setEditingProfile(false)}>Cancel</button>
        </section>}
      </>}
  </main></SitesChrome>;
}
