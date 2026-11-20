"""Financial cutover guards, flags, purchases, rentals and favorites lists."""
import contextlib
import dataclasses
import io
import os
import unittest.mock

from tooling.tests.migration_harness import ROOT, migrate


class FinancialMigrationCases:
    def financial_step(self, source, name):
        return next(s for s in migrate.load_source(migrate.SOURCES / source).steps if s.name == name)

    def financial_stage(self, step):
        return 'CREATE TEMP TABLE stage (' + ', '.join(f'"{n}" {t}' for n, t in step.columns) + ');\n'

    def test_when_credit_liability_or_payment_facts_are_unresolved_it_should_block_cutover(self):
        folder = migrate.SOURCES / 'credits-server'
        db = self.fresh_db('financial_fix_guard')
        self.sql(db, '''
            CREATE TABLE usd_purchase_intents(id text, status text, expires_at bigint);
            CREATE TABLE stripe_orders(id text, status text, created_at bigint);
            CREATE TABLE stripe_payment_disputes(dispute_id text, outcome text,
                reconciled_at bigint, updated_at bigint);
            CREATE TABLE stripe_payment_refunds(payment_intent_id text,
                reconciled_at bigint, updated_at bigint);
        ''')
        cases = [
            ("INSERT INTO usd_purchase_intents VALUES ('expired','EXPIRED',1)", 'intent:expired'),
            ("INSERT INTO usd_purchase_intents VALUES ('pending','PENDING',1)", 'intent:pending'),
            ("INSERT INTO stripe_payment_disputes VALUES ('open',NULL,20,10)", 'dispute:open'),
            ("INSERT INTO stripe_payment_disputes VALUES ('unknown','unexpected',20,10)", 'dispute:unknown'),
            ("INSERT INTO stripe_payment_disputes VALUES ('won','won',NULL,10)", 'dispute:won'),
            ("INSERT INTO stripe_payment_disputes VALUES ('stale','lost',9,10)", 'dispute:stale'),
            ("INSERT INTO stripe_payment_refunds VALUES ('refund',9,10)", 'refund:refund'),
        ]
        extract = (folder / 'cutover_guard.extract.sql').read_text()
        load = (folder / 'cutover_guard.load.sql').read_text()
        for insert, ref in cases:
            with self.subTest(ref=ref):
                self.sql(db, 'TRUNCATE usd_purchase_intents, stripe_payment_disputes, stripe_payment_refunds;' + insert + ';')
                self.assertIn(ref, self.sql(db, extract))
                with self.assertRaisesRegex(migrate.MigrationError, 'upstream liabilities'):
                    self.sql(db, 'CREATE TEMP TABLE stage AS ' + extract + ';' + load)
        self.sql(db, '''TRUNCATE usd_purchase_intents, stripe_payment_disputes, stripe_payment_refunds;
            INSERT INTO usd_purchase_intents VALUES ('settled','SETTLED',1);
            INSERT INTO stripe_payment_disputes VALUES ('closed','won',10,10);
            INSERT INTO stripe_payment_refunds VALUES ('refunded',10,10);''')
        self.sql(db, 'CREATE TEMP TABLE stage AS ' + extract + ';' + load)

    def test_when_rental_parent_is_newer_it_should_preserve_listing_and_periods(self):
        db = self.fresh_db('financial_fix_rentals')
        folder = migrate.SOURCES / 'signatures-server'
        self.sql(db, (ROOT / 'schemas/32_signatures.sql').read_text())
        self.sql(db, '''
            INSERT INTO signatures.rental_assets(contract_address,token_id,nft_id,network,chain_id,
                category,search_text,nft_created_at,nft_updated_at,fetched_at)
            VALUES ('asset',1,'asset','ethereum',1,'parcel','','2026-01-01','2026-01-01','2026-01-01');
        ''')
        rental_id = '11111111-1111-1111-1111-111111111111'
        period_id = '22222222-2222-2222-2222-222222222222'
        rental_stage = self.financial_stage(self.financial_step('signatures-server', 'rentals'))
        rental_row = f"""INSERT INTO stage VALUES ('{rental_id}','asset',1,'ethereum',1,'2027-01-01',
            ARRAY['0','0','0'],'signature','contract','executed','target',1,NULL,
            '2026-01-01','2026-11-01',NULL,'lessor','old-tenant');"""
        rental_load = (folder / 'rentals.load.sql').read_text()
        # A fresh insert must create both the parent and its listing in the same CTE.
        self.sql(db, rental_stage + rental_row + rental_load)
        self.assertEqual(self.sql(db, 'SELECT tenant FROM signatures.rental_listings').strip(), 'old-tenant')
        self.sql(db, f"""UPDATE signatures.rentals SET updated_at='2026-11-10',status='claimed';
            UPDATE signatures.rental_listings SET tenant='new-tenant',lessor='new-lessor';
            INSERT INTO signatures.rental_periods VALUES ('{period_id}','{rental_id}',2,4,100);""")
        self.sql(db, rental_stage + rental_row + rental_load)
        self.assertEqual(self.sql(db, 'SELECT status,lessor,tenant FROM signatures.rentals JOIN signatures.rental_listings USING(id)').strip(),
                         'claimed\tnew-lessor\tnew-tenant')
        period_stage = self.financial_stage(self.financial_step('signatures-server', 'rental_periods'))
        period_row = f"INSERT INTO stage VALUES ('{period_id}','{rental_id}',1,2,5,'signature','2026-11-01');"
        period_load = (folder / 'rental_periods.load.sql').read_text()
        self.sql(db, period_stage + period_row + period_load)
        self.assertEqual(self.sql(db, 'SELECT min_days,max_days,price_per_day FROM signatures.rental_periods').strip(), '2\t4\t100')
        # A matching parent version is accepted, including an ordinary re-import.
        self.sql(db, rental_stage + rental_row.replace('2026-11-01', '2026-11-10') + rental_load)
        self.sql(db, period_stage + period_row.replace('2026-11-01', '2026-11-10') + period_load)
        self.assertEqual(self.sql(db, 'SELECT tenant FROM signatures.rental_listings').strip(), 'old-tenant')
        self.assertEqual(self.sql(db, 'SELECT price_per_day FROM signatures.rental_periods').strip(), '5')

    # ── credits-server: flags, wallets, purchases ──

    def credits_load(self, db, step_name, rows, allow_empty=False):
        step = self.financial_step('credits-server', step_name)
        step = dataclasses.replace(step, allow_empty=allow_empty)
        inserts = ''.join(f'INSERT INTO stage VALUES ({row});\n' for row in rows)
        self.sql(db, migrate.CHECKPOINT_DDL + '\n'.join(migrate.step_vars(step, False)) + '\nBEGIN;\n'
                 + self.financial_stage(step) + inserts + step.load.read_text() + '\nCOMMIT;\n')

    def test_when_upstream_flags_a_wallet_it_should_import_only_live_multi_account_flags(self):
        db = self.fresh_db('credits_flag_extract')
        self.sql(db, '''
            CREATE TABLE flagged_wallets (address text, flag_type text, flagged_at timestamp, reason jsonb,
                expires_at timestamp);
            INSERT INTO flagged_wallets VALUES
              ('0xAA00000000000000000000000000000000000001', 'multi-account', '2026-01-01',
               '{"message":"m","ip":"203.0.113.1","linkedAddresses":["0xBB00000000000000000000000000000000000002"]}', NULL),
              ('0x0000000000000000000000000000000000000002', 'captcha', '2026-01-01', '{"message":"c"}', '2999-01-01'),
              ('0x0000000000000000000000000000000000000003', 'multi-account', '2026-01-01', '{"message":"x"}', '2000-01-01');
        ''')
        extract = self.financial_step('credits-server', 'wallet_flag').extract.read_text()
        self.assertEqual(self.sql(db, extract + ';\n').strip(),
                         '0xaa00000000000000000000000000000000000001\t203.0.113.1\t'
                         '{0xbb00000000000000000000000000000000000002}\t2026-01-01 00:00:00+00')

    def test_when_wallets_load_it_should_leave_the_operator_block_alone(self):
        db = self.catalyst_db('credits_wallet_block')
        address = '0x' + 'a' * 40
        self.sql(db, f"INSERT INTO credits.wallets (address, is_blocked_for_claiming, has_started_program) "
                     f"VALUES ('{address}', true, false);\n")
        self.credits_load(db, 'wallet', [f"'{address}', true, '2025-01-01', '2026-01-01'"])
        self.assertEqual(self.sql(db, 'SELECT is_blocked_for_claiming, has_started_program FROM credits.wallets;\n')
                         .strip(), 't\tt')

    def test_when_flags_reload_it_should_drop_lifted_ones_and_keep_node_flags(self):
        db = self.catalyst_db('credits_wallet_flags')
        imported, lifted, node = ('0x' + c * 40 for c in 'abc')
        self.sql(db, f"INSERT INTO credits.wallet_flags (address, source) VALUES ('{node}', 'admin');\n")
        row = "'{a}', '203.0.113.1', ARRAY['{a}'], '2026-01-01'"
        self.credits_load(db, 'wallet_flag', [row.format(a=imported), row.format(a=lifted)])
        flags = 'SELECT address, source FROM credits.wallet_flags ORDER BY address;\n'
        self.assertEqual(self.sql(db, flags).strip().splitlines(),
                         [f'{imported}\tcredits-server:multi-account', f'{lifted}\tcredits-server:multi-account',
                          f'{node}\tadmin'])
        # An operator lifts one on the node; upstream drops the other.
        self.sql(db, f"DELETE FROM credits.wallet_flags WHERE address = '{lifted}';\n"
                     f"INSERT INTO ops.admin_audit (actor, domain, action, target, at) "
                     f"VALUES ('op', 'credits', 'user.unflag', '{lifted}', '2026-02-01');\n")
        self.credits_load(db, 'wallet_flag', [row.format(a=lifted)])
        self.assertEqual(self.sql(db, flags).strip().splitlines(), [f'{node}\tadmin'])
        # A newer upstream flag after the unflag is imported again.
        self.credits_load(db, 'wallet_flag', [row.format(a=lifted).replace('2026-01-01', '2026-03-01')])
        self.assertIn(lifted, self.sql(db, flags))

    def test_when_a_wallet_started_the_program_on_the_node_it_should_keep_it(self):
        db = self.catalyst_db('credits_wallet_node_started')
        started, opted = '0x' + 'a' * 40, '0x' + 'b' * 40
        self.credits_load(db, 'wallet', [f"'{started}', false, '2025-01-01', '2026-01-01'",
                                         f"'{opted}', true, '2025-01-01', '2026-01-01'"])
        self.sql(db, f"UPDATE credits.wallets SET has_started_program = true WHERE address = '{started}';\n")
        # Upstream: the first still has not started; the second opts out.
        self.credits_load(db, 'wallet', [f"'{started}', false, '2025-01-01', '2026-01-01'",
                                         f"'{opted}', false, '2025-01-01', '2026-02-01'"])
        self.assertEqual(self.sql(db, 'SELECT address, has_started_program FROM credits.wallets ORDER BY 1;\n')
                         .strip().splitlines(), [f'{started}\tt', f'{opted}\tf'])

    def test_when_upstream_matches_a_node_start_and_later_reverts_it_should_keep_the_node_start(self):
        db = self.catalyst_db('credits_wallet_node_matched')
        address = '0x' + 'd' * 40
        self.credits_load(db, 'wallet', [f"'{address}', false, '2025-01-01', '2026-01-01'"])
        self.sql(db, f"UPDATE credits.wallets SET has_started_program = true WHERE address = '{address}';\n")
        # Upstream starts too, with an older created_at so the upsert rewrites the row.
        self.credits_load(db, 'wallet', [f"'{address}', true, '2024-01-01', '2026-02-01'"])
        self.credits_load(db, 'wallet', [f"'{address}', false, '2024-01-01', '2026-03-01'"])
        self.assertEqual(self.sql(db, 'SELECT has_started_program FROM credits.wallets;\n').strip(), 't')

    def test_when_a_node_start_predates_an_import_that_matches_it_and_upstream_later_reverts_it_should_keep_it(self):
        db = self.catalyst_db('credits_wallet_node_first_matched')
        address = '0x' + 'e' * 40
        self.sql(db, f"INSERT INTO credits.wallets (address, has_started_program) VALUES ('{address}', true);\n")
        self.credits_load(db, 'wallet', [f"'{address}', true, '2025-01-01', '2026-01-01'"])
        self.credits_load(db, 'wallet', [f"'{address}', false, '2025-01-01', '2026-02-01'"])
        self.assertEqual(self.sql(db, 'SELECT has_started_program FROM credits.wallets;\n').strip(), 't')

    def test_when_a_wallet_started_on_the_node_before_any_import_it_should_keep_it(self):
        db = self.catalyst_db('credits_wallet_node_first')
        address = '0x' + 'c' * 40
        self.sql(db, f"INSERT INTO credits.wallets (address, has_started_program) VALUES ('{address}', true);\n")
        self.credits_load(db, 'wallet', [f"'{address}', false, '2025-01-01', '2026-01-01'"])
        self.assertEqual(self.sql(db, 'SELECT has_started_program FROM credits.wallets;\n').strip(), 't')

    def test_when_upstream_really_lists_no_flags_and_the_operator_allows_it_it_should_delete_the_imported_ones(self):
        db = self.catalyst_db('credits_wallet_flags_allowed_empty')
        imported, node = '0x' + 'a' * 40, '0x' + 'b' * 40
        self.sql(db, f"INSERT INTO credits.wallet_flags (address, source) VALUES ('{node}', 'admin');\n")
        self.credits_load(db, 'wallet_flag', [f"'{imported}', '203.0.113.1', ARRAY['{imported}'], '2026-01-01'"])
        self.credits_load(db, 'wallet_flag', [], allow_empty=True)
        self.assertEqual(self.sql(db, 'SELECT address FROM credits.wallet_flags;\n').strip(), node)

    def test_when_upstream_lists_no_flags_it_should_refuse_to_delete_every_imported_one(self):
        db = self.catalyst_db('credits_wallet_flags_empty')
        address = '0x' + 'a' * 40
        self.credits_load(db, 'wallet_flag', [f"'{address}', '203.0.113.1', ARRAY['{address}'], '2026-01-01'"])
        with self.assertRaisesRegex(migrate.MigrationError, 'refusing to delete'):
            self.credits_load(db, 'wallet_flag', [])
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM credits.wallet_flags;\n').strip(), '1')

    def test_when_the_ledger_creates_a_wallet_it_should_not_have_started_the_program(self):
        db = self.catalyst_db('credits_ledger_wallet')
        address = '0x' + 'd' * 40
        self.credits_load(db, 'ledger', [f"'{address}', 1, 'credits-server:t1', 'grant', 5, '2026-01-01'"])
        self.assertEqual(self.sql(db, 'SELECT available, has_started_program FROM credits.wallets;\n').strip(),
                         '5\tf')

    def test_when_the_node_moved_a_purchase_later_it_should_keep_the_node_row(self):
        db = self.catalyst_db('credits_purchase_newer')
        address = '0x' + 'e' * 40
        row = (f"'order-1', '{address}', 'pack', 10, 100, 1, 'website', 'complete', true, 'cs', 'pi', 0, 0, NULL, "
               "'2026-01-01', '2026-01-02'")
        self.credits_load(db, 'purchase', [row])
        self.sql(db, "UPDATE credits.purchases SET status = 'refunded', refunded_cents = 100, revoked_credits = 10, "
                     "updated_at = '2026-02-01';\n")
        self.credits_load(db, 'purchase', [row])
        self.assertEqual(self.sql(db, 'SELECT status, refunded_cents FROM credits.purchases;\n').strip(),
                         'refunded\t100')
        self.credits_load(db, 'purchase', [row.replace("'2026-01-02'", "'2026-03-01'")])
        self.assertEqual(self.sql(db, 'SELECT status, refunded_cents FROM credits.purchases;\n').strip(), 'paid\t0')

    # ── signatures-server: one open listing per asset ──

    def test_when_a_batch_cancels_a_listing_and_carries_its_open_replacement_it_should_land_both(self):
        db = self.fresh_db('financial_rental_replace')
        self.sql(db, (ROOT / 'schemas/32_signatures.sql').read_text())
        self.sql(db, '''INSERT INTO signatures.rental_assets(contract_address,token_id,nft_id,network,chain_id,
                category,search_text,nft_created_at,nft_updated_at,fetched_at)
            VALUES ('asset',1,'asset','ethereum',1,'parcel','','2026-01-01','2026-01-01','2026-01-01');''')
        stage = self.financial_stage(self.financial_step('signatures-server', 'rentals'))
        load = (migrate.SOURCES / 'signatures-server/rentals.load.sql').read_text()
        old, new = '11111111-1111-1111-1111-111111111111', '33333333-3333-3333-3333-333333333333'

        def row(rental_id, signature, status, updated):
            return (f"INSERT INTO stage VALUES ('{rental_id}','asset',1,'ethereum',1,'2027-01-01',ARRAY['0','0','0'],"
                    f"'{signature}','contract','{status}','target',NULL,NULL,'2026-01-01','{updated}',NULL,'lessor',NULL);")
        self.sql(db, stage + row(old, 'sig-old', 'open', '2026-01-01') + load)
        # The replacement is first in the batch, as a cursor on (updated_at, id) can order it.
        self.sql(db, stage + row(new, 'sig-new', 'open', '2026-02-01') + row(old, 'sig-old', 'cancelled', '2026-02-01')
                 + load)
        self.assertEqual(self.sql(db, 'SELECT id, status FROM signatures.rentals ORDER BY id;\n').strip().splitlines(),
                         [f'{old}\tcancelled', f'{new}\topen'])
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM signatures.rental_listings;\n').strip(), '2')

    def test_when_upstream_updates_a_node_cancellation_it_should_keep_or_clear_its_reason(self):
        db = self.fresh_db('financial_rental_cancel_reason')
        self.sql(db, (ROOT / 'schemas/32_signatures.sql').read_text())
        self.sql(db, '''INSERT INTO signatures.rental_assets(contract_address,token_id,nft_id,network,chain_id,
                category,search_text,nft_created_at,nft_updated_at,fetched_at)
            VALUES ('asset',1,'asset','ethereum',1,'parcel','','2026-01-01','2026-01-01','2026-01-01');''')
        stage = self.financial_stage(self.financial_step('signatures-server', 'rentals'))
        load = (migrate.SOURCES / 'signatures-server/rentals.load.sql').read_text()
        rental_id = '11111111-1111-1111-1111-111111111111'

        def row(status, updated):
            return (f"INSERT INTO stage VALUES ('{rental_id}','asset',1,'ethereum',1,'2027-01-01',ARRAY['0','0','0'],"
                    f"'sig','contract','{status}','target',NULL,NULL,'2026-01-01','{updated}',NULL,'lessor',NULL);")
        self.sql(db, stage + row('open', '2026-01-01') + load)
        self.sql(db, "UPDATE signatures.rentals SET status='cancelled', cancel_reason='expired', updated_at='2026-01-02';")
        self.sql(db, stage + row('cancelled', '2026-01-03') + load)
        self.assertEqual(self.sql(db, 'SELECT status, cancel_reason FROM signatures.rentals;\n').strip(),
                         'cancelled\texpired')
        self.sql(db, stage + row('executed', '2026-01-04') + load)
        self.assertEqual(self.sql(db, "SELECT status, coalesce(cancel_reason, '-') FROM signatures.rentals;\n").strip(),
                         'executed\t-')

    def test_when_a_node_rental_cancellation_is_newer_it_should_skip_an_older_upstream_execution(self):
        db = self.fresh_db('financial_rental_newer_cancel')
        self.sql(db, (ROOT / 'schemas/32_signatures.sql').read_text())
        self.sql(db, '''INSERT INTO signatures.rental_assets(contract_address,token_id,nft_id,network,chain_id,
                category,search_text,nft_created_at,nft_updated_at,fetched_at)
            VALUES ('asset',1,'asset','ethereum',1,'parcel','','2026-01-01','2026-01-01','2026-01-01');''')
        stage = self.financial_stage(self.financial_step('signatures-server', 'rentals'))
        load = (migrate.SOURCES / 'signatures-server/rentals.load.sql').read_text()
        rental_id = '11111111-1111-1111-1111-111111111111'

        def row(status, updated):
            return (f"INSERT INTO stage VALUES ('{rental_id}','asset',1,'ethereum',1,'2027-01-01',ARRAY['0','0','0'],"
                    f"'sig','contract','{status}','target',NULL,NULL,'2026-01-01','{updated}',NULL,'lessor',NULL);")

        def state():
            return self.sql(db, 'SELECT status, cancel_reason FROM signatures.rentals;\n').strip()
        self.sql(db, stage + row('open', '2026-01-01') + load)
        self.sql(db, "UPDATE signatures.rentals SET status='cancelled', cancel_reason='expired', updated_at='2026-01-05';")
        self.sql(db, stage + row('executed', '2026-01-04') + load)
        self.assertEqual(state(), 'cancelled\texpired')
        self.sql(db, stage + row('executed', '2026-01-06') + load)
        self.assertEqual(state().split('\t')[0], 'executed')

    def test_when_a_batch_carries_two_open_listings_of_one_asset_it_should_land_the_latest(self):
        db = self.fresh_db('financial_rental_two_open')
        self.sql(db, (ROOT / 'schemas/32_signatures.sql').read_text())
        self.sql(db, '''INSERT INTO signatures.rental_assets(contract_address,token_id,nft_id,network,chain_id,
                category,search_text,nft_created_at,nft_updated_at,fetched_at)
            VALUES ('asset',1,'asset','ethereum',1,'parcel','','2026-01-01','2026-01-01','2026-01-01');''')
        stage = self.financial_stage(self.financial_step('signatures-server', 'rentals'))
        load = (migrate.SOURCES / 'signatures-server/rentals.load.sql').read_text()
        first, second = '11111111-1111-1111-1111-111111111111', '33333333-3333-3333-3333-333333333333'

        def row(rental_id, signature, updated):
            return (f"INSERT INTO stage VALUES ('{rental_id}','asset',1,'ethereum',1,'2027-01-01',ARRAY['0','0','0'],"
                    f"'{signature}','contract','open','target',NULL,NULL,'2026-01-01','{updated}',NULL,'lessor',NULL);")
        self.sql(db, stage + row(first, 'sig-1', '2026-01-01') + row(second, 'sig-2', '2026-02-01') + load)
        self.assertEqual(self.sql(db, 'SELECT id FROM signatures.rentals;\n').strip(), second)

    # ── marketplace-server: trades and coupons ──

    MARKETPLACE_SCHEMA = '''
        CREATE SCHEMA marketplace;
        CREATE TABLE marketplace.trades (id uuid, network text, chain_id int, signature text, hashed_signature text,
            checks jsonb, signer text, type text, expires_at timestamp, effective_since timestamp, contract text,
            created_at timestamp);
        CREATE TABLE marketplace.trade_assets (id uuid, trade_id uuid, direction text, asset_type smallint,
            contract_address text, beneficiary text, extra text, created_at timestamp);
        CREATE TABLE marketplace.trade_assets_erc20 (asset_id uuid, amount numeric);
        CREATE TABLE marketplace.trade_assets_erc721 (asset_id uuid, token_id text);
        CREATE TABLE marketplace.trade_assets_item (asset_id uuid, item_id text);
        CREATE TABLE marketplace.coupons (id uuid, network text, chain_id int, signer text, signature text,
            hashed_signature text, state_key text, coupon_manager text, coupon_address text, checks jsonb,
            discount_type smallint, discount_ppm int, root text, collections text[], effective_since timestamp,
            expires_at timestamp, created_at timestamp);
        CREATE TABLE marketplace.coupon_state (coupon_id uuid, uses int, cancelled boolean, revoked boolean,
            checked_at timestamp);
    '''

    def test_when_importing_marketplace_rows_it_should_promote_mirrored_trades_and_keep_newer_coupon_state(self):
        src, tgt = self.fresh_db('marketplace_src'), self.catalyst_db('marketplace_tgt')
        self.sql(src, self.MARKETPLACE_SCHEMA)
        trade, coupon = '11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222'
        sent, received = '33333333-3333-4333-8333-333333333333', '44444444-4444-4444-8444-444444444444'
        signer, contract = '0x' + 'a' * 40, '0x' + 'b' * 40
        self.sql(src, f"""
            INSERT INTO marketplace.trades VALUES ('{trade}', 'polygon', 137, '0xABC', '0xDEF', '{{}}',
              '{signer.upper().replace('0X', '0x')}', 'public_item_order', '2027-01-01', '2026-01-01', 'c', '2026-01-02');
            INSERT INTO marketplace.trade_assets VALUES
              ('{sent}', '{trade}', 'sent', 1, '{contract}', NULL, '0x', '2026-01-02'),
              ('{received}', '{trade}', 'received', 4, '{contract}', '{signer}', '', '2026-01-02');
            INSERT INTO marketplace.trade_assets_erc20 VALUES ('{sent}', 1000000000000000000);
            INSERT INTO marketplace.trade_assets_item VALUES ('{received}', '7');
            INSERT INTO marketplace.coupons VALUES ('{coupon}', 'polygon', 137, '{signer}', '0x01', '0x02', '0x03',
              '{contract}', '{contract}', '{{}}', 1, 100000, '0x04', ARRAY['{contract}'], '2026-01-01', '2027-01-01',
              '2026-01-01');
            INSERT INTO marketplace.coupon_state VALUES ('{coupon}', 3, false, false, '2026-01-05');""")
        # The trades mirror already copied the trade, with assets under its own ids; the node read the
        # coupon later than upstream did.
        self.sql(tgt, f"""
            INSERT INTO shop.trades (id, network, chain_id, signature, hashed_signature, checks, signer, type,
              expires_at, effective_since, created_at, mirrored_from, mirrored_at)
            VALUES ('{trade}', 'polygon', 137, '0xabc', '0xdef', '{{}}', '{signer}', 'public_item_order',
              '2027-01-01', '2026-01-01', '2026-01-02', 'upstream', '2026-01-03');
            INSERT INTO shop.trade_assets (trade_id, direction, asset_type, contract_address)
              VALUES ('{trade}', 'sent', 1, '{contract}');""")
        env = {'MARKETPLACE_SERVER_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt)}
        with unittest.mock.patch.dict(os.environ, env):
            code, out = self.migrate('run', 'marketplace-server', '--step', 'trades', '--step', 'coupons',
                                     sources_dir=migrate.SOURCES)
            self.assertEqual(code, 0, out)
            self.sql(tgt, f"INSERT INTO shop.coupon_state VALUES ('{coupon}', 5, false, false, '2026-02-01');")
            code, out = self.migrate('run', 'marketplace-server', '--step', 'coupon_state', sources_dir=migrate.SOURCES)
            self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, 'SELECT signature, signer, mirrored_from IS NULL FROM shop.trades;').strip(),
                         f'0xabc\t{signer}\tt')
        self.assertEqual(self.sql(tgt, 'SELECT a.id, a.extra, e.amount, i.item_id FROM shop.trade_assets a '
                                       'LEFT JOIN shop.trade_assets_erc20 e ON e.asset_id = a.id '
                                       'LEFT JOIN shop.trade_assets_item i ON i.asset_id = a.id ORDER BY a.id;')
                         .strip().splitlines(),
                         [f'{sent}\t\t1000000000000000000\t', f'{received}\t\t\t7'])
        self.assertEqual(self.sql(tgt, 'SELECT uses FROM shop.coupon_state;').strip(), '5')

    # ── marketplace-favorites-server: lists ──

    def favorites_load(self, db, step_name, rows):
        step = self.financial_step('marketplace-favorites-server', step_name)
        inserts = ''.join(f'INSERT INTO stage VALUES ({row});\n' for row in rows)
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.sql(db, migrate.CHECKPOINT_DDL + 'BEGIN;\n' + self.financial_stage(step) + inserts + step.load.read_text()
                 + '\nCOMMIT;\n')
        return err.getvalue()

    def test_when_the_node_edited_a_list_later_it_should_keep_its_fields_and_grants(self):
        db = self.catalyst_db('favorites_list_newer')
        list_id, owner = '44444444-4444-4444-4444-444444444444', '0x' + 'f' * 40
        row = (f"'{list_id}', 'Wish', 'up', '{owner}', false, '2026-01-01', '2026-01-02', '2026-01-02', "
               "'[{\"permission\":\"view\",\"grantee\":\"*\"}]'")
        self.favorites_load(db, 'lists', [row])
        self.sql(db, f"UPDATE shop.lists SET name = 'Renamed', updated_at = '2026-02-01' "
                     f"WHERE id = '{list_id}';\nDELETE FROM shop.list_acl WHERE list_id = '{list_id}';\n")
        self.favorites_load(db, 'lists', [row])
        mine = f"SELECT l.name, count(a.*) FROM shop.lists l LEFT JOIN shop.list_acl a ON a.list_id = l.id " \
               f"WHERE l.id = '{list_id}' GROUP BY l.name;\n"
        self.assertEqual(self.sql(db, mine).strip(), 'Renamed\t0')
        self.favorites_load(db, 'lists', [row.replace("'2026-01-02'", "'2026-03-01'")])
        self.assertEqual(self.sql(db, mine).strip(), 'Wish\t1')

    def test_when_a_list_collides_with_a_node_list_it_should_report_the_skipped_list_and_picks(self):
        db = self.catalyst_db('favorites_list_collide')
        owner = '0x' + 'f' * 40
        self.sql(db, f"INSERT INTO shop.lists (id, name, user_address) "
                     f"VALUES ('55555555-5555-5555-5555-555555555555', 'Wish', '{owner}');\n")
        upstream = '66666666-6666-6666-6666-666666666666'
        out = self.favorites_load(db, 'lists', [f"'{upstream}', 'Wish', NULL, '{owner}', true, '2026-01-01', NULL, "
                                                f"'2026-01-01', '[]'"])
        self.assertIn('1 list(s) skipped', out)
        out = self.favorites_load(db, 'picks', [f"'{upstream}', 'item', '{owner}', '2026-01-01'"])
        self.assertIn('1 pick(s) skipped', out)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM shop.picks;\n').strip(), '0')
