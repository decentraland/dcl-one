"""Financial cutover guards and rental aggregate freshness regressions."""


class FinancialMigrationCases:
    def financial_step(self, source, name):
        from tooling.tests.test_migration import migrate
        return next(s for s in migrate.load_source(migrate.SOURCES / source).steps if s.name == name)

    def financial_stage(self, step):
        return 'CREATE TEMP TABLE stage (' + ', '.join(f'"{n}" {t}' for n, t in step.columns) + ');\n'

    def test_when_credit_liability_or_payment_facts_are_unresolved_it_should_block_cutover(self):
        from tooling.tests.test_migration import migrate
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
        from tooling.tests.test_migration import migrate, ROOT
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
