"""Comms moderation imports use runtime scene identities, never Places UUID keys."""


class CommsMigrationCases:
    def comms_load(self, step):
        from tooling.tests import test_migration as harness
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
        from tooling.tests import test_migration as harness
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
