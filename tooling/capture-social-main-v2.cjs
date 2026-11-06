#!/usr/bin/env node
// Capture the shipped main v2 layout and real fresh ledger with its own executor.
// The observer creates/drops one owned database; no stamp or receipt is invented.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { execFileSync } = require('node:child_process')
const pin = '0fb63aa3ff0bf7e7cf515dab06e00263807a7fd6'
if (!process.argv[2] || !process.argv[3]) throw Error('Usage: capture-social-main-v2.cjs <clean-pinned-checkout> <output-directory>')
const checkout = path.resolve(process.argv[2])
const output = path.resolve(process.argv[3])
const git = (...args) => execFileSync('git', ['-C', checkout, ...args], { encoding: 'utf8' }).trim()
if (git('rev-parse', 'HEAD') !== pin || git('status', '--porcelain', '--untracked-files=no')) throw Error('Expected clean main v2 pin')
if (!process.env.CATALYST_TEST_DATABASE_URL) throw Error('CATALYST_TEST_DATABASE_URL required')
const generated = path.join(checkout, 'services/catalyst/db/tests/capture_main_v2.rs')
if (fs.existsSync(generated)) throw Error('Capture observer already exists')
fs.mkdirSync(output, { recursive: true })
fs.writeFileSync(generated, String.raw`
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
#[tokio::test]
async fn capture_real_main_v2() {
    assert_eq!(catalyst_db::SCHEMA_VERSION, 2);
    let source = std::env::var("CATALYST_TEST_DATABASE_URL").unwrap();
    let options: PgConnectOptions = source.parse().unwrap();
    let admin = PgPoolOptions::new().max_connections(1).connect_with(options.clone().database("postgres")).await.unwrap();
    let name = format!("catalyst_test_main_v2_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}"))).execute(&admin).await.unwrap();
    let pool = PgPoolOptions::new().max_connections(2).connect_with(options.database(&name)).await.unwrap();
    catalyst_db::apply_schema(&pool).await.unwrap();
    assert!(catalyst_db::schema_migrate::migrate(&pool, catalyst_db::SCHEMA_SQL, catalyst_db::MIGRATIONS_SQL).await.unwrap().is_empty());
    catalyst_db::bootstrap(&pool).await.unwrap();
    assert_eq!(catalyst_db::schema_migrate::status(&pool, catalyst_db::SCHEMA_SQL, catalyst_db::MIGRATIONS_SQL).await.unwrap().exit_code(), 0);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM meta.schema_steps WHERE step='0020_social_voice_notifications'").fetch_one(&pool).await.unwrap(), 0);
    let mut url = url::Url::parse(&source).unwrap(); url.set_path(&name);
    let dump = |args: &[&str]| {
        let result = std::process::Command::new(std::env::var("CAPTURE_PG_DUMP").unwrap_or_else(|_| "pg_dump".into()))
            .args(["--no-owner", "--no-privileges", "--inserts"]).args(args).arg(url.as_str()).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        String::from_utf8(result.stdout).unwrap().lines().filter(|line| !line.starts_with("\\restrict ") && !line.starts_with("\\unrestrict ") && !line.starts_with("CREATE COLLATION ") && *line != "SET transaction_timeout = 0;")
            .collect::<Vec<_>>().join("\n").trim_end().to_owned() + "\n"
    };
    let mut captured = dump(&["--schema-only"]);
    captured.push_str(&dump(&["--data-only", "--table=meta.schema", "--table=meta.schema_steps"]));
    std::fs::write(std::path::Path::new(&std::env::var("CAPTURE_MAIN_V2_OUTPUT").unwrap()).join("v2-main-0fb63aa3f.sql"), captured).unwrap();
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE {name} WITH (FORCE)"))).execute(&admin).await.unwrap();
}
`)
try {
  const target = path.join(require('node:os').tmpdir(), 'dcl-social-main-v2-0fb63aa3f-target')
  execFileSync('nix', ['develop', '--offline', '--command', 'bash', '-c', 'cargo test --manifest-path services/catalyst/Cargo.toml -p catalyst-db --test capture_main_v2 -- --nocapture'], { cwd: checkout, stdio: 'inherit', env: { ...process.env, CARGO_TARGET_DIR: target, CARGO_BUILD_JOBS: '2', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_INCREMENTAL: '0', CAPTURE_MAIN_V2_OUTPUT: output } })
  const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')
  const inputs = ['services/catalyst/db/src/lib.rs', 'services/catalyst/db/src/schema_migrate.rs', 'services/catalyst/db/build.rs', ...fs.readdirSync(path.join(checkout, 'schemas')).filter(f => f.endsWith('.sql')).sort().map(f => 'schemas/' + f), ...fs.readdirSync(path.join(checkout, 'schemas/migrations')).filter(f => f.endsWith('.sql')).sort().map(f => 'schemas/migrations/' + f)]
  const file = 'v2-main-0fb63aa3f.sql'
  fs.writeFileSync(path.join(output, 'v2-main-0fb63aa3f.json'), JSON.stringify({ repository: 'decentraland/dcl-one', revision: pin, captured_at: new Date().toISOString(), scope: 'Real main v2 catalyst-db apply_schema, migrate, status and bootstrap executor. Full pg_dump schema plus untouched real meta.schema and meta.schema_steps rows. No application database reset, synthetic stamp or ledger. Regeneration retains newly generated receipt timestamps; cluster collation DDL and pg_dump-only transaction_timeout are excluded.', inputs: inputs.map(file => ({ file, sha256: hash(path.join(checkout, file)) })), outputs: [{ file, sha256: hash(path.join(output, file)) }], postgres_dump: execFileSync(process.env.CAPTURE_PG_DUMP || 'pg_dump', ['--version'], { encoding: 'utf8' }).trim() }, null, 2) + '\n')
} finally { fs.unlinkSync(generated) }
