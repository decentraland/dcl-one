import { sitesFooterCopy } from "@dcl/one-ui/apps/sites/chrome/footer-copy";
import { EventDetailModal } from "@dcl/one-ui/apps/sites/events/DetailModal";
import { Dialog } from "@dcl/one-ui/apps/sites/events/Dialog";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import { CardImage } from "@dcl/one-ui/components/CardImage/CardImage";
import { useQueryClient } from "@tanstack/react-query";
import { useEventsCopy } from "@dcl/one-ui/apps/sites/events/locale";
import { useEffect, useId, useRef, useState } from "react";
import { Link, useNavigate } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { invalidateEventsAfterWrite } from "@/data/lib/catalyst/places/events-page";
import { openSignIn } from "../../components/auth/signin-store";
import {
  EVENT_PERMISSIONS, loadEventProfile, loadEventProfiles, loadModerationEvents, loadOwnedEvents,
  moderateEvent, saveEventPermissions, type EventProfile, type ManagedEvent,
} from "@/data/lib/catalyst/places/event-management";
import styles from "./EventManagement.module.css";

type Mode = "mine" | "pending" | "users";
const PAGE_SIZE = 25;
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
  const { text } = useEventsCopy();
  return (
    <nav className={styles.navigation} aria-label={text("Hangout navigation")}>
      <Link className={styles.link} to="/events">{text("All hangouts")}</Link>
      <Link className={styles.link} to="/events?tab=my">{text("My Hangouts")}</Link>
      <Link className={styles.link} to="/events/new-event">{text("Create a Hangout")}</Link>
      {canModerate(profile) && <Link className={styles.link} to="/events/admin/pending-events">{text("Moderation")}</Link>}
      {canManageUsers && <Link className={styles.link} to="/events/admin/users">{text("User permissions")}</Link>}
    </nav>
  );
}

export default function EventManagement({ mode }: { mode: Mode }) {
  const auth = useAuth();
  const identity = auth.isConnected ? auth.identity : null;
  return <ManagementSession key={`${identity?.signer.toLowerCase() ?? "anonymous"}:${mode}`} identity={identity} mode={mode} />;
}

