"""Comms moderation imports use runtime scene identities, never Places UUID keys."""


class CommsMigrationCases:
    def comms_load(self, step):
        from tooling.tests import migration_harness as harness
        return (harness.migrate.SOURCES / 'comms-gatekeeper' / f'{step}.load.sql').read_text()

    def comms_fixture(self):
        db = self.catalyst_db('worlds_fix_comms')
        self.sql(db, '''
            INSERT INTO content.deployments (deployer_address,version,entity_type,entity_id,
                entity_timestamp,entity_pointers,local_timestamp,auth_chain)
            VALUES ('owner','v3','scene','genesis-entity',now(),'{0,0}',now(),'[]');
            INSERT INTO content.active_pointers VALUES ('0,0','genesis-entity','scene');
            INSERT INTO explore.place(id,base_position,raw) VALUES
                ('11111111-1111-4111-8111-111111111111','0,0','{"deployment_id":"genesis-entity"}');
            INSERT INTO worlds.worlds(name) VALUES ('test.eth');
            INSERT INTO worlds.world_scenes(world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size)
            VALUES ('test.eth','world-entity','[]','{"metadata":{"scene":{"base":"10,0"}}}',
                    'owner',ARRAY['2,0','10,0'],0);
            INSERT INTO explore.world_scene_place(world_name,identity,entity_id,parcels) VALUES
                ('test.eth','22222222-2222-4222-8222-222222222222','world-entity',ARRAY['2,0','10,0']);
        ''')
        return db

    def comms_stage(self, step, identity=None):
        ids = [identity] if identity else ['11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222']
        admin = step == 'scene_admin'
        columns = 'admin_address text, added_by text, added_at timestamptz, active boolean' if admin else 'banned_address text, banned_by text, banned_at timestamptz'
        rows = ','.join("('%s','0x%s','operator','2026-01-01'%s)" % (value, 'a'*40, ',true' if admin else '') for value in ids)
        return f'CREATE TEMP TABLE stage(place_id text, {columns}) ON COMMIT DROP; INSERT INTO stage VALUES {rows};\n'

    def test_comms_moderation_resolves_genesis_and_world_runtime_keys(self):
        db = self.comms_fixture()
        for step in ('scene_admin','scene_ban'):
            self.sql(db, 'BEGIN;\n' + self.comms_stage(step) + self.comms_load(step) + '\nCOMMIT;')
        self.assertEqual(self.sql(db,'SELECT place_id FROM social.scene_admins ORDER BY place_id;').strip(), 'genesis-entity\nworld-entity')
        self.assertEqual(self.sql(db,'SELECT place_id FROM social.scene_bans ORDER BY place_id;').strip(), 'genesis-entity\ntest.eth:10,0')
        self.sql(db, "DELETE FROM social.scene_bans; UPDATE worlds.world_scenes SET entity='{}';")
        self.sql(db, 'BEGIN;\n' + self.comms_stage('scene_ban') + self.comms_load('scene_ban') + '\nCOMMIT;')
        self.assertEqual(self.sql(db,'SELECT place_id FROM social.scene_bans ORDER BY place_id;').strip(), 'genesis-entity\ntest.eth:2,0')

    def test_comms_moderation_aborts_on_unresolved_or_ambiguous_identity(self):
        from tooling.tests import migration_harness as harness
        db = self.comms_fixture()
        for step in ('scene_admin','scene_ban'):
            for identity in ('missing.eth','33333333-3333-4333-8333-333333333333'):
                with self.subTest(step=step,identity=identity):
                    with self.assertRaisesRegex(harness.migrate.MigrationError,'unresolved or ambiguous'):
                        self.sql(db,'BEGIN;\n'+self.comms_stage(step,identity)+self.comms_load(step)+'\nCOMMIT;')
        self.sql(db, '''INSERT INTO explore.place(id,base_position,raw) VALUES
            ('22222222-2222-4222-8222-222222222222','0,0','{"deployment_id":"genesis-entity"}');''')
        with self.assertRaisesRegex(harness.migrate.MigrationError,'unresolved or ambiguous'):
            self.sql(db,'BEGIN;\n'+self.comms_stage('scene_ban')+self.comms_load('scene_ban')+'\nCOMMIT;')
        self.assertEqual(self.sql(db,'SELECT count(*) FROM social.scene_bans;').strip(),'0')

    def test_comms_moderation_handles_aliases_and_multiple_batches_in_one_transaction(self):
        db = self.comms_fixture()
        self.sql(db, '''INSERT INTO explore.place(id,base_position,raw) VALUES
            ('33333333-3333-4333-8333-333333333333','0,0','{"deployment_id":"genesis-entity"}');''')
        for step in ('scene_admin','scene_ban'):
            stage = self.comms_stage(step)
            stage += "INSERT INTO stage SELECT '33333333-3333-4333-8333-333333333333', "
            stage += ('admin_address, added_by, added_at, active' if step == 'scene_admin'
                      else 'banned_address, banned_by, banned_at')
            stage += " FROM stage WHERE place_id='11111111-1111-4111-8111-111111111111';\n"
            load = self.comms_load(step)
            self.sql(db, 'BEGIN;\n' + stage + load + '\n' + load + '\nCOMMIT;')
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM social.scene_admins;').strip(), '2')
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM social.scene_bans;').strip(), '2')

    def test_comms_alias_precedence_survives_requested_single_row_batches(self):
        import os
        from unittest import mock
        from tooling.tests import migration_harness as harness
        db = self.comms_fixture()
        src = self.fresh_db('worlds_fix_comms_source')
        self.sql(db, '''INSERT INTO explore.place(id,base_position,raw) VALUES
            ('33333333-3333-4333-8333-333333333333','0,0','{"deployment_id":"genesis-entity"}');''')
        self.sql(src, '''
            CREATE TABLE scene_admin (id text, place_id text, admin text, added_by text,
                created_at bigint, active boolean);
            INSERT INTO scene_admin VALUES
                ('first','11111111-1111-4111-8111-111111111111','0x' || repeat('a',40),'active',1000,true),
                ('later','33333333-3333-4333-8333-333333333333','0x' || repeat('a',40),'revoked',2000,false);
            CREATE TABLE scene_bans (id text, place_id text, banned_address text,
                banned_by text, banned_at bigint);
            INSERT INTO scene_bans VALUES
                ('first','11111111-1111-4111-8111-111111111111','0x' || repeat('a',40),'earliest',1000),
                ('later','33333333-3333-4333-8333-333333333333','0x' || repeat('a',40),'later',2000);
        ''')
        self.mark_parents_completed(db, 'comms-gatekeeper')
        with mock.patch.dict(os.environ, {'COMMS_GATEKEEPER_DATABASE_URL': self.url(src),
                                         'TARGET_DATABASE_URL': self.url(db)}):
            for step in ('scene_admin', 'scene_ban'):
                code, output = self.migrate('run', 'comms-gatekeeper', '--step', step,
                                            '--batch-size', '1', sources_dir=harness.migrate.SOURCES)
                self.assertEqual(code, 0, output)
        self.assertEqual(self.sql(db, 'SELECT place_id, active, added_by FROM social.scene_admins;').strip(),
                         'genesis-entity\tt\tactive')
        self.assertEqual(self.sql(db, 'SELECT place_id, banned_by FROM social.scene_bans;').strip(),
                         'genesis-entity\tearliest')

    # ── re-runs keep moderation and credential changes made on the node ──

    def comms_rows(self, step, columns, rows):
        stage = f'CREATE TEMP TABLE stage({columns}) ON COMMIT DROP; INSERT INTO stage VALUES {rows};\n'
        return 'BEGIN;\n' + stage + self.comms_load(step) + '\nCOMMIT;'

    USER_BAN_COLUMNS = ('id uuid, banned_address text, banned_by text, reason text, custom_message text, '
                        'banned_device_id text, banned_at timestamptz, expires_at timestamptz, '
                        'lifted_at timestamptz, lifted_by text, created_at timestamptz')

    def user_ban_row(self, id_, lifted=None):
        lifted_at, lifted_by = (f"'{lifted}'", "'upstream'") if lifted else ('NULL', 'NULL')
        return (f"('{id_}','0x{'b' * 40}','mod','spam',NULL,NULL,'2026-01-01Z',NULL,"
                f"{lifted_at},{lifted_by},'2026-01-01Z')")

    def test_comms_user_ban_rerun_keeps_a_lift_made_on_the_node(self):
        db = self.catalyst_db('comms_user_ban_lift')
        ban = '44444444-4444-4444-8444-444444444444'
        self.sql(db, self.comms_rows('user_ban', self.USER_BAN_COLUMNS, self.user_ban_row(ban)))
        self.sql(db, f"UPDATE social.user_bans SET lifted_at = '2026-02-01', lifted_by = 'node' WHERE id = '{ban}';")
        self.sql(db, self.comms_rows('user_ban', self.USER_BAN_COLUMNS, self.user_ban_row(ban)))
        self.assertEqual(self.sql(db, 'SELECT lifted_at, lifted_by FROM social.user_bans;').strip(),
                         '2026-02-01 00:00:00\tnode')

    def test_comms_user_ban_rerun_applies_an_upstream_lift(self):
        db = self.catalyst_db('comms_user_ban_upstream_lift')
        ban = '44444444-4444-4444-8444-444444444444'
        self.sql(db, self.comms_rows('user_ban', self.USER_BAN_COLUMNS, self.user_ban_row(ban)))
        self.sql(db, self.comms_rows('user_ban', self.USER_BAN_COLUMNS, self.user_ban_row(ban, '2026-03-01Z')))
        self.assertEqual(self.sql(db, 'SELECT lifted_at, lifted_by FROM social.user_bans;').strip(),
                         '2026-03-01 00:00:00\tupstream')

    def scene_admin_run(self, db, added_at, active):
        stage = ("CREATE TEMP TABLE stage(place_id text, admin_address text, added_by text, added_at timestamptz, "
                 "active boolean) ON COMMIT DROP;\n"
                 f"INSERT INTO stage VALUES ('11111111-1111-4111-8111-111111111111','0x{'a' * 40}','operator',"
                 f"'{added_at}',{'true' if active else 'false'});\n")
        self.sql(db, 'BEGIN;\n' + stage + self.comms_load('scene_admin') + '\nCOMMIT;')

    def scene_admin_state(self, db):
        return self.sql(db, "SELECT active, added_at, COALESCE(removed_at::text, '-') FROM social.scene_admins;").strip()

    def test_comms_scene_admin_rerun_keeps_a_revocation_made_on_the_node(self):
        db = self.comms_fixture()
        self.scene_admin_run(db, '2026-01-01Z', True)
        self.sql(db, "UPDATE social.scene_admins SET active = false, removed_at = '2026-02-01';")
        self.scene_admin_run(db, '2026-01-01Z', True)
        self.assertEqual(self.scene_admin_state(db), 'f\t2026-01-01 00:00:00\t2026-02-01 00:00:00')

    def test_comms_scene_admin_rerun_keeps_a_grant_made_on_the_node(self):
        db = self.comms_fixture()
        self.scene_admin_run(db, '2026-01-01Z', False)
        self.sql(db, "UPDATE social.scene_admins SET active = true, removed_at = NULL, added_at = '2026-02-01';")
        self.scene_admin_run(db, '2026-01-01Z', False)
        self.assertEqual(self.scene_admin_state(db), 't\t2026-02-01 00:00:00\t-')

    def test_comms_scene_admin_rerun_applies_upstream_changes_newer_than_the_node(self):
        db = self.comms_fixture()
        self.scene_admin_run(db, '2026-01-01Z', True)
        self.scene_admin_run(db, '2026-01-01Z', False)
        self.assertEqual(self.scene_admin_state(db), 'f\t2026-01-01 00:00:00\t2026-01-01 00:00:00')
        self.sql(db, "UPDATE social.scene_admins SET removed_at = '2026-02-01';")
        self.scene_admin_run(db, '2026-03-01Z', True)
        self.assertEqual(self.scene_admin_state(db), 't\t2026-03-01 00:00:00\t-')

    STREAM_COLUMNS = ('id uuid, place_id text, streaming_url text, streaming_key text, ingress_id text, room_id text, '
                      'generated_by text, created_at timestamptz, expiration_time timestamptz, streaming boolean, '
                      'streaming_start_time timestamptz, streaming_state_version bigint, '
                      'streaming_checked_at timestamptz, active boolean, cleanup_pending boolean, '
                      'cleanup_expired boolean')

    def stream_row(self, id_, created_at, active=True, streaming=False, version=0):
        return (f"('{id_}','place','rtmp://x','key-{id_[:4]}','ingress-{id_[:4]}','room','0x{'c' * 40}',"
                f"'{created_at}',NULL,{str(streaming).lower()},NULL,{version},NULL,{str(active).lower()},false,false)")

    def node_credential(self, db, id_, created_at):
        self.sql(db, "INSERT INTO social.scene_stream_access (id, place_id, streaming_url, streaming_key, ingress_id, "
                     f"generated_by, created_at) VALUES ('{id_}','place','rtmp://x','node-key','node-ingress',"
                     f"'0x{'c' * 40}','{created_at}');")

    def stream_state(self, db):
        return self.sql(db, 'SELECT left(id::text, 4), active FROM social.scene_stream_access ORDER BY id;').strip()

    def cleanups(self, db):
        return self.sql(db, 'SELECT left(access_id::text, 4), ingress_id, notification_kind IS NULL '
                            'FROM social.scene_stream_cleanup ORDER BY access_id;').strip()

    def test_comms_stream_access_rerun_keeps_a_credential_the_node_retired(self):
        db = self.catalyst_db('comms_stream_retired')
        upstream, node = '55555555-5555-4555-8555-555555555555', '66666666-6666-4666-8666-666666666666'
        self.sql(db, self.comms_rows('scene_stream_access', self.STREAM_COLUMNS,
                                     self.stream_row(upstream, '2026-01-01Z', streaming=True)))
        self.sql(db, f"UPDATE social.scene_stream_access SET active = false, streaming = false WHERE id = '{upstream}';")
        self.node_credential(db, node, '2026-02-01')
        self.sql(db, self.comms_rows('scene_stream_access', self.STREAM_COLUMNS,
                                     self.stream_row(upstream, '2026-01-01Z', streaming=True)))
        self.assertEqual(self.stream_state(db), '5555\tf\n6666\tt')
        self.assertEqual(self.sql(db, f"SELECT streaming FROM social.scene_stream_access WHERE id = '{upstream}';")
                         .strip(), 'f')
        self.assertEqual(self.cleanups(db), '')

    def test_comms_stream_access_retiring_an_older_node_credential_queues_its_ingress_cleanup(self):
        db = self.catalyst_db('comms_stream_retire_cleanup')
        node, upstream = '66666666-6666-4666-8666-666666666666', '55555555-5555-4555-8555-555555555555'
        self.node_credential(db, node, '2026-01-01')
        self.sql(db, self.comms_rows('scene_stream_access', self.STREAM_COLUMNS,
                                     self.stream_row(upstream, '2026-02-01Z')))
        self.assertEqual(self.stream_state(db), '5555\tt\n6666\tf')
        self.assertEqual(self.cleanups(db), '6666\tnode-ingress\tt')

    def test_comms_stream_access_keeps_a_newer_node_credential_and_cleans_the_superseded_import(self):
        db = self.catalyst_db('comms_stream_newer_node')
        node, upstream = '66666666-6666-4666-8666-666666666666', '55555555-5555-4555-8555-555555555555'
        self.node_credential(db, node, '2026-03-01')
        for _ in range(2):
            self.sql(db, self.comms_rows('scene_stream_access', self.STREAM_COLUMNS,
                                         self.stream_row(upstream, '2026-02-01Z')))
        self.assertEqual(self.stream_state(db), '5555\tf\n6666\tt')
        self.assertEqual(self.cleanups(db), '5555\tingress-5555\tt')

    def test_comms_stream_access_rerun_keeps_newer_node_streaming_state(self):
        db = self.catalyst_db('comms_stream_version')
        upstream = '55555555-5555-4555-8555-555555555555'
        self.sql(db, self.comms_rows('scene_stream_access', self.STREAM_COLUMNS,
                                     self.stream_row(upstream, '2026-01-01Z', version=1)))
        self.sql(db, "UPDATE social.scene_stream_access SET streaming = true, streaming_state_version = 5;")
        self.sql(db, self.comms_rows('scene_stream_access', self.STREAM_COLUMNS,
                                     self.stream_row(upstream, '2026-01-01Z', version=1)))
        self.assertEqual(self.sql(db, 'SELECT streaming, streaming_state_version FROM social.scene_stream_access;')
                         .strip(), 't\t5')
