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

    def test_rental_asset_import_resets_source_blocks_only_when_upstream_wins(self):
        db = self.fresh_db('rental_asset_membership')
        self.sql(db, (ROOT / 'schemas/32_signatures.sql').read_text())
        step = self.financial_step('signatures-server', 'rental_assets')
        stage = self.financial_stage(step)
        row = """INSERT INTO stage VALUES ('asset',1,'estate-1','ethereum',1,'estate','name',
                 2,true,3,'2026-01-01','2026-01-01','2026-01-02');"""
        blocks = 'SELECT membership_block, metadata_block FROM signatures.rental_assets'
        self.sql(db, stage + row + step.load.read_text())
        self.assertEqual(self.sql(db, blocks).split(), ['0', '0'])
        self.sql(db, "UPDATE signatures.rental_assets SET membership_block=120, metadata_block=115, fetched_at='2026-01-03'")
        self.sql(db, stage + row + step.load.read_text())
        self.assertEqual(self.sql(db, blocks).split(), ['120', '115'])
        self.sql(db, stage + row.replace('2026-01-02', '2026-01-04') + step.load.read_text())
        self.assertEqual(self.sql(db, blocks).split(), ['0', '0'])

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

    # ── credits-server: studios, operators and budgets ──

    STUDIO_A = '11111111-1111-1111-1111-1111111111a1'
    STUDIO_B = '11111111-1111-1111-1111-1111111111b2'

    @staticmethod
    def sql_text(value):
        return 'NULL' if value is None else "'" + str(value).replace("'", "''") + "'"

    def studio_row(self, studio_id, name, status='active', updated='2026-01-02'):
        return f"'{studio_id}', {self.sql_text(name)}, '{status}', 'console', '2026-01-01', '{updated}'"

    def entry_row(self, entry_id, studio_id, cents, kind, credit=None, note=None, key=None):
        q = self.sql_text
        return (f"'{entry_id}', '{studio_id}', {cents}, '{kind}', {q(credit)}, {q(note)}, {q(key)}, "
                f"{q(key and 'hash-' + key)}, 'console', '2026-01-03'")

    def studio_balances(self, db):
        return self.sql(db, 'SELECT studio_id, sum(usd_cents) FROM credits.studio_budget_entries '
                            'GROUP BY studio_id ORDER BY studio_id;\n').strip().splitlines()

    def test_when_studio_budgets_load_each_balance_should_equal_the_sum_of_its_upstream_entries(self):
        db = self.catalyst_db('credits_studio_budget')
        player = '0x' + 'f' * 40
        # The ledger step imported the credit of the first grant (50 cents = 5 Credits), not yet the second.
        self.credits_load(db, 'ledger', [f"'{player}', 0, 'credits-server:credit:c-1', 'grant', 5, '2026-01-03'"])
        self.credits_load(db, 'studio', [self.studio_row(self.STUDIO_A, 'Alpha'), self.studio_row(self.STUDIO_B, 'Beta')])
        e = [f'22222222-2222-2222-2222-22222222222{i}' for i in range(5)]
        entries = [
            self.entry_row(e[0], self.STUDIO_A, 1000, 'deposit', key='k-0'),
            self.entry_row(e[1], self.STUDIO_A, -50, 'grant', credit='c-1', note='gift', key='k-1'),
            self.entry_row(e[2], self.STUDIO_A, -30, 'grant', credit='c-2', key='k-2'),
            self.entry_row(e[3], self.STUDIO_A, -20, 'adjustment', note='fix'),
            self.entry_row(e[4], self.STUDIO_B, 300, 'deposit'),
        ]
        upstream = [f'{self.STUDIO_A}\t900', f'{self.STUDIO_B}\t300']
        self.credits_load(db, 'studio_budget_entry', entries)
        self.assertEqual(self.studio_balances(db), upstream)
        entry = "SELECT kind, coalesce(ledger_id::text, '-'), coalesce(note, '-') FROM credits.studio_budget_entries WHERE id = '{}';\n"
        credit_id = "SELECT id FROM credits.ledger WHERE tx_ref = 'credits-server:credit:{}';\n"
        self.assertEqual(self.sql(db, entry.format(e[1])).strip().split('\t'),
                         ['grant', self.sql(db, credit_id.format('c-1')).strip(), 'gift'])
        self.assertEqual(self.sql(db, entry.format(e[2])).strip().split('\t'),
                         ['adjustment', '-', '(credits-server grant of credit c-2, not imported yet)'])
        # A replay writes nothing.
        self.credits_load(db, 'studio_budget_entry', entries)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM credits.studio_budget_entries;\n').strip(), '5')
        self.assertEqual(self.studio_balances(db), upstream)
        # Once the ledger imports the second credit, the carried grant links to it, balance unchanged.
        self.credits_load(db, 'ledger', [f"'{player}', 0, 'credits-server:credit:c-2', 'grant', 3, '2026-01-04'"])
        self.credits_load(db, 'studio_budget_entry', entries)
        self.assertEqual(self.sql(db, entry.format(e[2])).strip().split('\t'),
                         ['grant', self.sql(db, credit_id.format('c-2')).strip(), '-'])
        self.assertEqual(self.studio_balances(db), upstream)

    def test_when_the_node_changed_a_studio_later_it_should_keep_the_node_values(self):
        db = self.catalyst_db('credits_studio_newer')
        self.credits_load(db, 'studio', [self.studio_row(self.STUDIO_A, 'Alpha')])
        self.sql(db, f"UPDATE credits.studios SET name = 'Alpha Node', status = 'paused', updated_at = '2026-02-01' "
                     f"WHERE id = '{self.STUDIO_A}';\n")
        self.credits_load(db, 'studio', [self.studio_row(self.STUDIO_A, 'Alpha')])
        names = 'SELECT name, status FROM credits.studios;\n'
        self.assertEqual(self.sql(db, names).strip(), 'Alpha Node\tpaused')
        self.credits_load(db, 'studio', [self.studio_row(self.STUDIO_A, 'Alpha 2', updated='2026-03-01')])
        self.assertEqual(self.sql(db, names).strip(), 'Alpha 2\tactive')

    def test_when_the_node_already_names_another_studio_the_same_it_should_stop(self):
        db = self.catalyst_db('credits_studio_name')
        self.sql(db, f"INSERT INTO credits.studios (id, name, created_by) VALUES ('{self.STUDIO_B}', 'Beta', 'node');\n")
        with self.assertRaisesRegex(migrate.MigrationError, 'already has another studio named'):
            self.credits_load(db, 'studio', [self.studio_row(self.STUDIO_A, 'BETA')])
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM credits.studios;\n').strip(), '1')

    def test_studio_operators_follow_upstream_and_leave_the_node_ones_alone(self):
        db = self.catalyst_db('credits_studio_operators')
        self.credits_load(db, 'studio', [self.studio_row(self.STUDIO_A, 'Alpha')])
        op1, op2, node_op = '0x' + '1' * 40, '0x' + '2' * 40, '0x' + '3' * 40
        row = lambda a: f"'{self.STUDIO_A}', '{a}', 'console', '2026-01-02'"
        operators = 'SELECT address FROM credits.studio_operators ORDER BY address;\n'
        self.credits_load(db, 'studio_operator', [row(op1), row(op2)])
        self.sql(db, f"INSERT INTO credits.studio_operators (studio_id, address, added_by) "
                     f"VALUES ('{self.STUDIO_A}', '{node_op}', 'node');\n")
        # Upstream removed op2: it stops acting for the studio; the node's own operator stays.
        self.credits_load(db, 'studio_operator', [row(op1)])
        self.assertEqual(self.sql(db, operators).split(), [op1, node_op])
        # The node removed op1: a later run does not add it back.
        self.sql(db, f"DELETE FROM credits.studio_operators WHERE address = '{op1}';\n")
        self.credits_load(db, 'studio_operator', [row(op1)])
        self.assertEqual(self.sql(db, operators).split(), [node_op])
        # An empty upstream set while imported operators are recorded is refused, unless allowed.
        with self.assertRaisesRegex(migrate.MigrationError, 'refusing to remove'):
            self.credits_load(db, 'studio_operator', [])
        self.credits_load(db, 'studio_operator', [], allow_empty=True)
        self.assertEqual(self.sql(db, operators).split(), [node_op])
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM dcl_migration.credit_studio_operator_loaded;\n').strip(), '0')

    def test_when_credits_grants_import_they_should_keep_the_audit_metadata_once(self):
        src = self.fresh_db('credits_grant_audit_src')
        tgt = self.catalyst_db('credits_grant_audit_tgt')
        wallet, actor = '0x' + 'a' * 40, '0x' + 'b' * 40
        studio = self.STUDIO_A
        self.sql(src, f"""
            CREATE TABLE user_credits (id text PRIMARY KEY, user_address text, denomination text,
                credit_source text, usd_cents bigint, timestamp bigint, available_at bigint);
            CREATE TABLE on_demand_credits (credit_id text PRIMARY KEY REFERENCES user_credits,
                granter_address text NOT NULL, reason text NOT NULL, annotation text);
            CREATE TABLE usd_purchase_intents (id text, status text, user_address text,
                usd_cents bigint, created_at bigint);
            CREATE TABLE stripe_orders (id text, status text, created_at bigint);
            CREATE TABLE stripe_payment_disputes (dispute_id text, outcome text,
                reconciled_at bigint, updated_at bigint);
            CREATE TABLE stripe_payment_refunds (payment_intent_id text,
                reconciled_at bigint, updated_at bigint);

            INSERT INTO user_credits VALUES
              ('direct', '{wallet.upper()}', 'USD', 'grant', 100, 1767225600000, 1767312000000),
              ('studio', '{wallet}', 'USD', 'studio_grant', 50, 1767225600000, NULL),
              ('iap', '{wallet}', 'USD', 'iap', 20, 1767225600000, NULL),
              ('proceeds', '{wallet}', 'USD', 'sale_proceeds', 30, 1767225600000, NULL),
              ('mana', '{wallet}', 'MANA', 'grant', 10, 1767225600000, NULL),
              ('held', '{wallet}', 'USD', 'grant', 10, 1767225600000, 4102444800000);
            INSERT INTO on_demand_credits
              SELECT id, '{actor.upper()}', 'support', CASE WHEN id = 'direct' THEN 'note' END
              FROM user_credits;
        """)
        self.credits_load(tgt, 'studio', [self.studio_row(studio, 'Alpha')])
        env = {'CREDITS_SERVER_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt)}

        def run(*steps, dry_run=False):
            args = [arg for step in steps for arg in ('--step', step)]
            if dry_run:
                args.append('--dry-run')
            err = io.StringIO()
            with unittest.mock.patch.dict(os.environ, env), contextlib.redirect_stderr(err):
                code, out = self.migrate('run', 'credits-server', *args, '--batch-size', '2',
                                         sources_dir=migrate.SOURCES)
            self.assertEqual(code, 0, out + err.getvalue())
            return err.getvalue()

        extract = self.financial_step('credits-server', 'grant_audit').extract.read_text()
        self.assertEqual(self.sql(src, f'SELECT credit_id FROM ({extract}) g ORDER BY credit_id;').split(),
                         ['direct', 'studio'])
        run('ledger')
        self.assertEqual(self.count(tgt, 'ops.admin_audit'), 0)
        self.credits_load(tgt, 'studio_budget_entry', [self.entry_row(
            '22222222-2222-2222-2222-222222222222', studio, -50, 'grant', credit='studio')])
        summary = 'imported 2, skipped already linked 0, skipped no matching ledger row 0'
        self.assertIn(summary, run('grant_audit', dry_run=True))
        self.assertEqual(self.count(tgt, 'ops.admin_audit'), 0)
        self.assertIn(summary, run('grant_audit'))
        audit = """SELECT a.id, l.tx_ref, a.actor, a.target, a.domain, a.action, a.at::date,
                          a.detail->>'reason', COALESCE(a.detail->>'annotation', '-'),
                          a.detail->>'upstreamCreditId', a.detail ? 'studioName',
                          COALESCE(a.detail->>'studioName', '-')
                   FROM credits.ledger l JOIN ops.admin_audit a ON a.id = l.admin_audit_id
                   ORDER BY l.tx_ref;"""
        original = self.sql(tgt, audit).strip().splitlines()
        self.assertEqual([row.split('\t')[1:] for row in original], [
            ['credits-server:credit:direct', actor, wallet, 'credits', 'grant', '2026-01-01',
             'support', 'note', 'direct', 'f', '-'],
            ['credits-server:credit:studio', actor, wallet, 'credits', 'grant', '2026-01-01',
             'support', '-', 'studio', 't', 'Alpha'],
        ])
        self.assertEqual(self.sql(tgt, 'SELECT available FROM credits.wallets;').strip(), '20.0')
        self.assertEqual(self.count(tgt, 'credits.ledger'), 4)
        self.assertEqual(self.count(tgt, 'ops.admin_audit'), 2)

        self.sql(src, f"""
            INSERT INTO user_credits VALUES ('fresh', '{wallet}', 'USD', 'grant', 10, 1767225600000, NULL);
            INSERT INTO on_demand_credits VALUES ('fresh', '{actor}', 'welcome', NULL);
        """)
        logs = run('ledger', 'grant_audit')
        self.assertIn('imported 1, skipped already linked 1, skipped no matching ledger row 0', logs)
        replayed = self.sql(tgt, audit).strip().splitlines()
        self.assertEqual([replayed[0], replayed[2]], original)
        self.assertEqual(replayed[1].split('\t')[1:], [
            'credits-server:credit:fresh', actor, wallet, 'credits', 'grant', '2026-01-01',
            'welcome', '-', 'fresh', 'f', '-',
        ])
        self.assertEqual(self.count(tgt, 'ops.admin_audit'), 3)
        self.assertEqual(self.count(tgt, 'credits.ledger'), 5)
        self.assertEqual(self.sql(tgt, 'SELECT available FROM credits.wallets;').strip(), '21.0')

        self.sql(src, f"""
            INSERT INTO user_credits VALUES
              ('node-linked', '{wallet}', 'USD', 'grant', 10, 1767225600000, NULL),
              ('missing', '{wallet}', 'USD', 'grant', 10, 1767225600000, NULL);
            INSERT INTO on_demand_credits VALUES
              ('node-linked', '{actor}', 'upstream reason', NULL),
              ('missing', '{actor}', 'upstream reason', NULL);
        """)
        self.sql(tgt, f"""
            WITH node_audit AS (
                INSERT INTO ops.admin_audit (actor, domain, action, target, detail)
                VALUES ('0x{'c' * 40}', 'credits', 'grant', '{wallet}', '{{"reason":"node reason"}}')
                RETURNING id
            )
            INSERT INTO credits.ledger (address, kind, bucket, amount, tx_ref, admin_audit_id)
            SELECT '{wallet}', 'grant', 'paid', 1, 'credits-server:credit:node-linked', id FROM node_audit;
        """)
        existing = """SELECT row_to_json(a)
                      FROM credits.ledger l JOIN ops.admin_audit a ON a.id = l.admin_audit_id
                      WHERE l.tx_ref = 'credits-server:credit:node-linked';"""
        node_audit = self.sql(tgt, existing)
        logs = run('grant_audit')
        self.assertIn('imported 0, skipped already linked 2, skipped no matching ledger row 0', logs)
        self.assertIn('imported 0, skipped already linked 1, skipped no matching ledger row 1', logs)
        self.assertIn('WARNING:  credits-server/grant_audit: no matching paid ledger grant for upstream credit "missing"', logs)
        self.assertEqual(self.sql(tgt, existing), node_audit)
        self.assertEqual(self.count(tgt, 'ops.admin_audit'), 4)

    def test_studio_extracts_read_upstream_milliseconds_and_lower_case_operators(self):
        db = self.fresh_db('credits_studio_extract')
        self.sql(db, f'''
            CREATE TABLE studios (id uuid, name text, status text, created_by varchar(42), created_at bigint,
                updated_at bigint);
            CREATE TABLE studio_operators (studio_id uuid, address varchar(42), added_by varchar(42), created_at bigint);
            CREATE TABLE studio_budget_entries (id uuid, studio_id uuid, usd_cents bigint, kind text,
                credit_id varchar(66), note text, idempotency_key text, request_hash text, created_by varchar(42),
                created_at bigint);
            INSERT INTO studios VALUES ('{self.STUDIO_A}', 'Alpha', 'active', 'console', 1767225600000, 1767312000000);
            INSERT INTO studio_operators VALUES ('{self.STUDIO_A}', '0xAB{'c' * 38}', 'console', 1767225600000);
            INSERT INTO studio_budget_entries VALUES ('{'3' * 8}-3333-3333-3333-{'3' * 12}', '{self.STUDIO_A}', -50,
                'grant', 'c-1', NULL, 'k', 'h', 'console', 1767225600000);
        ''')
        folder = migrate.SOURCES / 'credits-server'

        def run(name, columns):
            extract = (folder / f'{name}.extract.sql').read_text()
            return self.sql(db, f'SELECT {columns} FROM ({extract}) x;\n').strip().split('\t')
        self.assertEqual(run('studio', "id, name, status, created_by, created_at AT TIME ZONE 'UTC', "
                                       "updated_at AT TIME ZONE 'UTC'"),
                         [self.STUDIO_A, 'Alpha', 'active', 'console', '2026-01-01 00:00:00', '2026-01-02 00:00:00'])
        self.assertEqual(run('studio_operator', 'address'), ['0xab' + 'c' * 38])
        self.assertEqual(run('studio_budget_entry', 'usd_cents, kind, credit_id'), ['-50', 'grant', 'c-1'])

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

    def rental_override_fixture(self, name):
        db = self.fresh_db(name)
        self.sql(db, (ROOT / 'schemas/32_signatures.sql').read_text())
        self.sql(db, '''INSERT INTO signatures.rental_assets(contract_address,token_id,nft_id,network,chain_id,
                category,search_text,nft_created_at,nft_updated_at,fetched_at)
            VALUES ('asset',1,'asset','ethereum',1,'parcel','','2026-01-01','2026-01-01','2026-01-01');''')
        stage = self.financial_stage(self.financial_step('signatures-server', 'rentals'))
        load = (migrate.SOURCES / 'signatures-server/rentals.load.sql').read_text()
        rental_id = '11111111-1111-1111-1111-111111111111'

        def run(status, updated, rent="NULL,NULL,'2026-01-01','{updated}',NULL,'lessor',NULL"):
            self.sql(db, stage + f"INSERT INTO stage VALUES ('{rental_id}','asset',1,'ethereum',1,'2027-01-01',"
                     f"ARRAY['0','0','0'],'sig','contract','{status}','target'," + rent.format(updated=updated)
                     + ');' + load)

        def state():
            return self.sql(db, """SELECT r.status, coalesce(r.cancel_reason, '-'), r.updated_at::date,
                coalesce(r.rented_days::text, '-'), coalesce(r.started_at::date::text, '-'), coalesce(l.tenant, '-')
                FROM signatures.rentals r JOIN signatures.rental_listings l USING (id);\n""").strip().split('\t')
        return db, rental_id, run, state

    def test_when_upstream_rents_a_listing_the_node_cancelled_later_it_should_apply_the_rent(self):
        rented = "7,NULL,'2026-01-01','{updated}','2026-01-04','lessor','tenant'"
        for status in ('executed', 'claimed'):
            for reason in ('expired', 'transferred', 'dissolved', 'nonce', None):
                with self.subTest(status=status, reason=reason):
                    db, _, run, state = self.rental_override_fixture('financial_rental_rent_wins')
                    run('open', '2026-01-01')
                    self.sql(db, "UPDATE signatures.rentals SET status='cancelled', cancel_reason=%s, "
                             "updated_at='2026-01-05';" % (f"'{reason}'" if reason else 'NULL'))
                    run(status, '2026-01-04', rented)
                    # The reason is cleared and the row is the upstream version, its timestamp included.
                    self.assertEqual(state(), [status, '-', '2026-01-04', '7', '2026-01-04', 'tenant'])

    def test_when_a_rent_replaced_a_node_cancellation_it_should_follow_later_upstream_versions_only(self):
        rented = "7,NULL,'2026-01-01','{updated}','2026-01-04','lessor','tenant'"
        db, rental_id, run, state = self.rental_override_fixture('financial_rental_rent_follow')
        run('open', '2026-01-01')
        self.sql(db, "UPDATE signatures.rentals SET status='cancelled', cancel_reason='expired', updated_at='2026-01-06';")
        run('executed', '2026-01-03', rented)
        # The periods step joins on the parent's upstream timestamp.
        periods = self.financial_step('signatures-server', 'rental_periods')
        self.sql(db, self.financial_stage(periods)
                 + f"INSERT INTO stage VALUES ('22222222-2222-2222-2222-222222222222','{rental_id}',1,7,100,"
                 "'sig','2026-01-03');" + (migrate.SOURCES / 'signatures-server/rental_periods.load.sql').read_text())
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM signatures.rental_periods;\n').strip(), '1')
        # Upstream versions older than the applied one stay skipped.
        for status in ('open', 'cancelled'):
            run(status, '2026-01-02')
            self.assertEqual(state()[:3], ['executed', '-', '2026-01-03'])
        # A later upstream version lands although it predates the node's cancellation.
        run('claimed', '2026-01-04', rented)
        self.assertEqual(state()[:3], ['claimed', '-', '2026-01-04'])

    def test_when_an_older_upstream_row_is_not_a_rent_it_should_keep_the_node_cancellation(self):
        db, _, run, state = self.rental_override_fixture('financial_rental_cancel_kept')
        run('open', '2026-01-01')
        self.sql(db, "UPDATE signatures.rentals SET status='cancelled', cancel_reason='expired', updated_at='2026-01-05';")
        for status in ('open', 'cancelled'):
            with self.subTest(status=status):
                run(status, '2026-01-04')
                self.assertEqual(state()[:3], ['cancelled', 'expired', '2026-01-05'])

    def test_when_the_node_closed_a_rented_listing_it_should_skip_an_older_upstream_row(self):
        for node in ('executed', 'claimed'):
            for upstream in ('open', 'cancelled', 'executed', 'claimed'):
                with self.subTest(node=node, upstream=upstream):
                    db, _, run, state = self.rental_override_fixture('financial_rental_fold_kept')
                    run('open', '2026-01-01')
                    self.sql(db, f"UPDATE signatures.rentals SET status='{node}', updated_at='2026-01-05';"
                             "UPDATE signatures.rental_listings SET tenant='node-tenant';")
                    run(upstream, '2026-01-04', "3,NULL,'2026-01-01','{updated}','2026-01-04','lessor','other'")
                    self.assertEqual(state(), [node, '-', '2026-01-05', '-', '-', 'node-tenant'])

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
            created_at timestamp, trade_digest text);
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
        other, other_sent = '55555555-5555-4555-8555-555555555555', '66666666-6666-4666-8666-666666666666'
        signer, contract = '0x' + 'a' * 40, '0x' + 'b' * 40
        self.sql(src, f"""
            INSERT INTO marketplace.trades VALUES ('{trade}', 'polygon', 137, '0xABC', '0xDEF', '{{}}',
              '{signer.upper().replace('0X', '0x')}', 'public_item_order', '2027-01-01', '2026-01-01', 'c', '2026-01-02',
              '0x{'C' * 64}');
            INSERT INTO marketplace.trades VALUES ('{other}', 'polygon', 137, '0x123', '0x456', '{{}}',
              '{signer}', 'bid', '2027-01-01', '2026-01-01', 'c', '2026-01-02', '0xnot-a-digest');
            INSERT INTO marketplace.trade_assets VALUES
              ('{sent}', '{trade}', 'sent', 1, '{contract}', NULL, '0x', '2026-01-02'),
              ('{received}', '{trade}', 'received', 4, '{contract}', '{signer}', '', '2026-01-02');
            INSERT INTO marketplace.trade_assets VALUES
              ('{other_sent}', '{other}', 'sent', 1, '{contract}', NULL, '', '2026-01-02');
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
        # The promoted copy takes upstream's V3 digest lowercase; a malformed one is not copied.
        self.assertEqual(self.sql(tgt, "SELECT signature, signer, mirrored_from IS NULL, COALESCE(trade_digest, 'none') "
                                       'FROM shop.trades ORDER BY id;').strip().splitlines(),
                         [f'0xabc\t{signer}\tt\t0x{"c" * 64}', f'0x123\t{signer}\tt\tnone'])
        self.assertEqual(self.sql(tgt, 'SELECT a.id, a.extra, e.amount, i.item_id FROM shop.trade_assets a '
                                       'LEFT JOIN shop.trade_assets_erc20 e ON e.asset_id = a.id '
                                       'LEFT JOIN shop.trade_assets_item i ON i.asset_id = a.id '
                                       f"WHERE a.trade_id = '{trade}' ORDER BY a.id;")
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
