"""Social, referral and global notification imports against the actual target schema."""
import os
from unittest.mock import patch


class SocialMigrationCases:
    def social_referral_dbs(self, service='social-service-ea'):
        src = self.fresh_db('social_fix_src')
        tgt = self.catalyst_db('social_fix_tgt')
        self.sql(src, '''CREATE TABLE referral_progress (
            id uuid PRIMARY KEY, invited_user text, referrer text, status text,
            signed_up_at bigint, tier_granted boolean NOT NULL, tier_granted_at bigint,
            created_at bigint, updated_at bigint);
            CREATE TABLE referral_reward_images (
            id uuid PRIMARY KEY, referrer text NOT NULL, reward_image_url text NOT NULL,
            tier integer NOT NULL, created_at bigint NOT NULL);''')
        if service == 'social-service-ea':
            self.sql(src, 'ALTER TABLE referral_progress ADD COLUMN invited_user_ip text;')
        env_name = 'SOCIAL_SERVICE_EA_DATABASE_URL' if service == 'social-service-ea' else 'REFERRAL_SERVER_DATABASE_URL'
        patcher = patch.dict(os.environ, {env_name: self.url(src), 'TARGET_DATABASE_URL': self.url(tgt)})
        patcher.start()
        self.addCleanup(patcher.stop)
        return src, tgt

    def test_when_importing_referrals_it_should_preserve_quota_tiers_and_transition_times(self):
        from tooling.tests import migration_harness as harness
        src, tgt = self.social_referral_dbs()
        invited, referrer = self.wallet('a'), self.wallet('b')
        self.sql(src, f"""INSERT INTO referral_progress VALUES (
            '00000000-0000-4000-8000-000000000001', '{invited}', '{referrer}', 'signed_up',
            1767225601000, true, 1767225602000, 1767225600000, 1767225603000, '192.0.2.1');""")
        code, out = self.migrate('run', 'social-service-ea', '--step', 'referral_progress', sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, '''SELECT status, tier_granted, invited_user_ip,
            signed_up_at, tier_granted_at FROM social.referral_progress;''').strip(),
            'tier_granted\tt\t192.0.2.1\t2026-01-01 00:00:01\t2026-01-01 00:00:02')
        self.assertEqual(self.sql(tgt, f"SELECT count(*) FILTER (WHERE tier_granted), count(*) FILTER (WHERE invited_user_ip = '192.0.2.1') FROM social.referral_progress WHERE referrer = '{referrer}';").strip(), '1\t1')
        self.sql(tgt, "UPDATE social.referral_progress SET reward_paid = '{\"receipt\":true}', login_days = '{2026-01-01}';")
        self.sql(src, "UPDATE referral_progress SET status = 'pending', tier_granted = false, signed_up_at = NULL, tier_granted_at = NULL;")
        code, out = self.migrate('run', 'social-service-ea', '--step', 'referral_progress', '--restart', sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, '''SELECT status, tier_granted, reward_paid ->> 'receipt',
            cardinality(login_days), reward_requested_at IS NULL, reward_retry_at IS NULL
            FROM social.referral_progress;''').strip(), 'tier_granted\tt\ttrue\t1\tt\tt')

    def test_when_legacy_referrals_have_equal_status_it_should_repair_granted_state_without_rebinding(self):
        from tooling.tests import migration_harness as harness
        src, tgt = self.social_referral_dbs('referral-server')
        invited, referrer = self.wallet('a'), self.wallet('b')
        self.sql(src, f"""INSERT INTO referral_progress VALUES (
            '00000000-0000-4000-8000-000000000001', '{invited}', '{referrer}', 'tier_granted',
            1767225601000, true, 1767225602000, 1767225600000, 1767225603000);""")
        self.sql(tgt, f"""INSERT INTO social.referral_progress (invited_user, referrer, status, invited_user_ip)
            VALUES ('{invited}', '{referrer}', 'tier_granted', '192.0.2.2');""")
        self.mark_parents_completed(tgt, 'referral-server')
        code, out = self.migrate('run', 'referral-server', '--step', 'referral_progress', sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, 'SELECT tier_granted, invited_user_ip, tier_granted_at FROM social.referral_progress;').strip(),
                         't\t192.0.2.2\t2026-01-01 00:00:02')
        self.sql(tgt, f"UPDATE social.referral_progress SET referrer = '{self.wallet('c')}', status = 'rejected_ip_match';")
        code, out = self.migrate('run', 'referral-server', '--step', 'referral_progress', '--restart', sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, 'SELECT referrer, status FROM social.referral_progress;').strip(),
                         f"{self.wallet('c')}\trejected_ip_match")

    def test_when_importing_reward_images_it_should_keep_all_snapshots_without_reissuing_rewards(self):
        from tooling.tests import migration_harness as harness
        src, tgt = self.social_referral_dbs()
        self.sql(src, f"""INSERT INTO referral_reward_images VALUES
            ('00000000-0000-4000-8000-000000000001', '{self.wallet('a').upper()}', 'https://example.com/1.png', 1, 1767225600000),
            ('00000000-0000-4000-8000-000000000002', '{self.wallet('a')}', 'https://example.com/2.png', 1, 1767225601000);""")
        for flags in [(), ('--restart',)]:
            code, out = self.migrate('run', 'social-service-ea', '--step', 'referral_reward_images', *flags,
                                     sources_dir=harness.migrate.SOURCES)
            self.assertEqual(code, 0, out)
        self.assertEqual(self.count(tgt, 'social.referral_reward_images'), 2)
        self.assertEqual(self.sql(tgt, 'SELECT DISTINCT referrer FROM social.referral_reward_images;').strip(), self.wallet('a'))
        self.assertEqual(self.count(tgt, 'social.referral_progress'), 0)
        self.assertEqual(self.count(tgt, 'social.notification_deliveries'), 0)

    def test_when_converting_old_broadcast_copies_it_should_preserve_reads_and_unrelated_rows(self):
        from tooling.tests import migration_harness as harness
        _, tgt = self.notifications_dbs()
        wallet = self.wallet('a')
        self.sql(tgt, f"""INSERT INTO social.notifications
            (id, address, type, metadata, broadcast_key, occurred_at, read, read_at) VALUES
            (md5('{self.BROADCAST}:' || '{wallet}')::uuid, '{wallet}', 'announcement', '{{}}',
             'notifications-workers:{self.BROADCAST}', '2026-01-01', true, '2026-01-02'),
            ('00000000-0000-4000-8000-000000000099', '{self.wallet('b')}', 'announcement', '{{}}',
             'notifications-workers:{self.BROADCAST}', '2026-01-01', false, NULL);""")
        for flags in [(), ('--restart',)]:
            code, out = self.migrate('run', 'notifications-workers', '--step', 'broadcast', *flags,
                                     sources_dir=harness.migrate.SOURCES)
            self.assertEqual(code, 0, out)
        self.assertEqual(self.count(tgt, 'social.notifications'), 1)
        self.assertEqual(self.sql(tgt, 'SELECT address, read_at FROM social.broadcast_reads;').strip(),
                         f'{wallet}\t2026-01-02 00:00:00')
        self.assertEqual(self.count(tgt, 'social.broadcast_notifications'), 1)
        self.assertEqual(self.count(tgt, 'social.notification_deliveries'), 0)

    # ── social-service-ea: node writes survive re-runs, upstream deletions reach the node ──

    SOCIAL_EA_SCHEMA = """
        CREATE TABLE social_settings (address text PRIMARY KEY, private_messages_privacy text,
          blocked_users_messages_visibility text, show_situation_reactions text);
        CREATE TABLE friendships (id uuid PRIMARY KEY, address_requester text, address_requested text,
          is_active boolean, created_at timestamp, updated_at timestamp);
        CREATE TABLE friendship_actions (id uuid PRIMARY KEY, friendship_id uuid, action text, acting_user text,
          metadata json, "timestamp" timestamp);
        CREATE TABLE blocks (id uuid, blocker_address text, blocked_address text, blocked_at timestamp);
        CREATE TABLE user_mutes (muter_address text, muted_address text, muted_at timestamp);
        CREATE TABLE communities (id uuid PRIMARY KEY, name text, description text, owner_address text,
          private boolean, active boolean, unlisted boolean, editors_choice boolean,
          created_at timestamp, updated_at timestamp);
        CREATE TABLE community_members (community_id uuid, member_address text, role text, joined_at timestamp);
        CREATE TABLE community_bans (community_id uuid, banned_address text, banned_by text, banned_at timestamp,
          reason text, active boolean, unbanned_by text, unbanned_at timestamp);
        CREATE TABLE community_requests (id uuid, community_id uuid, member_address text, type text, status text,
          created_at timestamp, updated_at timestamp);
        CREATE TABLE community_posts (id uuid, community_id uuid, author_address text, content text,
          created_at timestamp);
    """
    SOCIAL_COMMUNITY = '00000000-0000-4000-8000-00000000c0c1'

    def social_ea_dbs(self):
        src = self.fresh_db('social_ea_src')
        tgt = self.catalyst_db('social_ea_tgt')
        self.sql(src, self.SOCIAL_EA_SCHEMA)
        patcher = patch.dict(os.environ, {'SOCIAL_SERVICE_EA_DATABASE_URL': self.url(src),
                                          'TARGET_DATABASE_URL': self.url(tgt)})
        patcher.start()
        self.addCleanup(patcher.stop)
        return src, tgt

    def social_ea_run(self, *steps, restart=True):
        from tooling.tests import migration_harness as harness
        args = ['run', 'social-service-ea']
        for step in steps:
            args += ['--step', step]
        code, out = self.migrate(*args, *(['--restart'] if restart else []), sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        return out

    def social_ea_community(self, src, owner):
        self.sql(src, f"""INSERT INTO communities VALUES ('{self.SOCIAL_COMMUNITY}', 'c', '', '{owner}',
            false, true, false, false, '2026-01-01', '2026-01-01');""")

    def test_when_the_node_changed_imported_settings_it_should_keep_them_on_a_rerun(self):
        src, tgt = self.social_ea_dbs()
        changed, reset, untouched = self.wallet('a'), self.wallet('b'), self.wallet('c')
        self.sql(src, f"""INSERT INTO social_settings VALUES
            ('{changed}', 'only_friends', 'show_messages', 'show'),
            ('{reset}', 'all', 'show_messages', 'show'),
            ('{untouched}', 'only_friends', 'show_messages', 'show');""")
        self.social_ea_run('user_settings')
        self.sql(tgt, f"""UPDATE social.user_settings SET private_messages_privacy = 'all' WHERE address = '{changed}';
            DELETE FROM social.user_settings WHERE address = '{reset}';""")
        self.sql(src, "UPDATE social_settings SET show_situation_reactions = 'hide';")
        self.social_ea_run('user_settings')
        self.assertEqual(self.sql(tgt, 'SELECT address, private_messages_privacy, show_situation_reactions '
                                       'FROM social.user_settings ORDER BY address;').strip().splitlines(),
                         [f'{changed}\tall\tshow', f'{untouched}\tonly_friends\thide'])

    def test_when_the_node_lifted_an_imported_ban_it_should_keep_the_lift_and_the_rejoined_member(self):
        src, tgt = self.social_ea_dbs()
        owner, banned, rebanned = self.wallet('a'), self.wallet('b'), self.wallet('c')
        self.social_ea_community(src, owner)
        self.sql(src, f"""INSERT INTO community_members VALUES ('{self.SOCIAL_COMMUNITY}', '{owner}', 'owner', '2026-01-01');
            INSERT INTO community_bans VALUES
            ('{self.SOCIAL_COMMUNITY}', '{banned}', '{owner}', '2026-01-02', NULL, true, NULL, NULL),
            ('{self.SOCIAL_COMMUNITY}', '{rebanned}', '{owner}', '2026-01-02', NULL, false, '{owner}', '2026-01-03');""")
        steps = ('communities', 'community_members', 'community_bans')
        self.social_ea_run(*steps)
        self.sql(tgt, f"""UPDATE social.community_bans SET active = false, unbanned_by = '{owner}',
              unbanned_at = timezone('UTC', now()) WHERE banned_address = '{banned}';
            UPDATE social.community_bans SET active = true, unbanned_by = NULL, unbanned_at = NULL
              WHERE banned_address = '{rebanned}';
            INSERT INTO social.community_members VALUES ('{self.SOCIAL_COMMUNITY}', '{banned}', 'member', timezone('UTC', now()));""")
        self.social_ea_run(*steps)
        self.assertEqual(self.sql(tgt, 'SELECT banned_address, active FROM social.community_bans ORDER BY 1;')
                         .strip().splitlines(), [f'{banned}\tf', f'{rebanned}\tt'])
        self.assertEqual(self.sql(tgt, f"SELECT count(*) FROM social.community_members WHERE member_address = '{banned}';")
                         .strip(), '1')

    def test_when_a_batch_resolves_a_request_and_carries_its_replacement_it_should_keep_the_replacement(self):
        src, tgt = self.social_ea_dbs()
        owner, wallet, other = self.wallet('a'), self.wallet('b'), self.wallet('c')
        old, new, accepted = ('00000000-0000-4000-8000-0000000000e1', '00000000-0000-4000-8000-0000000000e2',
                              '00000000-0000-4000-8000-0000000000e3')
        self.social_ea_community(src, owner)
        self.sql(src, f"""INSERT INTO community_requests VALUES
            ('{old}', '{self.SOCIAL_COMMUNITY}', '{wallet}', 'request_to_join', 'pending', '2026-01-01', '2026-01-01'),
            ('{accepted}', '{self.SOCIAL_COMMUNITY}', '{other}', 'invite', 'pending', '2026-01-01', '2026-01-01');""")
        self.social_ea_run('communities', 'community_requests')
        self.sql(tgt, f"UPDATE social.community_requests SET status = 'accepted' WHERE id = '{accepted}';")
        self.sql(src, f"""UPDATE community_requests SET status = 'cancelled', updated_at = '2026-01-02' WHERE id = '{old}';
            INSERT INTO community_requests VALUES
            ('{new}', '{self.SOCIAL_COMMUNITY}', '{wallet}', 'request_to_join', 'pending', '2026-01-02', '2026-01-02');""")
        self.social_ea_run('community_requests')
        self.assertEqual(self.sql(tgt, 'SELECT id, status FROM social.community_requests ORDER BY id;').strip().splitlines(),
                         [f'{old}\tcancelled', f'{new}\tpending', f'{accepted}\taccepted'])

    def test_when_upstream_removes_rows_it_should_delete_the_imported_ones_and_keep_the_nodes(self):
        src, tgt = self.social_ea_dbs()
        owner, kicked, kept, node = self.wallet('a'), self.wallet('b'), self.wallet('c'), self.wallet('d')
        post, edited = '00000000-0000-4000-8000-0000000000f1', '00000000-0000-4000-8000-0000000000f2'
        self.social_ea_community(src, owner)
        self.sql(src, f"""INSERT INTO community_members VALUES
            ('{self.SOCIAL_COMMUNITY}', '{owner}', 'owner', '2026-01-01'),
            ('{self.SOCIAL_COMMUNITY}', '{kicked}', 'member', '2026-01-01'),
            ('{self.SOCIAL_COMMUNITY}', '{kept}', 'member', '2026-01-01');
            INSERT INTO blocks VALUES (NULL, '{owner}', '{kicked}', '2026-01-01'), (NULL, '{owner}', '{kept}', '2026-01-01');
            INSERT INTO user_mutes VALUES ('{owner}', '{kicked}', '2026-01-01'), ('{owner}', '{kept}', '2026-01-01');
            INSERT INTO community_posts VALUES ('{post}', '{self.SOCIAL_COMMUNITY}', '{kicked}', 'hi', '2026-01-01'),
              ('{edited}', '{self.SOCIAL_COMMUNITY}', '{kicked}', 'hello', '2026-01-01');""")
        steps = ('communities', 'community_members', 'blocks', 'blocks_reconcile', 'user_mutes',
                 'user_mutes_reconcile', 'community_posts', 'community_posts_reconcile')
        self.social_ea_run(*steps)
        self.sql(tgt, f"""INSERT INTO social.community_members VALUES ('{self.SOCIAL_COMMUNITY}', '{node}', 'member', '2026-01-05');
            DELETE FROM social.community_members WHERE member_address = '{kept}';
            INSERT INTO social.blocks VALUES ('{owner}', '{node}', '2026-01-05');
            DELETE FROM social.blocks WHERE blocked_address = '{kept}';
            UPDATE social.community_posts SET content = 'edited on the node' WHERE id = '{edited}';""")
        self.sql(src, f"""DELETE FROM community_members WHERE member_address = '{kicked}';
            DELETE FROM blocks WHERE blocked_address = '{kicked}';
            DELETE FROM user_mutes WHERE muted_address = '{kicked}';
            DELETE FROM community_posts;
            INSERT INTO community_posts VALUES ('00000000-0000-4000-8000-0000000000f3', '{self.SOCIAL_COMMUNITY}',
              '{owner}', 'still here', '2026-01-03');""")
        self.social_ea_run(*steps)
        self.assertEqual(self.sql(tgt, 'SELECT member_address, role FROM social.community_members ORDER BY 1;')
                         .strip().splitlines(), [f'{owner}\towner', f'{node}\tmember'])
        self.assertEqual(self.sql(tgt, 'SELECT blocked_address FROM social.blocks;').strip(), node)
        self.assertEqual(self.sql(tgt, 'SELECT muted_address FROM social.user_mutes;').strip(), kept)
        self.assertEqual(self.sql(tgt, 'SELECT content FROM social.community_posts ORDER BY id;').strip().splitlines(),
                         ['edited on the node', 'still here'])

    def test_when_upstream_lists_nothing_it_should_refuse_to_delete_every_imported_member(self):
        from tooling.tests import migration_harness as harness
        src, tgt = self.social_ea_dbs()
        owner = self.wallet('a')
        self.social_ea_community(src, owner)
        self.social_ea_run('communities', 'community_members')
        self.sql(src, 'DELETE FROM communities;')
        code, out = self.migrate('run', 'social-service-ea', '--step', 'community_members',
                                 sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertEqual(self.count(tgt, 'social.community_members'), 1)

    def test_when_friendship_pairs_differ_only_in_case_it_should_keep_the_newest(self):
        src, tgt = self.social_ea_dbs()
        a, b = self.wallet('a'), self.wallet('b')
        self.sql(src, f"""INSERT INTO friendships VALUES
            ('00000000-0000-4000-8000-000000000a01', '{a.upper().replace('0X', '0x')}', '{b}', false, '2026-01-01', '2026-01-01'),
            ('00000000-0000-4000-8000-000000000a02', '{b}', '{a}', true, '2026-01-02', '2026-01-03'),
            ('00000000-0000-4000-8000-000000000a03', '{a}', '{b.upper().replace('0X', '0x')}', false, '2026-01-01', '2026-01-02');
            INSERT INTO friendship_actions VALUES
            ('00000000-0000-4000-8000-000000000b01', '00000000-0000-4000-8000-000000000a01', 'request', '{a}', NULL, '2026-01-01'),
            ('00000000-0000-4000-8000-000000000b02', '00000000-0000-4000-8000-000000000a02', 'request', '{b}', NULL, '2026-01-02'),
            ('00000000-0000-4000-8000-000000000b03', '00000000-0000-4000-8000-000000000a02', 'accept', '{a}', NULL, '2026-01-03');""")
        self.social_ea_run('friendships', 'friendship_actions')
        self.assertEqual(self.sql(tgt, 'SELECT id, is_active FROM social.friendships;').strip(),
                         '00000000-0000-4000-8000-000000000a02\tt')
        self.assertEqual(self.count(tgt, 'social.friendship_actions'), 2)

    def test_when_the_node_holds_a_newer_block_or_mute_it_should_keep_its_time(self):
        src, tgt = self.social_ea_dbs()
        a, b = self.wallet('a'), self.wallet('b')
        self.sql(src, f"""INSERT INTO blocks VALUES (NULL, '{a}', '{b}', '2026-01-01');
            INSERT INTO user_mutes VALUES ('{a}', '{b}', '2026-01-01');""")
        self.sql(tgt, f"""INSERT INTO social.blocks VALUES ('{a}', '{b}', '2026-02-01');
            INSERT INTO social.user_mutes VALUES ('{a}', '{b}', '2026-02-01');""")
        self.social_ea_run('blocks', 'user_mutes')
        self.assertEqual(self.sql(tgt, 'SELECT blocked_at FROM social.blocks UNION ALL SELECT muted_at FROM social.user_mutes;')
                         .strip().splitlines(), ['2026-02-01 00:00:00'] * 2)
