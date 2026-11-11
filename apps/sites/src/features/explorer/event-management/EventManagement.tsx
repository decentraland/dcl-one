import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { openSignIn } from "../../components/auth/signin-store";
import {
  EVENT_PERMISSIONS, loadEventProfile, loadEventProfiles, loadManagedEvents,
  moderateEvent, saveEventPermissions, type EventProfile, type ManagedEvent,
} from "@/data/lib/catalyst/places/event-management";
import styles from "./EventManagement.module.css";

type Mode = "mine" | "pending" | "users";
const PAGE_SIZE = 25;
const TITLES: Record<Mode, string> = { mine: "My Hangouts", pending: "Hangout moderation", users: "User permissions" };
const message = (error: unknown) => error instanceof Error ? error.message : "The request could not be completed.";
const has = (profile: EventProfile | null, permission: string) => profile?.permissions.includes(permission) ?? false;
const canModerate = (profile: EventProfile | null) => has(profile, "approve_any_event") || has(profile, "edit_any_event");

type Access = { profile: EventProfile; profiles: EventProfile[] | null };
function useEventAccess(identity: AuthIdentity | null, version: number) {
  const latestIdentity = useRef(identity);
  latestIdentity.current = identity;
  const [read, setRead] = useState<{ data?: Access; error?: string } | null>(null);
  const signer = identity?.signer.toLowerCase();
  useEffect(() => {
    const current = latestIdentity.current;
    if (!current) return;
    const controller = new AbortController();
    async function load() {
      const profile = await loadEventProfile(current!, controller.signal);
      let profiles: EventProfile[] | null = null;
      // The permission is only a hint to probe. The moderator-only endpoint
      // decides whether private profiles and administration may be exposed.
      if (has(profile, "edit_any_profile")) {
        try { profiles = await loadEventProfiles(current!, controller.signal); }
        catch (error) {
          if (!(error instanceof CatalystError && error.status === 403)) throw error;
        }
      }
      return { profile, profiles };
    }
    load().then(
      (data) => { if (!controller.signal.aborted) setRead({ data }); },
      (error: unknown) => { if (!controller.signal.aborted) setRead({ error: message(error) }); },
    );
    return () => controller.abort();
  }, [signer, version]);
  // A permission save refreshes access in the background; preserve the current
  // authorized view until its replacement arrives. Wallet changes remount us.
  const current = read;
  return { profile: current?.data?.profile ?? null, profiles: current?.data?.profiles ?? null, error: current?.error, loading: Boolean(identity) && !current };
}

export function EventNavigation({ profile = null, canManageUsers = false }: { profile?: EventProfile | null; canManageUsers?: boolean }) {
  return (
    <nav className={styles.navigation} aria-label="Hangout navigation">
      <Link className={styles.link} to="/events">All hangouts</Link>
      <Link className={styles.link} to="/events?tab=my">My Hangouts</Link>
      <Link className={styles.link} to="/events/new-event">Create a Hangout</Link>
      {canModerate(profile) && <Link className={styles.link} to="/events/admin/pending-events">Moderation</Link>}
      {canManageUsers && <Link className={styles.link} to="/events/admin/users">User permissions</Link>}
      {has(profile, "edit_any_schedule") && <Link className={styles.link} to="/landings/event-schedule">Schedules</Link>}
    </nav>
  );
}

// Public discovery needs no private profile lookup. Management loads access
// after the user follows My Hangouts.
export function EventDiscoveryNavigation() {
  return <div className={styles.discoveryNavigation}><EventNavigation /></div>;
}

export default function EventManagement({ mode }: { mode: Mode }) {
  const auth = useAuth();
  const identity = auth.isConnected ? auth.identity : null;
  return <ManagementSession key={`${identity?.signer.toLowerCase() ?? "anonymous"}:${mode}`} identity={identity} mode={mode} />;
}

