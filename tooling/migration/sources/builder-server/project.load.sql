-- files_version and manifest_revision stay the node's: an existing row keeps
-- its counters so a client's ETag/If-Match is not invalidated by a re-run.
INSERT INTO builder.projects AS p
    (id, eth_address, title, description, scene_id, cols, rows, parcels, transforms,
     gltf_shapes, nft_shapes, scripts, entities, creation_coords, is_public, is_template,
     template_status, video, is_deleted, created_at, updated_at)
SELECT id, lower(eth_address), title, description, scene_id, cols, rows, parcels, transforms,
       gltf_shapes, nft_shapes, scripts, entities, creation_coords, is_public, is_template,
       template_status, video, is_deleted,
       created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (id) DO UPDATE SET
    eth_address = EXCLUDED.eth_address,
    title = EXCLUDED.title,
    description = EXCLUDED.description,
    scene_id = EXCLUDED.scene_id,
    cols = EXCLUDED.cols,
    rows = EXCLUDED.rows,
    parcels = EXCLUDED.parcels,
    transforms = EXCLUDED.transforms,
    gltf_shapes = EXCLUDED.gltf_shapes,
    nft_shapes = EXCLUDED.nft_shapes,
    scripts = EXCLUDED.scripts,
    entities = EXCLUDED.entities,
    creation_coords = EXCLUDED.creation_coords,
    is_public = EXCLUDED.is_public,
    is_template = EXCLUDED.is_template,
    template_status = EXCLUDED.template_status,
    video = EXCLUDED.video,
    is_deleted = EXCLUDED.is_deleted,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE (p.eth_address, p.title, p.description, p.scene_id, p.cols, p.rows, p.parcels, p.transforms,
       p.gltf_shapes, p.nft_shapes, p.scripts, p.entities, p.creation_coords, p.is_public,
       p.is_template, p.template_status, p.video, p.is_deleted, p.created_at, p.updated_at)
      IS DISTINCT FROM
      (EXCLUDED.eth_address, EXCLUDED.title, EXCLUDED.description, EXCLUDED.scene_id, EXCLUDED.cols,
       EXCLUDED.rows, EXCLUDED.parcels, EXCLUDED.transforms, EXCLUDED.gltf_shapes,
       EXCLUDED.nft_shapes, EXCLUDED.scripts, EXCLUDED.entities, EXCLUDED.creation_coords,
       EXCLUDED.is_public, EXCLUDED.is_template, EXCLUDED.template_status, EXCLUDED.video,
       EXCLUDED.is_deleted, EXCLUDED.created_at, EXCLUDED.updated_at);
