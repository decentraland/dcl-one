//! Renders `schemas/*.sql` (name order) into `$OUT_DIR/schema.sql`, which
//! `db::SCHEMA_SQL` embeds. One file per domain keeps parallel work off one
//! giant DDL file; `catalyst schema print` writes the same rendering (the
//! gitignored root `schema.sql` is that output, never a source).

use std::path::PathBuf;
use std::{env, fs};

const HEADER: &str = "\
-- Catalyst v2: current-state DDL for an empty `catalyst` database.
-- Design/review contract: DATABASE.md. The inherited indexes below are a
-- baseline; query/layout changes require representative workload evidence.
--
-- RENDERED FILE. The source is schemas/*.sql, concatenated in name order by
-- build.rs (one file per domain: 00 content + meta, 10 explore, 20 worlds,
-- 30 shop, 40 social, 50 create: builder + camera_reel). Edit those;
-- `catalyst schema print` writes this rendering.
--
-- Applied once, by an operator, with `catalyst schema apply` (or psql -f). No
-- IF NOT EXISTS: a second apply on a populated database must fail, not no-op.

";

/// The repository root, from the [env] table of the root .cargo/config.toml.
fn root() -> PathBuf {
    PathBuf::from(std::env::var("DCL_ONE_ROOT").expect("DCL_ONE_ROOT"))
}

fn main() {
    println!("cargo:rerun-if-env-changed=DCL_ONE_ROOT");
    let dir = &root().join("schemas");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.expect("schema entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "schema/ holds no .sql file");

    let mut out = String::from(HEADER);
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path.file_name().unwrap().to_string_lossy();
        let body =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        out.push_str(&format!("-- ════ schemas/{name} ════\n\n"));
        out.push_str(body.trim_end_matches('\n'));
        out.push_str("\n\n");
    }
    let out = format!("{}\n", out.trim_end_matches('\n'));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let target = out_dir.join("schema.sql");
    fs::write(&target, out).unwrap_or_else(|e| panic!("writing {}: {e}", target.display()));

    let target = out_dir.join("migrations.sql");
    fs::write(&target, render_migrations())
        .unwrap_or_else(|e| panic!("writing {}: {e}", target.display()));
}

const MIGRATIONS_HEADER: &str = "\
-- Catalyst v2: the additive steps `catalyst schema migrate` applies, in name
-- order, to a database stamped before them (a column an existing table gained,
-- a constraint, a backfill). schema.sql already holds every effect for a new
-- database; `schema apply` records the steps as carried from birth. Each step
-- is recorded in meta.schema_steps with the sha256 of its text and never runs
-- twice; a recorded step whose text changed is refused, and so is a step
-- that drops, renames or retypes anything (src/schema_migrate.rs, the lint).
--
-- RENDERED FILE. The source is schemas/migrations/NNNN_<name>.sql, concatenated
-- in name order by build.rs; `catalyst schema print --migrations` writes this
-- rendering.

";

/// `schemas/migrations/*.sql` in name order, each under a `step` marker the
/// runtime splits on (`schema_migrate::parse_steps`).
fn render_migrations() -> String {
    let dir = &root().join("schemas/migrations");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .map(|entry| entry.expect("migration entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    let mut out = String::from(MIGRATIONS_HEADER);
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path.file_stem().unwrap().to_string_lossy();
        assert!(
            stem.len() > 5
                && stem.as_bytes()[..4].iter().all(u8::is_ascii_digit)
                && stem.as_bytes()[4] == b'_',
            "schemas/migrations/{stem}.sql: a step is named NNNN_<what_it_adds>.sql"
        );
        let body =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        out.push_str(&format!("-- ════ step {stem} ════\n\n"));
        out.push_str(body.trim_end_matches('\n'));
        out.push_str("\n\n");
    }
    format!("{}\n", out.trim_end_matches('\n'))
}
