-- Scene projects. A project with no owner (rows older than the eth_address
-- column) cannot be read or written by anyone and is not imported. The
-- thumbnail upstream is a file name inside the project's S3 folder, not an
-- upload hash, so it is not carried (builder.projects.thumbnail is an FK to
-- uploaded_contents).
SELECT p.id,
       p.eth_address,
       p.title,
       COALESCE(p.description, '') AS description,
       p.scene_id,
       p.cols,
       p.rows,
       COALESCE(p.parcels, 0) AS parcels,
       COALESCE(p.transforms, 0) AS transforms,
       COALESCE(p.gltf_shapes, 0) AS gltf_shapes,
       COALESCE(p.nft_shapes, 0) AS nft_shapes,
       COALESCE(p.scripts, 0) AS scripts,
       COALESCE(p.entities, 0) AS entities,
       p.creation_coords,
       p.is_public,
       p.is_template,
       p.template_status,
       p.video,
       p.is_deleted,
       p.created_at AT TIME ZONE 'UTC' AS created_at,
       p.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM projects p
WHERE p.eth_address IS NOT NULL
