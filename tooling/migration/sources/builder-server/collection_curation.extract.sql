SELECT cc.id,
       cc.collection_id,
       cc.status::text AS status,
       cc.assignee,
       to_jsonb(cc) ->> 'reviewed_by' AS reviewed_by,
       CASE WHEN jsonb_typeof(to_jsonb(cc) -> 'rejection_reasons') = 'array'
            THEN ARRAY(SELECT jsonb_array_elements_text(to_jsonb(cc) -> 'rejection_reasons')) END AS rejection_reasons,
       to_jsonb(cc) ->> 'rejection_message' AS rejection_message,
       cc.created_at AT TIME ZONE 'UTC' AS created_at,
       cc.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM collection_curations cc
