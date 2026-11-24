import { adminCopy, rejectionReason, relativeDateLabel, type RejectReasonId } from "@dcl/one-ui/apps/sites/events/admin-copy";
import { AdminPage, adminPageStyles } from "@dcl/one-ui/apps/sites/events/AdminPage";
import { Busy } from "@dcl/one-ui/apps/sites/events/Busy";
import { EventDetailModal } from "@dcl/one-ui/apps/sites/events/DetailModal";
import { PendingEventCard } from "@dcl/one-ui/apps/sites/events/PendingEventCard";
import { Notice } from "@dcl/one-ui/apps/sites/events/Notice";
import { RejectEventModal } from "@dcl/one-ui/apps/sites/events/RejectEventModal";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import { skipToken, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";

import { getIdentity } from "@/data/lib/auth/session";
import { invalidateEventsAfterWrite } from "@/data/lib/catalyst/places/events-page";
import {
  editEventHref, eventAdminKeys, eventOccurrence, eventReviewStatus, moderateEvent, moderationQueueQuery, type ManagedEvent,
} from "@/data/lib/catalyst/places/event-management";
import { useEventsFooterCopy } from "../../events/locale";
import { eventImage } from "../../events/view";
import { openSignIn } from "../../components/auth/signin-store";
import SitesLayout from "../../components/chrome/SitesLayout";
import { AdminAccessGate } from "./AdminAccessGate";
import { EventAdminTabs } from "./EventAdminTabs";
import { useEventAdmin, type EventAdminAccess } from "./useEventAdmin";
import { useToast } from "./useToast";
import styles from "./PendingEventsPage.module.css";

/** The Users and Pending pages render in English, as decentraland.org's (its admin copy has no translations). */
const DATE_LOCALE = "en-US";
const PENDING_PATH = "/events/admin/pending-events";
const failure = (error: unknown) => error instanceof Error && error.message ? error.message : adminCopy.pending.actionError;

export default function PendingEventsPage() {
  const access = useEventAdmin();
  const footerCopy = useEventsFooterCopy();
  return (
    <SitesLayout active="whatson" headerSurface="overlay" overlayNav footerCopy={footerCopy}>
      <AdminPage bar={<EventAdminTabs active="pending" />}>
        <AdminAccessGate access={access} allowed={access.canSeePending}>
          <PendingQueue key={access.identity?.signer.toLowerCase()} access={access} />
        </AdminAccessGate>
      </AdminPage>
    </SitesLayout>
  );
}

function PendingQueue({ access }: { access: EventAdminAccess }) {
  const identity = access.identity;
  // The queue is a signed read, so its cards render in the browser only, where the page origin is the window's.
  const pageOrigin = typeof window === "undefined" ? "" : window.location.origin;
  const client = useQueryClient();
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const queue = useQuery(identity ? moderationQueueQuery(identity) : { queryKey: eventAdminKeys.queue("anonymous"), queryFn: skipToken });
  const [rejecting, setRejecting] = useState<ManagedEvent | null>(null);
  // The review a moderation just closed. Its id leaves the URL in a later commit than the reason
  // dialog leaves state, so without this the review would show again for that commit.
  const [closing, setClosing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useToast();
  const mounted = useRef(true);
  useEffect(() => {
    // StrictMode runs this setup again after a cleanup, so the flag is restored here, not only at creation.
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const signer = identity?.signer.toLowerCase() ?? "";
  const own = (event: ManagedEvent) => (event.user ?? "").toLowerCase() === signer;
  // A reviewer's lists have every creator's events; an own-events approver's have only theirs
  // (the API answers them no one else's pending ones), kept to theirs here too, Recently Approved included.
  const reviewer = access.canApproveAny || access.canEditAny;
  const visible = (event: ManagedEvent) => reviewer || own(event);
  const pending = (queue.data?.events ?? []).filter(visible);
  // As the API's signed event PATCH: another creator's event takes `edit_any_event` before any change
  // (else 403), then `approve_any_event` to approve; the owner approves with either approval grant. A
  // rejection reason is stored only from an `approve_any_event` reviewer, so Reject stays theirs.
  const mayEdit = (event: ManagedEvent) => access.canEditAny || own(event);
  const mayApprove = (event: ManagedEvent) => mayEdit(event) && (access.canApproveAny || (access.canApproveOwn && own(event)));
  const mayReject = (event: ManagedEvent) => mayEdit(event) && access.canApproveAny;
  const recentlyApproved = (queue.data?.recentlyApproved ?? []).filter(visible);
  const openId = params.get("id");
  if (closing !== null && openId !== closing) setClosing(null);
  const review = openId && openId !== closing ? [...pending, ...recentlyApproved].find((event) => event.id === openId) ?? null : null;
  const now = new Date();

  function select(id: string | null) {
    setError(null);
    if (id && id === closing) setClosing(null);
    setParams((current) => {
      const next = new URLSearchParams(current);
      if (id) next.set("id", id);
      else next.delete("id");
      return next;
    }, { replace: !id, preventScrollReset: true });
  }

  function closeReview() {
    if (!busy) select(null);
  }

  async function moderate(event: ManagedEvent, action: "approve" | "reject", reason?: string) {
    const signer = getIdentity();
    if (!signer || signer.signer.toLowerCase() !== identity?.signer.toLowerCase()) {
      openSignIn();
      return;
    }
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await moderateEvent(signer, event.id, action, reason);
      void invalidateEventsAfterWrite(client);
      if (!mounted.current) return;
      setClosing(event.id);
      setRejecting(null);
      select(null);
      setToast({ message: action === "approve" ? adminCopy.pending.approveSuccess : adminCopy.pending.rejectSuccess, tone: "success" });
    } catch (cause) {
      if (mounted.current) setError(failure(cause));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  const cards = (events: ManagedEvent[]) => (
    <ul className={styles.grid}>
      {events.map((event) => {
        const { start } = eventOccurrence(event);
        return (
          <li key={event.id}>
            <PendingEventCard
              title={event.name}
              image={eventImage(event.image, pageOrigin)}
              href={`${PENDING_PATH}?id=${encodeURIComponent(event.id)}`}
              onOpen={(click) => {
                click.preventDefault();
                select(event.id);
              }}
              status={eventReviewStatus(event)}
              dateLabel={relativeDateLabel(start, now, DATE_LOCALE)}
              creatorName={event.user_name ?? undefined}
              time={start ? { label: new Date(start).toLocaleTimeString(DATE_LOCALE, { hour: "numeric", minute: "2-digit" }), dateTime: start } : undefined}
            />
          </li>
        );
      })}
    </ul>
  );

  const reviewPending = review !== null && eventReviewStatus(review) === "pending";
  const reviewStart = review ? eventOccurrence(review).start : null;
  const errorAlert = error ? <Notice tone="error">{error}</Notice> : undefined;

  return (
    <>
      <section className={adminPageStyles.section} aria-labelledby="pending-events-heading">
        <h1 id="pending-events-heading" className={adminPageStyles.title}>{adminCopy.pending.title}</h1>
        {queue.isPending ? (
          <Busy layout="page">{adminCopy.pending.loading}</Busy>
        ) : queue.isError ? (
          <div className={adminPageStyles.status}>
            <Notice tone="error">{failure(queue.error)}</Notice>
            <Button variant="secondary" onClick={() => void queue.refetch()}>{adminCopy.pending.retry}</Button>
          </div>
        ) : pending.length ? cards(pending) : (
          <p className={styles.empty}>{adminCopy.pending.empty}</p>
        )}
      </section>
      <section className={adminPageStyles.section} aria-labelledby="recently-approved-heading">
        <h2 id="recently-approved-heading" className={adminPageStyles.title}>
          {adminCopy.pending.recentlyApproved}{" "}
          <span className={adminPageStyles.subtitle}>{adminCopy.pending.recentlyApprovedSubtitle}</span>
        </h2>
        {cards(recentlyApproved)}
      </section>
      {review && !rejecting ? (
        <EventDetailModal
          title={review.name}
          image={eventImage(review.image, pageOrigin)}
          live={false}
          when={reviewStart ? { label: new Date(reviewStart).toLocaleString(DATE_LOCALE), dateTime: reviewStart } : undefined}
          location={{ world: review.world, label: review.world ? review.server || "World location not published" : `${review.x},${review.y}` }}
          description={review.description ?? undefined}
          creator={review.user_name ? { name: review.user_name } : undefined}
          onClose={closeReview}
          feedback={errorAlert}
          actions={<>
            {reviewPending && mayApprove(review) ? (
              <Button disabled={busy} onClick={() => void moderate(review, "approve")}>{adminCopy.pending.approve}</Button>
            ) : null}
            {reviewPending && mayReject(review) ? (
              <Button variant="tertiary" disabled={busy} onClick={() => { setError(null); setRejecting(review); }}>{adminCopy.pending.reject}</Button>
            ) : null}
            {mayEdit(review) ? (
              <Button variant="tertiary" disabled={busy} onClick={() => navigate(editEventHref(review.id))}>{adminCopy.pending.edit}</Button>
            ) : null}
          </>}
        />
      ) : null}
      {rejecting ? (
        <RejectEventModal
          busy={busy}
          feedback={errorAlert}
          onClose={() => { setError(null); setRejecting(null); }}
          onSubmit={({ reasons, notes }: { reasons: RejectReasonId[]; notes: string }) => void moderate(rejecting, "reject", rejectionReason(reasons, notes))}
        />
      ) : null}
      <div role="status">
        {toast ? <p className={adminPageStyles.toast} data-tone={toast.tone}>{toast.message}</p> : null}
      </div>
    </>
  );
}
