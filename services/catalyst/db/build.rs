//! Renders root `schemas/*.sql` and the two explicitly enrolled definitions
//! in `schemas/releases/v3/`, ordered by filename, into `$OUT_DIR/schema.sql`.
//! `db::SCHEMA_SQL` embeds it and `catalyst schema print` writes the same
//! rendering. The gitignored root `schema.sql` is output, never a source.

use std::path::PathBuf;
use std::{env, fs};

const HEADER: &str = "\
-- Catalyst: current-state DDL for an empty `catalyst` database.
-- Design/review contract: DATABASE.md. The inherited indexes below are a
-- baseline; query/layout changes require representative workload evidence.
--
-- RENDERED FILE. Sources are schemas/*.sql and the two explicitly enrolled
-- definitions in schemas/releases/v3/, ordered by filename by build.rs.
-- Edit those source definitions;
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
    // These sole definitions are also used by the explicit v3 release. Keep
    // them outside automatic root discovery and enroll them only with v3.
    files.extend([
        dir.join("releases/v3/41_social_voice_notifications.sql"),
        dir.join("releases/v3/42_social_private_voice_indexes.sql"),
    ]);
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    assert!(!files.is_empty(), "schema/ holds no .sql file");

    let mut out = String::from(HEADER);
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let repository = root();
        let name = path
            .strip_prefix(&repository)
            .expect("schema within repository")
            .display();
        let body =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        out.push_str(&format!("-- ════ {name} ════\n\n"));
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

    let target = out_dir.join("release.sql");
    fs::write(&target, render_release())
        .unwrap_or_else(|e| panic!("writing {}: {e}", target.display()));
}

/// The release into this binary's schema version
/// (`schemas/releases/v3-private-voice-pending.sql`) as statements
/// `schema migrate --release` runs inside its own transaction: each `\ir` is
/// replaced by the file it names, and the psql-only `\set ON_ERROR_STOP on`,
/// `BEGIN;` and `COMMIT;` lines are dropped. Any other psql meta-command
/// fails the build.
fn render_release() -> String {
    let dir = &root().join("schemas/releases");
    let path = dir.join("v3-private-voice-pending.sql");
    println!("cargo:rerun-if-changed={}", path.display());
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut out = String::new();
    let mut dropped = Vec::new();
    for line in source.lines() {
        match line.trim_end() {
            "\\set ON_ERROR_STOP on" | "BEGIN;" | "COMMIT;" => dropped.push(line.trim_end()),
            include if include.starts_with("\\ir ") => {
                let included = dir.join(include["\\ir ".len()..].trim());
                println!("cargo:rerun-if-changed={}", included.display());
                let body = fs::read_to_string(&included)
                    .unwrap_or_else(|e| panic!("reading {}: {e}", included.display()));
                out.push_str(body.trim_end_matches('\n'));
                out.push('\n');
            }
            meta if meta.starts_with('\\') => {
                panic!(
                    "{}: psql meta-command {meta:?} has no SQL equivalent",
                    path.display()
                )
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    dropped.sort();
    assert_eq!(
        dropped,
        ["BEGIN;", "COMMIT;", "\\set ON_ERROR_STOP on"],
        "{}: the release sets ON_ERROR_STOP and opens and commits one transaction",
        path.display()
    );
    out
}

const MIGRATIONS_HEADER: &str = "\
-- Catalyst: the additive steps `catalyst schema migrate` applies, in name
-- order, to a database stamped before them (a column an existing table gained,
-- a constraint, a backfill). schema.sql already holds every effect for a new
-- database; `schema apply` records active steps as carried from birth. Retired
-- steps remain immutable but never execute or gain a new receipt. Each active step
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
