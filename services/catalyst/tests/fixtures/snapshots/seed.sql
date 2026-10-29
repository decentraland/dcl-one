-- Deterministic content for tests/snapshot_generation.rs and the golden in
-- old-generator.json, which the OLD generator
-- (catalyrst/crates/catalyrst-db/src/snapshot_generator.rs,
-- generate_snapshots_multi) wrote from this same seed on a v2-schema database
-- with search_path=content (its SQL is unqualified). Apply to an empty
-- `schema apply` database.
--
-- The pass ends at END = 2020-01-01T00:00:00Z + 372 d + 5 h
-- (end_ms 1610010000000), which divides into one year, one month, one week
-- and one day window plus 5 h of remainder:
--   Y 2020-01-01 .. 2020-12-02   M 2020-12-02 .. 2020-12-30
--   W 2020-12-30 .. 2021-01-06   I 2021-01-06 .. 2021-01-07
-- Synced rows (a peer's) and local rows (a content.local_entities row, the
-- write path's provenance) are mixed in every window; the generator reads
-- them alike. Every local_timestamp is 2021-01-07 06:00, before any
-- generation, so no window is outdated.

-- Y: 600 profiles, 12 h apart from 2020-01-11, with a three-link ephemeral
-- chain: the file passes 256 KiB, so its CID is the chunked DAG.
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
SELECT '0x' || lpad(to_hex(i), 40, '0'),
       'v3',
       'profile',
       'bafkreiprofile' || lpad(i::text, 6, '0'),
       '{}'::json,
       timestamp '2020-01-11 00:00:00' + (i * interval '12 hours'),
       ARRAY['0x' || lpad(to_hex(i), 40, '0')],
       timestamp '2021-01-07 06:00:00',
       json_build_array(
           json_build_object('type', 'SIGNER', 'payload', '0x' || lpad(to_hex(i), 40, '0'), 'signature', ''),
           json_build_object('type', 'ECDSA_EPHEMERAL',
                             'payload', 'Decentraland Login' || chr(10) || 'Ephemeral address: 0x' || lpad(to_hex(i * 7919), 40, '0') || chr(10) || 'Expiration: 2021-02-01T00:00:00.000Z',
                             'signature', '0x' || repeat(lpad(to_hex(i), 4, '0'), 32) || '1b'),
           json_build_object('type', 'ECDSA_SIGNED_ENTITY',
                             'payload', 'bafkreiprofile' || lpad(i::text, 6, '0'),
                             'signature', '0x' || repeat(lpad(to_hex(i + 1), 4, '0'), 32) || '1c'))
FROM generate_series(1, 600) AS i;

-- The first instant of the first window (inclusive start), synced.
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
VALUES
    ('0x1111111111111111111111111111111111111111', 'v3', 'wearable', 'bafkreigenesiswearable',
     '{}', '2020-01-01 00:00:00', ARRAY['urn:decentraland:ethereum:collections-v1:genesis:hat'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x1111111111111111111111111111111111111111","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreigenesiswearable","signature":"0xaa"}]');

-- Before the first window: in no file.
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
VALUES
    ('0x2222222222222222222222222222222222222222', 'v3', 'scene', 'bafkreipregenesis',
     '{}', '2019-12-31 23:59:59.999', ARRAY['-150,-150'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x2222222222222222222222222222222222222222","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreipregenesis","signature":"0xbb"}]');

-- On the Y/M boundary (2020-12-02 00:00): in both files. Local.
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
VALUES
    ('0x3333333333333333333333333333333333333333', 'v3', 'scene', 'bafkreiboundaryscene',
     '{}', '2020-12-02 00:00:00', ARRAY['10,10', '10,11', '11,10'],
     '2021-01-07 06:00:00',
     -- stored with its keys out of order and spaced: the line re-serializes it (keys sorted)
     '[ {"signature": "", "payload": "0x3333333333333333333333333333333333333333", "type": "SIGNER"},
        {"payload": "bafkreiboundaryscene", "type": "ECDSA_SIGNED_ENTITY", "signature": "0xcc"} ]');
INSERT INTO content.local_entities (entity_id, signer, origin, published_at)
VALUES ('bafkreiboundaryscene', '0x3333333333333333333333333333333333333333', 'land-publish', '2021-01-07 06:00:00');

-- M: a scene overwritten inside the window (only the newer is active), and
-- an outfits entity whose SIGNER link has no signature key at all. Local.
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
VALUES
    ('0x4444444444444444444444444444444444444444', 'v3', 'scene', 'bafkreimonthscenenew',
     '{}', '2020-12-20 12:00:00.250', ARRAY['20,20'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x4444444444444444444444444444444444444444","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreimonthscenenew","signature":"0xdd"}]'),
    ('0x5555555555555555555555555555555555555555', 'v3', 'outfits', 'bafkreimonthoutfits',
     '{}', '2020-12-21 08:30:00', ARRAY['0x5555555555555555555555555555555555555555:outfits'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x5555555555555555555555555555555555555555"},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreimonthoutfits","signature":"0xee"}]');
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain, deleter_deployment)
SELECT '0x4444444444444444444444444444444444444444', 'v3', 'scene', 'bafkreimonthsceneold',
       '{}', '2020-12-10 12:00:00', ARRAY['20,20'],
       '2021-01-07 06:00:00',
       '[{"type":"SIGNER","payload":"0x4444444444444444444444444444444444444444","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreimonthsceneold","signature":"0xdf"}]',
       id
FROM content.deployments WHERE entity_id = 'bafkreimonthscenenew';
INSERT INTO content.local_entities (entity_id, signer, origin, published_at)
VALUES ('bafkreimonthscenenew', '0x4444444444444444444444444444444444444444', 'land-publish', '2021-01-07 06:00:00'),
       ('bafkreimonthsceneold', '0x4444444444444444444444444444444444444444', 'land-publish', '2021-01-07 06:00:00');

-- W: two entities on the same instant (inserted in entity_id order, which
-- is the order both generators write them), and a store.
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
VALUES
    ('0x6666666666666666666666666666666666666666', 'v3', 'emote', 'bafkreiweektiea',
     '{}', '2021-01-02 10:00:00', ARRAY['urn:decentraland:matic:collections-v2:0xabc:0'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x6666666666666666666666666666666666666666","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreiweektiea","signature":"0xf1"}]'),
    ('0x6666666666666666666666666666666666666666', 'v3', 'emote', 'bafkreiweektieb',
     '{}', '2021-01-02 10:00:00', ARRAY['urn:decentraland:matic:collections-v2:0xabc:1'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x6666666666666666666666666666666666666666","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreiweektieb","signature":"0xf2"}]'),
    ('0x7777777777777777777777777777777777777777', 'v3', 'store', 'bafkreiweekstore',
     '{}', '2021-01-04 00:00:00.001', ARRAY['urn:decentraland:off-chain:marketplace-stores:0x7777777777777777777777777777777777777777'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x7777777777777777777777777777777777777777","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreiweekstore","signature":"0xf3"}]');

-- I: the last instant of the last window (inclusive end, local), and the
-- remainder, which no file covers.
INSERT INTO content.deployments
    (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
VALUES
    ('0x8888888888888888888888888888888888888888', 'v3', 'profile', 'bafkreidayendprofile',
     '{}', '2021-01-07 00:00:00', ARRAY['0x8888888888888888888888888888888888888888'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x8888888888888888888888888888888888888888","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreidayendprofile","signature":"0xf4"}]'),
    ('0x9999999999999999999999999999999999999999', 'v3', 'profile', 'bafkreiremainder',
     '{}', '2021-01-07 01:00:00', ARRAY['0x9999999999999999999999999999999999999999'],
     '2021-01-07 06:00:00',
     '[{"type":"SIGNER","payload":"0x9999999999999999999999999999999999999999","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"bafkreiremainder","signature":"0xf5"}]');
INSERT INTO content.local_entities (entity_id, signer, origin, published_at)
VALUES ('bafkreidayendprofile', '0x8888888888888888888888888888888888888888', 'land-publish', '2021-01-07 06:00:00');
