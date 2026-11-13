"""Referral and global notification regressions against the actual target schema."""
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
