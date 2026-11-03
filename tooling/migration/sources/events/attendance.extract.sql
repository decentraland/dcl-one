-- events `event_attendees` -> explore.event_attendance_local 'going' rows, as
-- attendees.rs rsvp_going writes them (lowercased signer, {"user_name"}).
-- Rows of events that no longer exist are left out: the mirror sweep would
-- delete them. A wallet listed twice in different case is one RSVP.
SELECT a.event_id::text                                   AS event_id,
       lower(a."user")                                    AS signer,
       max(a.user_name)                                   AS user_name,
       COALESCE(min(a.created_at), min(e.created_at))     AS signed_at
FROM event_attendees a
JOIN events e ON e.id = a.event_id
GROUP BY a.event_id, lower(a."user")