function ModerationCards({ events, onOpen }: { events: ManagedEvent[]; onOpen: (event: ManagedEvent) => void }) {
  const { locale, text } = useEventsCopy();
  const statusPrefix = useId();
  return (
    <ul className={styles.pendingGrid}>
      {events.map((event) => {
        const start = event.next_start_at ?? event.start_at;
        const statusId = `${statusPrefix}-${event.id}`;
        return (
          <li key={event.id}>
            <button className={styles.pendingCard} data-pending={!event.approved} type="button" aria-label={event.name} aria-describedby={statusId} onClick={() => onOpen(event)}>
              <span className={styles.pendingCover}>
                <CardImage className={styles.pendingImage} src={event.image} alt="" />
                <span className={styles.pendingChips}>
                  {start && <time dateTime={start}>{new Date(start).toLocaleDateString(locale, { month: "short", day: "numeric" })}</time>}
                  <span className={styles.pendingStatus} id={statusId}>{text(event.approved ? "Approved" : "Pending")}</span>
                </span>
              </span>
              <span className={styles.pendingDetails}>
                <span className={styles.pendingName}>{event.name}</span>
                {event.user_name && <span>{text("By")} {event.user_name}</span>}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

type Read = { key: string; events: ManagedEvent[]; recentlyApproved?: ManagedEvent[]; hasMore: boolean; error?: string };
function ManagementSession({ mode, identity }: { mode: Mode; identity: AuthIdentity | null }) {
  const { locale, text, errorText } = useEventsCopy();
  const navigate = useNavigate();
  const rejectionTitle = useId();
  const client = useQueryClient();
  const [review, setReview] = useState<ManagedEvent | null>(null);
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
    const request = mode === "pending"
      ? loadModerationEvents(current, controller.signal)
      : loadOwnedEvents(current, { offset: page * PAGE_SIZE, limit: PAGE_SIZE, search: query }, controller.signal);
    request.then(
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

  function resetSelection() { setReview(null); setRejection(null); setFeedback(null); setEditingProfile(false); }
  function changePage(next: number) { resetSelection(); setPage(next); }

  function openReview(event: ManagedEvent) {
    setFeedback(null);
    setRejection(null);
    setReview(event);
  }

  function openRejection(event: ManagedEvent) {
    setFeedback(null);
    setReview(null);
    setRejection({ id: event.id, name: event.name, reason: "" });
  }

  function closeReview() {
    if (busy) return;
    setReview(null);
    setFeedback(null);
  }

  async function applyModeration(id: string, action: "approve" | "reject", reason?: string) {
    const signer = getIdentity();
    if (!signer || signer.signer.toLowerCase() !== identity?.signer.toLowerCase()) { openSignIn(); return; }
    if (busy) return;
    setBusy(true); setFeedback(null);
    try {
      await moderateEvent(signer, id, action, reason);
      void invalidateEventsAfterWrite(client);
      if (!mounted.current) return;
      setReview(null); setRejection(null); setVersion((previous) => previous + 1);
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

  const reviewCanModerate = review && !review.approved && (mode === "mine" || has(access.profile, "approve_any_event"));
  const rejectionFields = rejection && (
    <>
      <h2 id={rejectionTitle}>{text("Reject {name}", { name: rejection.name })}</h2>
      {mode === "pending" && feedback && <p role="alert">{errorText(feedback)}</p>}
      <label className={styles.label}>{text("Reason")}
        <textarea className={styles.input} maxLength={500} value={rejection.reason} onChange={(event) => setRejection({ ...rejection, reason: event.target.value })} />
      </label>
      <div className={styles.moderationActions}>
        <Button variant="tertiary" disabled={busy} onClick={() => { setFeedback(null); setRejection(null); }}>{text("Cancel")}</Button>
        <Button disabled={busy || !rejection.reason.trim()} onClick={() => { void applyModeration(rejection.id, "reject", rejection.reason); }}>{text("Confirm rejection")}</Button>
      </div>
    </>
  );

  return (
    <SitesChrome active="whatson" headerSurface={mode === "pending" ? "overlay" : "solid"} overlayNav={mode === "pending"} footerCopy={sitesFooterCopy[locale]}>
      <main className={mode === "pending" ? styles.pendingRoot : styles.root}>
        {mode !== "pending" && <EventNavigation profile={access.profile} canManageUsers={canManageUsers} />}
        <h1 id={mode === "pending" ? "pending-events-heading" : undefined}>{mode === "users" ? "User permissions" : text(mode === "mine" ? "My Hangouts" : "Pending Events")}</h1>
        {!identity ? (
          <div>
            <p>{mode === "mine" ? text("Sign in to manage your hangouts.") : "Sign in to continue."}</p>
            <button className={styles.button} onClick={() => openSignIn()}>{text("Sign in")}</button>
          </div>
        ) : access.error && mode !== "mine" ? <p role="alert">{access.error}</p>
          : access.loading && mode !== "mine" ? <p role="status">Checking access…</p>
          : !allowed ? <p role="alert">{mode === "pending" && has(access.profile, "approve_own_event")
            ? "Approve your own pending hangouts from My Hangouts."
            : "Your account does not have permission to manage this area."}</p>
          : (
            <>
              {mode !== "pending" && <>
                <label className={styles.label}>
                  {mode === "mine" ? text("Search hangouts") : "Search wallets"}
                  <input className={styles.input} type="search" value={search} disabled={busy} onChange={(event) => { resetSelection(); setSearch(event.target.value); }} />
                </label>
                {search.trim().length > 0 && search.trim().length < 3 && <p>{text("Enter at least three characters to search.")}</p>}
              </>}
              {feedback && !review && !(mode === "pending" && rejection) && <p role="status">{errorText(feedback)}</p>}
              {loading ? <p role="status">{text("Loading…")}</p> : current?.error && mode !== "users" ? (
                <div>
                  <p role="alert">{errorText(current.error)}</p>
                  <button className={styles.button} onClick={() => setVersion((previous) => previous + 1)}>{text("Retry")}</button>
                </div>
              ) : mode === "pending" ? (
                <>
                  <section aria-labelledby="pending-events-heading">
                    {events.length > 0 ? <ModerationCards events={events} onOpen={openReview} /> : <p className={styles.pendingEmpty}>{text("No events waiting for approval")}</p>}
                  </section>
                  <section className={styles.recentlyApproved} aria-labelledby="recently-approved-heading">
                    <h2 id="recently-approved-heading">{text("Recently Approved")} <span>{text("(Last 24hs)")}</span></h2>
                    <ModerationCards events={current?.recentlyApproved ?? []} onOpen={openReview} />
                  </section>
                </>
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
                            <p>{event.start_at ? new Date(event.start_at).toLocaleString(locale) : text("Date not published")} · {event.world ? event.server || text("World location not published") : `${event.x},${event.y}`}</p>
                            <p>{text(event.rejected ? "Rejected" : event.approved ? "Approved" : "Pending review")}{event.rejection_reason ? `: ${event.rejection_reason}` : ""}</p>
                            <Link className={styles.link} to={`/events/edit-event/${encodeURIComponent(event.id)}`}>{text("Edit hangout")}</Link>
                            {event.approved && <Link className={styles.link} to={`/events?id=${encodeURIComponent(event.id)}`}>{text("View hangout")}</Link>}
                            {mayApprove && (
                              <div>
                                <button className={styles.button} disabled={busy} onClick={() => openReview(event)}>{text("Approve {name}", { name: event.name })}</button>
                                <button className={styles.button} disabled={busy} onClick={() => openRejection(event)}>{text("Reject {name}", { name: event.name })}</button>
                              </div>
                            )}
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                  {events.length === 0 && <p>{text("No hangouts found. Create your first hangout to get started.")}</p>}
                </>
              )}
              {mode !== "pending" && <nav className={styles.pager} aria-label={text("Hangout pages")}>
                <button className={styles.button} disabled={page === 0 || loading || busy} onClick={() => changePage(page - 1)}>{text("Previous page")}</button>
                <span>{text("Page {page}", { page: page + 1 })}</span>
                <button className={styles.button} disabled={!more || loading || busy} onClick={() => changePage(page + 1)}>{text("Next page")}</button>
              </nav>}
              {review && (
                <EventDetailModal
                  title={review.name}
                  image={review.image}
                  live={false}
                  when={review.start_at ? { label: new Date(review.start_at).toLocaleString(locale), dateTime: review.start_at } : undefined}
                  location={{ world: review.world, label: review.world ? review.server || text("World location not published") : `${review.x},${review.y}` }}
                  description={review.description ?? undefined}
                  creator={review.user_name ? { name: review.user_name } : undefined}
                  onClose={closeReview}
                  feedback={feedback && <p role="alert">{errorText(feedback)}</p>}
                  actions={<>
                    {mode === "pending" && has(access.profile, "edit_any_event") && <Button variant="tertiary" disabled={busy} onClick={() => navigate(`/events/edit-event/${encodeURIComponent(review.id)}`)}>{text("Edit Event")}</Button>}
                    {mode === "pending" && reviewCanModerate && <Button variant="tertiary" disabled={busy} onClick={() => openRejection(review)}>{text("Reject")}</Button>}
                    {mode === "mine" && <Button variant="tertiary" disabled={busy} onClick={closeReview}>{text("Cancel")}</Button>}
                    {reviewCanModerate && <Button disabled={busy} onClick={() => { void applyModeration(review.id, "approve"); }}>{text("Approve event")}</Button>}
                  </>}
                />
              )}
              {rejectionFields && (mode === "pending"
                ? <Dialog labelledBy={rejectionTitle} onClose={() => { if (!busy) { setFeedback(null); setRejection(null); } }}>
                    <div className={styles.rejectDialog}>{rejectionFields}</div>
                  </Dialog>
                : <section className={styles.editor} aria-labelledby={rejectionTitle}>{rejectionFields}</section>)}
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
                  <button className={styles.button} disabled={busy} onClick={() => setEditingProfile(false)}>{text("Cancel")}</button>
                </section>
              )}
            </>
          )}
      </main>
    </SitesChrome>
  );
}
