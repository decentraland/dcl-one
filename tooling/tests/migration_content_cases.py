"""Builder, camera-reel, badges and events imports against the schema."""
import contextlib
import io

from tooling.tests import migration_harness as harness

COLLECTION = '11111111-1111-4111-8111-111111111111'
ITEM = '22222222-2222-4222-8222-222222222222'
PROJECT = '33333333-3333-4333-8333-333333333333'
SCENE = '44444444-4444-4444-8444-444444444444'
PHOTO = '55555555-5555-4555-8555-555555555555'
OWNER = '0x' + 'a' * 40


class ContentMigrationCases:
    def content_load(self, db, source, step_name, rows):
        """Stages `rows` (SQL value tuples, in manifest column order) and runs the step's load."""
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / source).steps
                    if s.name == step_name)
        defs = ', '.join(f'"{n}" {t}' for n, t in step.columns)
        values = ', '.join(f'({row})' for row in rows)
        variables = "\\set camera_api_url 'https://camera.example.test'\n" if source == 'camera-reel-service' else ''
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.sql(db, variables + f'BEGIN;\nCREATE TEMP TABLE stage ({defs}) ON COMMIT DROP;\n'
                         f'INSERT INTO stage VALUES {values};\n{step.load.read_text()}\nCOMMIT;\n')
        return err.getvalue()

    def content_db(self, name):
        db = self.catalyst_db(name)
        self.sql(db, harness.migrate.CHECKPOINT_DDL)
        return db

    @staticmethod
    def builder_rows(name, updated):
        collection = (f"'{COLLECTION}', '{name}', '{OWNER}', NULL, NULL, NULL, NULL, false, false, "
                      f"'2026-01-01Z', '{updated}'")
        item = (f"'{ITEM}', '{COLLECTION}', '{OWNER}', '{name}', 'd', NULL, NULL, 'wearable', 'Common', '', '', "
                f"'{{}}', NULL, NULL, NULL, NULL, NULL, false, 'pending', 0, NULL, NULL, '2026-01-01Z', '{updated}'")
        project = (f"'{PROJECT}', '{OWNER}', '{name}', '', '{SCENE}', 1, 1, 0, 0, 0, 0, 0, 0, NULL, false, false, "
                   f"NULL, NULL, false, '2026-01-01Z', '{updated}'")
        return collection, item, project

    def import_builder(self, db, name, updated):
        collection, item, project = self.builder_rows(name, updated)
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.content_load(db, 'builder-server', 'item', [item])
        self.content_load(db, 'builder-server', 'project', [project])

    def builder_names(self, db):
        return self.sql(db, f"""SELECT (SELECT name FROM builder.collections WHERE id = '{COLLECTION}'),
                                       (SELECT name FROM builder.items WHERE id = '{ITEM}'),
                                       (SELECT title FROM builder.projects WHERE id = '{PROJECT}');\n""").strip()

    def test_when_importing_builder_rows_it_should_land_them_with_normalized_values(self):
        db = self.content_db('content_builder_import')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.assertEqual(self.builder_names(db), 'Upstream\tUpstream\tUpstream')
        self.assertEqual(self.sql(db, f"SELECT rarity, price IS NULL, beneficiary IS NULL, is_approved "
                                      f"FROM builder.items WHERE id = '{ITEM}';\n").strip(), 'common\tt\tt\tf')

    def test_when_the_node_wrote_builder_rows_after_upstream_it_should_keep_them_on_reimport(self):
        db = self.content_db('content_builder_node_wins')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, f"""UPDATE builder.collections SET name = 'Node', updated_at = '2026-03-01' WHERE id = '{COLLECTION}';
                         UPDATE builder.items SET name = 'Node', updated_at = '2026-03-01' WHERE id = '{ITEM}';
                         UPDATE builder.projects SET title = 'Node', updated_at = '2026-03-01' WHERE id = '{PROJECT}';\n""")
        self.import_builder(db, 'Upstream again', '2026-02-15Z')
        self.assertEqual(self.builder_names(db), 'Node\tNode\tNode')
        self.import_builder(db, 'Upstream later', '2026-04-01Z')
        self.assertEqual(self.builder_names(db), 'Upstream later\tUpstream later\tUpstream later')

    def camera_rows(self, is_public):
        return [f"'{PHOTO}', '{OWNER}', 'https://camera.example.test/api/images/{PHOTO}-photo.png', "
                f"'https://camera.example.test/api/images/{PHOTO}-thumbnail-photo.png', {is_public}, "
                f"'{{\"userAddress\": \"{OWNER}\"}}', '2026-01-01Z'"]

    def camera_db(self, name):
        db = self.content_db(name)
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     f"('camera-reel-service', '{PHOTO}-photo.png', 'bafkreiimage', 10, 'image/png'),"
                     f"('camera-reel-service', '{PHOTO}-thumbnail-photo.png', 'bafkreithumb', 5, 'image/png');\n")
        return db

    def is_public(self, db):
        return self.sql(db, f"SELECT is_public FROM camera_reel.images WHERE id = '{PHOTO}';\n").strip()

    def test_when_importing_a_photo_it_should_name_its_blob_and_follow_upstream_visibility(self):
        db = self.camera_db('content_camera_import')
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('false'))
        self.assertEqual(self.sql(db, f"SELECT user_address, image_hash, thumbnail_hash, is_public, review_status "
                                      f"FROM camera_reel.images WHERE id = '{PHOTO}';\n").strip(),
                         f'{OWNER}\tbafkreiimage\tbafkreithumb\tf\tok')
        self.assertEqual(self.sql(db, 'SELECT id FROM camera_reel.imported_images;\n').strip(), PHOTO)
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('true'))
        self.assertEqual(self.is_public(db), 't')

    def test_when_the_owner_toggled_visibility_on_the_node_it_should_keep_it_on_reimport(self):
        db = self.camera_db('content_camera_node_wins')
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('false'))
        self.sql(db, f"UPDATE camera_reel.images SET is_public = true WHERE id = '{PHOTO}';\n")
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('false'))
        self.assertEqual(self.is_public(db), 't')

    @staticmethod
    def schedule_row(name, updated):
        return (f"'s1', '{name}', NULL, NULL, NULL, '[]', NULL, NULL, true, '2026-01-01Z', '{updated}'")

    def test_when_importing_schedules_it_should_keep_a_newer_node_row(self):
        db = self.content_db('content_schedules')
        self.content_load(db, 'events', 'schedule', [self.schedule_row('Upstream', '2026-02-01Z')])
        self.sql(db, "UPDATE explore.schedules_local SET name = 'Node', updated_at = '2026-03-01' WHERE id = 's1';\n")
        self.content_load(db, 'events', 'schedule', [self.schedule_row('Upstream again', '2026-02-15Z')])
        name = "SELECT name FROM explore.schedules_local WHERE id = 's1';\n"
        self.assertEqual(self.sql(db, name).strip(), 'Node')
        self.content_load(db, 'events', 'schedule', [self.schedule_row('Upstream later', '2026-04-01Z')])
        self.assertEqual(self.sql(db, name).strip(), 'Upstream later')

    def test_when_importing_profile_permissions_it_should_grant_them_and_report_the_count(self):
        db = self.content_db('content_profile_permissions')
        row = ("'{user}', NULL, false, NULL, NULL, true, false, false, '{perms}', '2026-01-01Z', '2026-01-01Z'")
        notices = self.content_load(db, 'events', 'profile_settings', [
            row.format(user=OWNER, perms='["approve_any_event", "edit_any_profile"]'),
            row.format(user='0x' + 'b' * 40, perms='[]')])
        self.assertIn('importing 2 permission grants (approve_any_event, edit_any_profile)', notices)
        self.assertEqual(self.sql(db, 'SELECT "user", permissions_granted FROM explore.event_profile_settings '
                                      'ORDER BY 1;\n').splitlines(),
                         [f'{OWNER}\tt', f'0x{"b" * 40}\tf'])

    def test_when_a_badge_was_edited_on_the_node_it_should_keep_it_on_reimport(self):
        src, tgt = self.badges_dbs()
        run = ('run', 'badges', '--step', 'catalog')
        self.assertEqual(self.migrate(*run, sources_dir=harness.migrate.SOURCES)[0], 0)
        self.sql(tgt, "UPDATE social.badge_definitions SET name = 'Node name' WHERE id = 'dclcitizen';\n"
                      "UPDATE social.badge_tiers SET tier_name = 'Node tier' WHERE tier_id = 'emotionista-bronze';\n")
        self.sql(src, "UPDATE badge_definitions SET description = 'Upstream edit' "
                      "WHERE id IN ('dclcitizen', 'emotionista', 'open_for_business');\n")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code, out = self.migrate(*run, sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('kept badge(s) dclcitizen, emotionista as edited on the node', err.getvalue())
        self.assertEqual(self.sql(tgt, "SELECT id, name, description FROM social.badge_definitions WHERE id IN "
                                       "('dclcitizen', 'emotionista', 'open_for_business') ORDER BY id;\n")
                         .splitlines(),
                         ['dclcitizen\tNode name\tLanded', 'emotionista\tEmotionista\tEmotes purchased',
                          'open_for_business\tOpen for Business\tUpstream edit'])
        self.assertEqual(self.sql(tgt, "SELECT tier_name FROM social.badge_tiers "
                                       "WHERE tier_id = 'emotionista-bronze';\n").strip(), 'Node tier')

    def test_event_notice_import_bounds_locks_in_batches_and_dry_runs(self):
        src, tgt = self.notifications_dbs()
        self.sql(src, "DELETE FROM notifications WHERE address IS NOT NULL; INSERT INTO notifications "
                     "SELECT md5(g::text)::uuid, 'series', 'events_started', "
                     "'0x' || lpad(to_hex(g),40,'0'), '{}', "
                     "1767225600000, NULL, 1767225600000, 1767225600000 "
                     "FROM generate_series(1,1024) g;")
        self.sql(tgt, """
            CREATE FUNCTION public.check_notice_lock_bound() RETURNS trigger LANGUAGE plpgsql AS $f$
            BEGIN
              IF (SELECT count(*) FROM pg_locks WHERE pid=pg_backend_pid()
                  AND locktype='advisory' AND granted) > 257 THEN
                RAISE EXCEPTION 'event import retained more than 256 wallet bucket locks';
              END IF;
              RETURN NULL;
            END $f$;
            CREATE TRIGGER check_notice_lock_bound BEFORE INSERT ON social.notifications
            FOR EACH STATEMENT EXECUTE FUNCTION public.check_notice_lock_bound();
        """)
        args = ('run', 'notifications-workers', '--step', 'notification', '--batch-size', '4096')
        for dry in (True, False):
            with self.subTest(dry_run=dry):
                code, out = self.migrate(*args, *(('--dry-run',) if dry else ()),
                                         sources_dir=harness.migrate.SOURCES)
                self.assertEqual(code, 0, out)
                self.assertEqual(self.count(tgt, 'social.notifications'), 0 if dry else 1024)
                if dry:
                    self.assertEqual(self.sql(tgt, "SELECT to_regnamespace('dcl_migration') IS NULL;").strip(), 't')
                self.assertEqual(self.count(tgt, 'social.notification_deliveries'), 0)

    def test_event_notice_import_preserves_identity_and_newer_runtime_rows(self):
        db = self.content_db('event_notice_history')
        kinds = ('events_starts_soon', 'events_started', 'event_created',
                 'event_approved', 'event_rejected', 'event_deleted')
        for index, kind in enumerate(kinds):
            source_id = f'00000000-0000-4000-8000-{index:012d}'
            event_id = f'event-{index}'
            self.sql(db, f"INSERT INTO social.notifications(id,address,type,metadata,read,read_at,in_app,occurred_at) "
                         f"VALUES('{source_id}','{OWNER}','{kind}','{{}}',true,'2026-01-01',false,'2026-01-01');")
            row = (f"'{source_id}','{event_id}','{OWNER}','{kind}','{{\"name\":\"Imported\"}}',"
                   "'2026-02-01Z',NULL,'2026-01-01Z','2026-02-01Z'")
            self.content_load(db, 'notifications-workers', 'notification', [row])
            query = (f"SELECT id,metadata->>'eventId',broadcast_key,read,in_app FROM social.notifications "
                     f"WHERE address='{OWNER}' AND type='{kind}';")
            self.assertEqual(self.sql(db, query).strip(),
                             f'{source_id}\t{event_id}\tevent-notice:{kind}:{event_id}\tt\tf')
            self.sql(db, f"UPDATE social.notifications SET metadata=metadata || "
                         "'{\"name\":\"Newer node\",\"occurredAt\":\"2026-03-01T00:00:00Z\"}' "
                         f"WHERE id='{source_id}';")
            self.content_load(db, 'notifications-workers', 'notification', [row])
            self.assertEqual(self.sql(db, f"SELECT metadata->>'name' FROM social.notifications "
                                          f"WHERE id='{source_id}';").strip(), 'Newer node')
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM social.notification_deliveries;').strip(), '0')

    def test_event_attendance_reconcile_preserves_later_local_changes(self):
        db = self.content_db('event_attendance_reconcile')
        self.sql(db, "INSERT INTO explore.event(id,name,raw,origin,approved) "
                     "VALUES('series','Series','{\"approved\":true}','local',true);")
        wallets = ['0x' + c * 40 for c in 'abcdef']
        for wallet in wallets[:4]:
            self.content_load(db, 'events', 'attendance',
                              [f"'series','{wallet}','Imported','2026-01-01Z'"])
        self.sql(db, f"UPDATE explore.event_attendance_local SET action='cancel',signed_at='2026-02-01' "
                     f"WHERE signer='{wallets[1]}';"
                     f"UPDATE explore.event_attendance_local SET signed_at='2026-02-01',signed_payload='{{}}' "
                     f"WHERE signer='{wallets[2]}';"
                     "INSERT INTO explore.event_attendance_local(event_id,signer,action,signed_at) "
                     f"VALUES('series','{wallets[4]}','going','2026-02-01');")
        self.content_load(db, 'events', 'attendance_reconcile', [f"'series','{wallets[3]}'"])
        query = 'SELECT signer,action FROM explore.event_attendance_local ORDER BY signer;'
        expected = '\n'.join(f'{wallet}\t{action}' for wallet, action in
                             [(wallets[1], 'cancel'), (wallets[2], 'going'),
                              (wallets[3], 'going'), (wallets[4], 'going')])
        self.assertEqual(self.sql(db, query).strip(), expected)
        self.content_load(db, 'events', 'attendance_reconcile', [f"'series','{wallets[3]}'"])
        self.assertEqual(self.sql(db, query).strip(), expected)
        # Re-import must not reclaim newer local writes.
        for wallet in wallets[1:3]:
            self.content_load(db, 'events', 'attendance',
                              [f"'series','{wallet}','Imported again','2026-01-01Z'"])
        self.assertEqual(self.sql(db, query).strip(), expected)

    def test_event_notice_import_reuses_a_runtime_carrier_and_merges_case_duplicates(self):
        db = self.content_db('event_notice_carrier')
        runtime = '10000000-0000-4000-8000-000000000001'
        source = '20000000-0000-4000-8000-000000000001'
        self.sql(db, f"INSERT INTO social.notifications(id,address,type,metadata,occurred_at,in_app,read,read_at) "
                     f"VALUES('{runtime}','{OWNER}','event_rejected',"
                     "'{\"eventId\":\"series\",\"occurredAt\":\"2026-03-01T00:00:00Z\",\"name\":\"Runtime\"}',"
                     "'2026-03-01',false,false,NULL);")
        row = (f"'{source}','series','{OWNER}','event_rejected','{{\"name\":\"Old source\"}}',"
               "'2026-01-01Z','2026-02-01Z','2026-01-01Z','2026-01-01Z'")
        self.content_load(db, 'notifications-workers', 'notification', [row])
        self.assertEqual(self.sql(db, "SELECT id,metadata->>'name',in_app,read FROM social.notifications;").strip(),
                         f'{runtime}\tRuntime\tf\tf')
        # Different upstream IDs for one normalized recipient/type/event converge in a single load.
        rows = [(f"'30000000-0000-4000-8000-{i:012d}','second','{OWNER}','events_started',"
                 f"'{{\"name\":\"Version {i}\"}}','2026-01-0{i}Z',NULL,'2026-01-01Z','2026-01-0{i}Z'")
                for i in (1, 2)]
        self.content_load(db, 'notifications-workers', 'notification', rows)
        self.assertEqual(self.sql(db, "SELECT count(*),max(metadata->>'name') FROM social.notifications "
                                     "WHERE type='events_started';").strip(), '1\tVersion 2')

    def test_event_attendance_empty_source_requires_explicit_override(self):
        db = self.content_db('event_attendance_empty')
        self.sql(db, "INSERT INTO explore.event(id,name,raw) VALUES('series','Series','{}');")
        self.content_load(db, 'events', 'attendance', [f"'series','{OWNER}','Imported','2026-01-01Z'"])
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / 'events').steps
                    if s.name == 'attendance_reconcile')
        script = ('BEGIN; CREATE TEMP TABLE stage(event_id text, signer text) ON COMMIT DROP;\n'
                  + step.load.read_text() + '\nCOMMIT;')
        with self.assertRaisesRegex(harness.migrate.MigrationError, 'upstream lists no attendees'):
            self.sql(db, script)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM explore.event_attendance_local;').strip(), '1')
        self.sql(db, "SET dcl_migration.allow_empty='on'; " + script)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM explore.event_attendance_local;').strip(), '0')
