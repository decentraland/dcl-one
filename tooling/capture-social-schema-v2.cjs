#!/usr/bin/env node
// Capture actual prepared-v2 upgrade states from the pinned binary's own tests.
// The generated integration test adds only a pg_dump observer to unchanged
// upgrade calls; it is removed afterwards. No ledger is synthesized.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { execFileSync } = require('node:child_process')
const pin = 'c1fab3991'
const checkout = path.resolve(process.argv[2] || '')
const output = path.resolve(process.argv[3] || '')
// A historical binary must never overwrite a current checkout's artifacts.
const captureTarget = path.join(require('node:os').tmpdir(), 'dcl-social-schema-v2-c1fab3991-target')
if (!process.argv[2] || !process.argv[3]) throw Error('Usage: capture-social-schema-v2.cjs <clean-pinned-checkout> <output-directory>')
const git = (...args) => execFileSync('git', ['-C', checkout, ...args], { encoding: 'utf8' }).trim()
if (git('rev-parse', 'HEAD') !== git('rev-parse', pin) || git('status', '--porcelain', '--untracked-files=no')) throw Error('Expected clean v2 pin')
if (!process.env.CATALYST_TEST_DATABASE_URL) throw Error('CATALYST_TEST_DATABASE_URL required; no silent capture skip')
const source = 'services/catalyst/tests/schema_upgrade_from_closure_base.rs'
const original = fs.readFileSync(path.join(checkout, source), 'utf8')
const generated = path.join(checkout, 'services/catalyst/tests/capture_social_schema_v2.rs')
if (fs.existsSync(generated)) throw Error('Capture test already exists')
fs.mkdirSync(output, { recursive: true })
const observer = String.raw`
fn capture_prepared(pool: &PgPool, label: &str) {
    assert_eq!(catalyst::db::SCHEMA_VERSION, 2);
    let dir = std::path::PathBuf::from(std::env::var("CAPTURE_V2_OUTPUT").unwrap());
    let mut url = url::Url::parse(&std::env::var("CATALYST_TEST_DATABASE_URL").unwrap()).unwrap();
    url.set_path(pool.connect_options().get_database().unwrap());
    let dump = |args: &[&str]| {
        let bin = std::env::var("CAPTURE_PG_DUMP").unwrap_or_else(|_| "pg_dump".into());
        let result = std::process::Command::new(bin).args(["--no-owner", "--no-privileges", "--inserts"])
            .args(args).arg(url.as_str()).output().unwrap();
        assert!(result.status.success(), "pg_dump: {}", String::from_utf8_lossy(&result.stderr));
        String::from_utf8(result.stdout).unwrap().lines().filter(|line| !line.starts_with("\\restrict ") && !line.starts_with("\\unrestrict ") && !line.starts_with("CREATE COLLATION ") && *line != "SET transaction_timeout = 0;")
            .collect::<Vec<_>>().join("\n").trim_end().to_owned() + "\n"
    };
    if label == "upgrade" {
        let mut layout = dump(&["--schema-only"]);
        layout.push_str(&dump(&["--data-only", "--table=meta.schema", "--table=meta.schema_steps"]));
        std::fs::write(dir.join("v2-prepared-c1fab3991.sql"), layout).unwrap();
    }
    let tables: &[&str] = if label == "upgrade" { &["--table=credits.purchases", "--table=worlds.worlds", "--table=explore.event_profile_settings"] } else { &["--table=explore.place", "--table=explore.event", "--table=explore.events_local", "--table=explore.event_moderation", "--table=social.communities", "--table=social.community_members"] };
    let mut args = vec!["--data-only"]; args.extend_from_slice(tables);
    std::fs::write(dir.join(format!("v2-{label}-rows-c1fab3991.sql")), dump(&args)).unwrap();
}
`
let instrumented = original
const afterMigrate = '.unwrap_or_else(|e| panic!("{e}"));'
let occurrence = 0
instrumented = instrumented.replaceAll(afterMigrate, match => {
  occurrence++
  return match + (occurrence <= 2 ? `\n    capture_prepared(pool, "${occurrence === 1 ? 'upgrade' : 'origin'}");` : '')
})
if (occurrence !== 2) throw Error('Pinned test observation points changed')
fs.writeFileSync(generated, instrumented + observer)
try {
  for (const filter of ['a_database_stamped_with_the_closure_base_upgrades_to_head_with_its_rows', 'the_origin_backfill_keeps_local_rows_and_releases_edited_mirrored_events']) {
    execFileSync('nix', ['develop', '--offline', '--command', 'cargo', 'test', '--manifest-path', 'services/catalyst/Cargo.toml', '--test', 'capture_social_schema_v2', filter, '--', '--nocapture'], { cwd: checkout, stdio: 'inherit', env: { ...process.env, CARGO_TARGET_DIR: captureTarget, CAPTURE_V2_OUTPUT: output } })
  }
  const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')
  const inputs = [source, 'services/catalyst/db/src/schema_migrate.rs', 'schemas/releases/v2-shop-schema.sql', 'services/catalyst/tests/fixtures/schema/closure-base-c96370f21.sql']
  const outputs = ['v2-prepared-c1fab3991.sql', 'v2-upgrade-rows-c1fab3991.sql', 'v2-origin-rows-c1fab3991.sql']
  fs.writeFileSync(path.join(output, 'v2-prepared-c1fab3991.json'), JSON.stringify({ repository: 'decentraland/dcl-one', revision: git('rev-parse', 'HEAD'), captured_at: new Date().toISOString(), scope: 'Actual version2 reviewed release and schema_migrate executor in the pinned populated closure-base tests. pg_dump observes immediately after migrate; original assertions continue. Real ledger rows retained, not synthesized; row captures select only their test tables. Cluster-provided collation DDL and pg_dump-only transaction_timeout are excluded; no external application database used. Regeneration varies generated UUIDs/timestamps.', inputs: inputs.map(file => ({ file, sha256: hash(path.join(checkout, file)) })), outputs: outputs.map(file => ({ file, sha256: hash(path.join(output, file)) })), postgres_dump: execFileSync(process.env.CAPTURE_PG_DUMP || 'pg_dump', ['--version'], { encoding: 'utf8' }).trim() }, null, 2) + '\n')
} finally { fs.unlinkSync(generated) }
