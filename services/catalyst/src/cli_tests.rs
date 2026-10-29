//! `--help` never prints a DSN or a credential: a walk over the clap tree of
//! every subcommand (`config.rs` keeps the rule for `serve`, where a hidden
//! value is exactly a `_FILE` secret slot; the operator subcommands each
//! declare their own `--database-url` and printed the password until
//! 2026-09-26).

use clap::CommandFactory;

/// The serve command flattens `Config` (hundreds of arguments): building it
/// in a debug test needs more than the test thread's 2 MiB stack.
fn on_a_wide_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

/// A DSN or a credential slot: its value must never reach a pasted
/// `--help` (a ticket, a unit log). `config.rs` keeps the rule for
/// `serve`; the operator subcommands each declare their own
/// `--database-url` and are held to it here.
fn secret_slot(env: &str) -> bool {
    env == "DATABASE_URL"
        || env.ends_with("_DATABASE_URL")
        || env.ends_with("_TOKEN")
        || env.ends_with("_API_KEY")
        || env.ends_with("_PRIVATE_KEY")
        || env.ends_with("_SECRET")
        || env.ends_with("_PASSWORD")
        || env.ends_with("_DSN")
        || env.ends_with("_RPC_URLS")
        || env.starts_with("RPC_ENDPOINT_")
}

fn walk(cmd: &clap::Command, path: &str, offenders: &mut Vec<String>) {
    for arg in cmd.get_arguments() {
        let Some(env) = arg.get_env().and_then(|e| e.to_str()) else {
            continue;
        };
        if secret_slot(env) && !arg.is_hide_env_values_set() {
            offenders.push(format!("{path} --{} prints {env}", arg.get_id()));
        }
    }
    for sub in cmd.get_subcommands() {
        walk(sub, &format!("{path} {}", sub.get_name()), offenders);
    }
}

#[test]
fn no_help_of_any_subcommand_prints_a_secret_bearing_env_value() {
    on_a_wide_stack(|| {
        let mut offenders = Vec::new();
        walk(&crate::Cli::command(), "catalyst", &mut offenders);
        assert!(offenders.is_empty(), "{offenders:#?}");
    });
}

#[test]
fn the_operator_database_urls_are_hidden() {
    on_a_wide_stack(|| {
        let cli = crate::Cli::command();
        let mut seen = 0;
        for (sub, leaf) in [
            ("schema", "apply"),
            ("schema", "migrate"),
            ("builder", "catalog"),
            ("storage", "gc"),
        ] {
            let cmd = cli
                .find_subcommand(sub)
                .unwrap()
                .find_subcommand(leaf)
                .unwrap();
            let arg = cmd
                .get_arguments()
                .find(|a| a.get_id() == "database_url")
                .unwrap();
            assert!(
                arg.is_hide_env_values_set(),
                "catalyst {sub} {leaf} --help prints DATABASE_URL"
            );
            seen += 1;
        }
        assert_eq!(seen, 4);
    });
}

#[test]
fn schema_layout_prints_this_builds_stamp_and_steps() {
    on_a_wide_stack(|| {
        let cli =
            <crate::Cli as clap::Parser>::try_parse_from(["catalyst", "schema", "layout"]).unwrap();
        assert!(matches!(
            cli.command,
            crate::Command::Schema {
                command: crate::SchemaCommand::Layout
            }
        ));
        let layout = crate::schema_layout();
        let lines: Vec<&str> = layout.lines().collect();
        assert_eq!(
            lines[..2],
            [
                format!("identity {}", catalyst::db::SCHEMA_IDENTITY),
                format!("version {}", catalyst::db::SCHEMA_VERSION)
            ]
        );
        assert!(lines[2].starts_with("schema ") && lines[2].len() == "schema ".len() + 64);
        let steps = catalyst::schema_migrate::parse_steps(catalyst::db::MIGRATIONS_SQL);
        assert_eq!(lines.len(), 3 + steps.len());
        for (line, step) in lines[3..].iter().zip(&steps) {
            assert_eq!(*line, format!("step {} {}", step.name, step.sha256));
        }
    });
}
