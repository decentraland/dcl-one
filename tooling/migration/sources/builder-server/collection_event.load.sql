INSERT INTO builder.collection_events (id, collection_id, type, actor, actor_address, payload, created_at)
SELECT s.id, s.collection_id, s.type, s.actor,
       CASE WHEN lower(s.actor_address) ~ '^0x[0-9a-f]{40}$' THEN lower(s.actor_address) END,
       s.payload, s.created_at AT TIME ZONE 'UTC'
FROM stage s
JOIN builder.collections c ON c.id = s.collection_id
WHERE s.type IN ('collection.published', 'review.human_required', 'review.automated_started',
                 'review.automated_passed', 'review.automated_rejected', 'review.automated_error',
                 'review.appeal_requested', 'review.assigned', 'review.approved', 'review.rejected',
                 'changes.submitted', 'review.comment')
  AND s.actor IN ('creator', 'curator', 'validator', 'system')
  AND jsonb_typeof(s.payload) = 'object'
ON CONFLICT DO NOTHING;
