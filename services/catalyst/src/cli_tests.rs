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
            ("camera-reel", "import"),
            ("content", "import-denylist"),
            ("shop", "notice-rewind"),
            ("shop", "compare-sources"),
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
        assert_eq!(seen, 7);
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

/// `content import-denylist --added-by` takes a wallet and lowercases it;
/// anything else is refused before the file or the database is touched.
/// `--print-ids` needs neither the wallet nor the database.
#[test]
fn import_denylist_takes_only_a_wallet() {
    on_a_wide_stack(|| {
        let parse = |args: &[&str]| {
            let mut argv = vec!["catalyst", "content", "import-denylist", "--file", "-"];
            argv.extend_from_slice(args);
            <crate::Cli as clap::Parser>::try_parse_from(argv)
        };
        let db = ["--database-url", "postgres://example.invalid/db"];
        let cli = parse(&[
            db[0],
            db[1],
            "--added-by",
            " 0x00000000000000000000000000000000000000AA ",
        ])
        .unwrap();
        let crate::Command::Content {
            command: crate::ContentCommand::ImportDenylist { added_by, .. },
        } = cli.command
        else {
            panic!("not import-denylist");
        };
        assert_eq!(
            added_by.as_deref(),
            Some("0x00000000000000000000000000000000000000aa")
        );
        for bad in [
            "admin",
            "0x1234",
            "0x00000000000000000000000000000000000000zz",
        ] {
            let err = parse(&[db[0], db[1], "--added-by", bad]).err().unwrap();
            assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation, "{bad}");
            assert!(err.to_string().contains("wallet address"), "{err}");
        }
        assert!(parse(&["--print-ids"]).is_ok());
    });
}

/// `notice-rewind` takes exactly one of `--from-block` (1 or more) and
/// `--since` (after the epoch), and an explicit `--actor`.
#[test]
fn notice_rewind_takes_one_starting_point_in_range() {
    on_a_wide_stack(|| {
        let parse = |extra: &[&str]| {
            let mut args = vec![
                "catalyst",
                "shop",
                "notice-rewind",
                "--database-url",
                "postgres://example.invalid/db",
                "--producer",
                "transfer_received",
            ];
            args.extend_from_slice(extra);
            <crate::Cli as clap::Parser>::try_parse_from(args)
        };
        let kind = |extra: &[&str]| parse(extra).err().map(|e| e.kind());
        assert_eq!(
            kind(&["--from-block", "10", "--since", "2026-11-01"]),
            Some(clap::error::ErrorKind::ArgumentConflict)
        );
        assert_eq!(
            kind(&[]),
            Some(clap::error::ErrorKind::MissingRequiredArgument)
        );
        for bad in ["--from-block=0", "--from-block=-3"] {
            assert_eq!(
                kind(&[bad]),
                Some(clap::error::ErrorKind::ValueValidation),
                "{bad}"
            );
        }
        for bad in ["1970-01-01", "1969-12-31T00:00:00Z", "2026-11-01T00:00:00"] {
            assert_eq!(
                kind(&[&format!("--since={bad}")]),
                Some(clap::error::ErrorKind::ValueValidation),
                "{bad}"
            );
        }
        let cli = parse(&["--from-block", "10", "--actor", "ops-oncall"]).unwrap();
        let crate::Command::Shop {
            command:
                crate::ShopCommand::NoticeRewind {
                    from_block, actor, ..
                },
        } = cli.command
        else {
            panic!("not notice-rewind");
        };
        assert_eq!(from_block, Some(10));
        assert_eq!(actor.as_deref(), Some("ops-oncall"));
    });
}
