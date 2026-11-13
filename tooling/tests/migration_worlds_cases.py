"""Migration regressions for worlds, world permission scopes and event profile state."""


class WorldsMigrationCases:
    def migration_sql(self, source, step, kind):
        from tooling.tests import migration_harness as harness
        return (harness.migrate.SOURCES / source / f'{step}.{kind}.sql').read_text()

    def permission_fixture(self):
        from tooling.tests import migration_harness as harness
        db = self.catalyst_db('worlds_fix_permissions')
        self.sql(db, harness.migrate.CHECKPOINT_DDL)
        self.sql(db, '''
            CREATE TABLE public.world_permissions (id int, world_name text, permission_type text,
                address text, created_at timestamp, updated_at timestamp);
            CREATE TABLE public.world_permission_parcels (permission_id int, parcel text);
            INSERT INTO worlds.worlds(name) VALUES ('scope.eth');
            INSERT INTO public.world_permissions VALUES
                (1, 'Scope.ETH', 'deployment', '0x' || repeat('A',40), '2026-01-01', '2026-01-01');
        ''')
        return db

    def permission_stage(self):
        return 'CREATE TEMP TABLE stage AS ' + self.migration_sql('worlds-content-server', 'world_permission', 'extract') + ';\n'

    def test_world_permissions_preserve_complete_integer_range_and_empty_scope(self):
        db = self.permission_fixture()
        self.sql(db, "INSERT INTO public.world_permission_parcels VALUES (1, '2147483647,-2147483648'), (1, ' +1000000000, 0 ');")
        load = self.migration_sql('worlds-content-server', 'world_permission', 'load')
        self.sql(db, self.permission_stage() + load)
        self.assertEqual(self.sql(db, 'SELECT parcel FROM worlds.world_permission_parcels ORDER BY parcel;').strip(),
                         '1000000000,0\n2147483647,-2147483648')
        self.sql(db, 'DELETE FROM public.world_permission_parcels;')
        self.sql(db, self.permission_stage() + load)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM worlds.world_permissions;').strip(), '1')
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM worlds.world_permission_parcels;').strip(), '0')

    def test_world_permissions_reject_invalid_scope_without_widening_existing_grant(self):
        from tooling.tests import migration_harness as harness
        db = self.permission_fixture()
        load = self.migration_sql('worlds-content-server', 'world_permission', 'load')
        self.sql(db, "INSERT INTO public.world_permission_parcels VALUES (1, '1,2');")
        self.sql(db, self.permission_stage() + load)
        for parcel in ('bad', '2147483648,0', '-2147483649,0', '1.5,2', '9' * 100 + ',0'):
            with self.subTest(parcel=parcel):
                self.sql(db, f"UPDATE public.world_permission_parcels SET parcel = '{parcel}';")
                with self.assertRaisesRegex(harness.migrate.MigrationError, 'unrepresentable parcel restrictions'):
                    self.sql(db, 'BEGIN;\n' + self.permission_stage() + load + '\nCOMMIT;')
                self.assertEqual(self.sql(db, 'SELECT parcel FROM worlds.world_permission_parcels;').strip(), '1,2')

    def test_world_permissions_delete_grants_revoked_upstream_but_keep_node_grants(self):
        db = self.permission_fixture()
        load = self.migration_sql('worlds-content-server', 'world_permission', 'load')
        b, c, d = ('0x' + ch * 40 for ch in 'bcd')
        self.sql(db, f"""INSERT INTO public.world_permissions VALUES
            (2, 'scope.eth', 'streaming', '{b}', '2026-01-01', '2026-01-01'),
            (3, 'scope.eth', 'deployment', '{c}', '2026-01-01', '2026-01-01');""")
        self.sql(db, self.permission_stage() + load)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM worlds.world_permissions;').strip(), '3')
        # On the node: a new grant, a revocation, and a parcel edit.
        self.sql(db, f"""INSERT INTO worlds.world_permissions (world_name, permission, address)
                VALUES ('scope.eth', 'deployment', '{d}');
            DELETE FROM worlds.world_permissions WHERE address = '{b}';
            UPDATE worlds.world_permissions SET updated_at = '2026-06-01' WHERE address = '{c}';""")
        # Upstream: every grant revoked but the node-revoked one.
        self.sql(db, f"DELETE FROM public.world_permissions WHERE address <> '{b}';")
        self.sql(db, self.permission_stage() + load)
        self.assertEqual(self.sql(db, 'SELECT address FROM worlds.world_permissions ORDER BY address;').strip(),
                         f'{c}\n{d}')
        self.assertEqual(self.sql(db, 'SELECT address FROM dcl_migration.world_permission_loaded;').strip(), b)
        # Upstream re-creates the grant the node revoked: a new grant, imported.
        self.sql(db, "UPDATE public.world_permissions SET created_at = '2026-07-01', updated_at = '2026-07-01';")
        self.sql(db, self.permission_stage() + load)
        self.assertEqual(self.sql(db, f"SELECT count(*) FROM worlds.world_permissions WHERE address = '{b}';").strip(), '1')

    def world_fixture(self, name):
        from tooling.tests import migration_harness as harness
        db = self.catalyst_db(name)
        self.sql(db, harness.migrate.CHECKPOINT_DDL)
        self.sql(db, self.WCS_SCHEMA)
        return db

    def world_stage(self):
        return 'CREATE TEMP TABLE stage AS ' + self.migration_sql('worlds-content-server', 'world', 'extract') + ';\n'

    def test_worlds_changed_on_the_node_keep_their_settings_until_upstream_changes_again(self):
        db = self.world_fixture('worlds_fix_node_settings')
        load = self.migration_sql('worlds-content-server', 'world', 'load')
        x, y = '0x' + 'a' * 40, '0x' + 'b' * 40
        self.sql(db, f"""INSERT INTO public.worlds (name, access, title, settings_version, created_at, updated_at)
            VALUES ('kept.dcl.eth', '{{"type": "allow-list", "wallets": ["{x}"]}}', 'upstream', 1,
                    '2026-01-01', '2026-01-01'),
                   ('bare.dcl.eth', null, 'upstream', 1, '2026-01-01', '2026-01-01');
            -- a node grant edit on a world not imported yet creates a bare row
            INSERT INTO worlds.worlds (name) VALUES ('bare.dcl.eth');""")
        self.sql(db, self.world_stage() + load)
        self.assertEqual(self.sql(db, 'SELECT name, title FROM worlds.worlds ORDER BY name;').strip(),
                         'bare.dcl.eth\tupstream\nkept.dcl.eth\tupstream')
        self.sql(db, f"""UPDATE worlds.worlds SET title = 'node', settings_version = settings_version + 1,
                updated_at = '2026-03-01' WHERE name = 'kept.dcl.eth';
            UPDATE worlds.world_permissions SET address = '{y}' WHERE world_name = 'kept.dcl.eth';""")
        self.sql(db, self.world_stage() + load)
        self.assertEqual(self.sql(db, "SELECT title FROM worlds.worlds WHERE name = 'kept.dcl.eth';").strip(), 'node')
        self.assertEqual(self.sql(db, "SELECT address FROM worlds.world_permissions WHERE world_name = 'kept.dcl.eth';")
                         .strip(), y)
        self.sql(db, "UPDATE public.worlds SET title = 'later', settings_version = 2, updated_at = '2026-04-01';")
        self.sql(db, self.world_stage() + load)
        self.assertEqual(self.sql(db, "SELECT title FROM worlds.worlds WHERE name = 'kept.dcl.eth';").strip(), 'later')
        self.assertEqual(self.sql(db, "SELECT address FROM worlds.world_permissions WHERE world_name = 'kept.dcl.eth';")
                         .strip(), x)

    def test_worlds_with_unknown_access_settings_are_reported(self):
        import contextlib
        import io
        db = self.world_fixture('worlds_fix_coerced_access')
        load = self.migration_sql('worlds-content-server', 'world', 'load')
        self.sql(db, """INSERT INTO public.worlds (name, access, created_at, updated_at) VALUES
            ('weird.dcl.eth', '{"type": "weird"}', '2026-01-01', '2026-01-01'),
            ('nosecret.dcl.eth', '{"type": "shared-secret"}', '2026-01-01', '2026-01-01'),
            ('fine.dcl.eth', '{"type": "allow-list", "wallets": []}', '2026-01-01', '2026-01-01'),
            ('open.dcl.eth', null, '2026-01-01', '2026-01-01');""")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.sql(db, self.world_stage() + load)
        self.assertIn('2 worlds had an unknown or incomplete access setting and became empty allow-lists:'
                      ' nosecret.dcl.eth, weird.dcl.eth', err.getvalue())
        self.assertEqual(self.sql(db, 'SELECT name, access_type FROM worlds.worlds ORDER BY name;').strip(),
                         'fine.dcl.eth\tallow-list\nnosecret.dcl.eth\tallow-list\nopen.dcl.eth\tunrestricted\n'
                         'weird.dcl.eth\tallow-list')

    def test_event_profiles_preserve_email_lifecycle_instants_and_newer_target(self):
        db = self.catalyst_db('worlds_fix_profiles')
        self.sql(db, '''
            CREATE TABLE public.profile_settings ("user" text, email text, email_verified boolean,
                email_verified_at timestamptz, email_updated_at timestamptz,
                use_local_time boolean, notify_by_email boolean, notify_by_browser boolean,
                permissions text[], created_at timestamp, updated_at timestamp);
            INSERT INTO public.profile_settings VALUES
                ('Wallet', 'a@example.org', true, '2026-01-02 10:00:00+03', '2026-01-01 10:00:00-03',
                 true, true, true, '{}', '2026-01-01', '2026-01-03'),
                ('Empty', null, false, null, null, true, false, false, '{}', '2026-01-01', '2026-01-03');
        ''')
        stage = 'CREATE TEMP TABLE stage AS ' + self.migration_sql('events', 'profile_settings', 'extract') + ';\n'
        load = self.migration_sql('events', 'profile_settings', 'load')
        self.sql(db, "SET TIME ZONE 'Pacific/Honolulu';\n" + stage + load)
        self.assertEqual(self.sql(db, '''SELECT email_verified_at, email_updated_at
            FROM explore.event_profile_settings WHERE "user"='wallet';''').strip(),
            '2026-01-02 07:00:00\t2026-01-01 13:00:00')
        self.assertEqual(self.sql(db, '''SELECT email_verified_at IS NULL AND email_updated_at IS NULL
            FROM explore.event_profile_settings WHERE "user"='empty';''').strip(), 't')
        self.sql(db, '''UPDATE explore.event_profile_settings SET email_verified_at=null,
            email_updated_at='2026-02-01', updated_at='2026-02-01' WHERE "user"='wallet';''')
        self.sql(db, stage + load)
        self.assertEqual(self.sql(db, '''SELECT email_verified_at IS NULL, email_updated_at
            FROM explore.event_profile_settings WHERE "user"='wallet';''').strip(), 't\t2026-02-01 00:00:00')

    def test_event_browser_subscriptions_preserve_keys_and_first_import_time(self):
        db = self.catalyst_db('worlds_fix_subscriptions')
        self.sql(db, '''CREATE TABLE public.profile_subscriptions (endpoint text PRIMARY KEY,
            "user" text NOT NULL, p256dh text NOT NULL, auth text NOT NULL);
            INSERT INTO public.profile_subscriptions VALUES ('https://push.example/device', 'Wallet', 'public-key', 'auth-secret');''')
        stage = 'CREATE TEMP TABLE stage AS ' + self.migration_sql('events', 'profile_subscription', 'extract') + ';\n'
        load = self.migration_sql('events', 'profile_subscription', 'load')
        self.sql(db, stage + load)
        before = self.sql(db, 'SELECT * FROM explore.event_profile_subscriptions;')
        self.assertIn('https://push.example/device\twallet\tpublic-key\tauth-secret\t', before)
        self.sql(db, stage + load)
        self.assertEqual(self.sql(db, 'SELECT * FROM explore.event_profile_subscriptions;'), before)
        self.sql(db, "UPDATE public.profile_subscriptions SET p256dh='new-key';")
        self.sql(db, stage + load)
        self.assertEqual(self.sql(db, 'SELECT count(*), min(p256dh) FROM explore.event_profile_subscriptions;').strip(), '1\tnew-key')
