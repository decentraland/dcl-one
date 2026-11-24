CREATE OR REPLACE FUNCTION pg_temp.builder_collection_fingerprint(c builder.collections) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(c.name, c.eth_address, c.salt, c.contract_address, c.urn_suffix, c.third_party_id,
                              c.publication_pending, c.created_at, c.updated_at, c.linked_contract_address,
                              c.linked_contract_network, c.lock, c.forum_link, c.forum_id, c.forum_claimed_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_item_fingerprint(i builder.items) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(i.collection_id, i.eth_address, i.name, i.description, i.thumbnail, i.video, i.type,
                              i.rarity, i.price, i.beneficiary, i.data, i.metrics, i.utility, i.mappings, i.urn_suffix,
                              i.blockchain_item_id, i.is_published, i.in_catalyst, i.total_supply,
                              i.local_content_hash, i.content_hash, i.catalyst_content_hash, i.created_at,
                              i.updated_at, i.video_named_by)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_item_content_fingerprint(f builder.item_contents) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(f.hash, f.position, f.named_by)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_uploaded_content_fingerprint(u builder.uploaded_contents) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(u.size, u.created_at, u.video)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_virtual_third_party_fingerprint(v builder.virtual_third_parties) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(v.managers, v.raw_metadata, v.is_programmatic, v.created_at, v.updated_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_collection_curation_fingerprint(k builder.collection_curations) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(k.collection_id, k.status, k.assignee, k.reviewed_by, k.rejection_reasons,
                              k.rejection_message, k.created_at, k.updated_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_item_curation_fingerprint(k builder.item_curations) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(k.item_id, k.status, k.content_hash, k.is_mapping_complete, k.created_at,
                              k.updated_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_collection_event_fingerprint(e builder.collection_events) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(e.collection_id, e.type, e.actor, e.actor_address, e.payload, e.created_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_slot_usage_cheque_fingerprint(q builder.slot_usage_cheques) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(q.collection_id, q.third_party_id, q.signature, q.salt, q.qty, q.created_at,
                              q.updated_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_lock_units(unit_collections uuid[], unit_items uuid[]) RETURNS void
LANGUAGE plpgsql AS $$ BEGIN
  PERFORM 1 FROM builder.collections c WHERE c.id = ANY(unit_collections) ORDER BY c.id FOR UPDATE;
  PERFORM 1 FROM builder.items i WHERE i.collection_id = ANY(unit_collections) OR i.id = ANY(unit_items)
    ORDER BY i.id FOR UPDATE;
END $$;