type Read = { key: string; events: ManagedEvent[]; hasMore: boolean; error?: string };
function ManagementSession({ mode, identity }: { mode: Mode; identity: AuthIdentity | null }) {
  const [page, setPage] = useState(0);
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState("");
  const [version, setVersion] = useState(0);
  const [accessVersion, setAccessVersion] = useState(0);
  const access = useEventAccess(identity, accessVersion);
  const [read, setRead] = useState<Read | null>(null);
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [rejection, setRejection] = useState<{ id: string; name: string; reason: string } | null>(null);
  const [address, setAddress] = useState("");
  const [permissions, setPermissions] = useState<string[]>([]);
  const [editingProfile, setEditingProfile] = useState(false);
  const canManageUsers = access.profiles !== null;
  const allowed = mode === "mine" || (mode === "pending" ? canModerate(access.profile) : canManageUsers);
  const key = JSON.stringify([page, query, version]);
  const latestIdentity = useRef(identity);
  latestIdentity.current = identity;

  const settledSearch = useRef(search);
  useEffect(() => {
    if (search === settledSearch.current) return;
    const timer = setTimeout(() => {
      settledSearch.current = search;
      setPage(0);
      setQuery(search.trim().length >= 3 ? search.trim() : "");
    }, 300);
    return () => clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    const current = latestIdentity.current;
    if (!current || !allowed || mode === "users") return;
    const controller = new AbortController();
    loadManagedEvents(current, { own: mode === "mine", offset: page * PAGE_SIZE, limit: PAGE_SIZE, search: query }, controller.signal).then(
      (data) => { if (!controller.signal.aborted) setRead({ key, ...data }); },
      (error: unknown) => { if (!controller.signal.aborted) setRead({ key, events: [], hasMore: false, error: message(error) }); },
    );
    return () => controller.abort();
  }, [allowed, mode, page, query, key]);

  const current = read?.key === key ? read : null;
  const loading = mode === "users" ? access.loading : Boolean(identity) && allowed && !current;
  const filteredProfiles = (access.profiles ?? []).filter((profile) => !query || profile.user.toLowerCase().includes(query.toLowerCase()));
  const profiles = filteredProfiles.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  const events = current?.events ?? [];
  const more = mode === "users" ? (page + 1) * PAGE_SIZE < filteredProfiles.length : current?.hasMore;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  function resetSelection() { setRejection(null); setFeedback(null); setEditingProfile(false); }
  function changePage(next: number) { resetSelection(); setPage(next); }

  async function applyModeration(id: string, action: "approve" | "reject", reason?: string) {
    const signer = getIdentity();
    if (!signer || signer.signer.toLowerCase() !== identity?.signer.toLowerCase()) { openSignIn(); return; }
    setBusy(true); setFeedback(null);
    try {
      await moderateEvent(signer, id, action, reason);
      if (!mounted.current) return;
      setRejection(null); setVersion((previous) => previous + 1);
      setFeedback(action === "approve" ? "Hangout approved." : "Hangout rejected.");
    } catch (error) { if (mounted.current) setFeedback(message(error)); }
    finally { if (mounted.current) setBusy(false); }
  }

  async function savePermissions() {
    const signer = getIdentity();
    if (!signer || signer.signer.toLowerCase() !== identity?.signer.toLowerCase()) { openSignIn(); return; }
    setBusy(true); setFeedback(null);
    try {
      const saved = await saveEventPermissions(signer, address.trim(), permissions);
      if (!mounted.current) return;
      setEditingProfile(false);
      setAccessVersion((previous) => previous + 1);
      const effectiveMatches = saved.permissions.length === permissions.length && permissions.every((permission) => saved.permissions.includes(permission));
      setFeedback(effectiveMatches
        ? "Delegated permissions saved. Moderator accounts always retain every permission."
        : "The server kept the account's effective permissions. Moderator accounts always retain every permission.");
    } catch (error) { if (mounted.current) setFeedback(message(error)); }
    finally { if (mounted.current) setBusy(false); }
  }

  function editProfile(profile?: EventProfile) {
    setAddress(profile?.user ?? ""); setPermissions(profile?.permissions ?? []); setEditingProfile(true);
  }

  return (
    <SitesChrome active="whatson">
      <main className={styles.root}>
        <EventNavigation profile={access.profile} canManageUsers={canManageUsers} />
        <h1>{TITLES[mode]}</h1>
        {!identity ? (
          <div>
            <p>Sign in to {mode === "mine" ? "manage your hangouts" : "continue"}.</p>
            <button className={styles.button} onClick={() => openSignIn()}>Sign in</button>
          </div>
        ) : access.error && mode !== "mine" ? <p role="alert">{access.error}</p>
          : access.loading && mode !== "mine" ? <p role="status">Checking access…</p>
          : !allowed ? <p role="alert">{mode === "pending" && has(access.profile, "approve_own_event")
            ? "Approve your own pending hangouts from My Hangouts."
            : "Your account does not have permission to manage this area."}</p>
          : (
            <>
              <label className={styles.label}>
                Search {mode === "users" ? "wallets" : "hangouts"}
                <input className={styles.input} type="search" value={search} disabled={busy} onChange={(event) => { resetSelection(); setSearch(event.target.value); }} />
              </label>
              {search.trim().length > 0 && search.trim().length < 3 && <p>Enter at least three characters to search.</p>}
              {feedback && <p role="status">{feedback}</p>}
              {loading ? <p role="status">Loading…</p> : current?.error && mode !== "users" ? (
                <div>
                  <p role="alert">{current.error}</p>
                  <button className={styles.button} onClick={() => setVersion((previous) => previous + 1)}>Retry</button>
                </div>
              ) : mode === "users" ? (
                <>
                  <button className={styles.button} onClick={() => editProfile()}>Add permissions for a wallet</button>
                  <ul className={styles.list}>
                    {profiles.map((profile) => (
                      <li className={styles.item} key={profile.user}>
                        <span>{profile.user}</span>
                        <span>{profile.permissions.map((permission) => EVENT_PERMISSIONS.find((entry) => entry.id === permission)?.label ?? permission).join(", ") || "No permissions"}</span>
                        <button className={styles.button} aria-label={`Edit permissions for ${profile.user}`} onClick={() => editProfile(profile)}>Edit permissions</button>
                      </li>
                    ))}
                  </ul>
                  {profiles.length === 0 && <p>No matching users.</p>}
                </>
              ) : (
                <>
                  <ul className={styles.list}>
                    {events.map((event) => {
                      const pending = !event.approved && !event.rejected && !event.deleted_by_user && !event.deleted_by_admin && new Date(event.next_finish_at ?? event.finish_at ?? 0).getTime() > Date.now();
                      const mayApprove = pending && (has(access.profile, "approve_any_event") || (has(access.profile, "approve_own_event") && event.user?.toLowerCase() === identity.signer.toLowerCase()));
                      return (
                        <li className={styles.item} key={event.id}>
                          {event.image && <img className={styles.image} src={event.image} alt="" width={160} height={80} />}
                          <div className={styles.details}>
                            <h2>{event.name}</h2>
                            <p>{event.description}</p>
                            <p>{event.start_at ? new Date(event.start_at).toLocaleString() : "Date not published"} · {event.world ? event.server || "World location not published" : `${event.x},${event.y}`}</p>
                            <p>{event.rejected ? "Rejected" : event.approved ? "Approved" : "Pending review"}{event.rejection_reason ? `: ${event.rejection_reason}` : ""}</p>
                            {(mode === "mine" || has(access.profile, "edit_any_event")) && <Link className={styles.link} to={`/events/edit-event/${encodeURIComponent(event.id)}`}>Edit hangout</Link>}
                            {event.approved && <Link className={styles.link} to={`/events?id=${encodeURIComponent(event.id)}`}>View hangout</Link>}
                            {mayApprove && (
                              <div>
                                <button className={styles.button} disabled={busy} onClick={() => { void applyModeration(event.id, "approve"); }}>Approve {event.name}</button>
                                <button className={styles.button} disabled={busy} onClick={() => setRejection({ id: event.id, name: event.name, reason: "" })}>Reject {event.name}</button>
                              </div>
                            )}
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                  {events.length === 0 && <p>{mode === "mine" ? "No hangouts found. Create your first hangout to get started." : "No pending hangouts on this page."}</p>}
                </>
              )}
              <nav className={styles.pager} aria-label="Hangout pages">
                <button className={styles.button} disabled={page === 0 || loading || busy} onClick={() => changePage(page - 1)}>Previous page</button>
                <span>Page {page + 1}</span>
                <button className={styles.button} disabled={!more || loading || busy} onClick={() => changePage(page + 1)}>Next page</button>
              </nav>
              {rejection && (
                <section className={styles.editor} aria-label={`Reject ${rejection.name}`}>
                  <h2>Reject {rejection.name}</h2>
                  <label className={styles.label}>Reason
                    <textarea className={styles.input} maxLength={500} value={rejection.reason} onChange={(event) => setRejection({ ...rejection, reason: event.target.value })} />
                  </label>
                  <button className={styles.button} disabled={busy || !rejection.reason.trim()} onClick={() => { void applyModeration(rejection.id, "reject", rejection.reason); }}>Confirm rejection</button>
                  <button className={styles.button} disabled={busy} onClick={() => setRejection(null)}>Cancel</button>
                </section>
              )}
              {editingProfile && (
                <section className={styles.editor} aria-label="Edit user permissions">
                  <h2>Edit user permissions</h2>
                  <p>Moderator accounts always retain every permission. This form updates delegated grants. The legacy profile permission does not grant user administration.</p>
                  <label className={styles.label}>Wallet address
                    <input className={styles.input} value={address} onChange={(event) => setAddress(event.target.value)} />
                  </label>
                  {EVENT_PERMISSIONS.map((permission) => (
                    <label className={styles.label} key={permission.id}>
                      <input type="checkbox" checked={permissions.includes(permission.id)} onChange={(event) => setPermissions((previous) => event.target.checked ? [...previous, permission.id] : previous.filter((value) => value !== permission.id))} />
                      {permission.label}
                    </label>
                  ))}
                  <button className={styles.button} disabled={busy} onClick={() => { void savePermissions(); }}>Save permissions</button>
                  <button className={styles.button} disabled={busy} onClick={() => setEditingProfile(false)}>Cancel</button>
                </section>
              )}
            </>
          )}
      </main>
    </SitesChrome>
  );
}
