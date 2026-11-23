SELECT DISTINCT a.event_id::text AS event_id, lower(a."user") AS signer
FROM event_attendees a JOIN events e ON e.id = a.event_id
