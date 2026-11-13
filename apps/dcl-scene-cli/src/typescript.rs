//! The TypeScript a scene checks with: 5.x, 6.x or 7.x, whichever its
//! `node_modules` carries (the vendored archive's is 6.x).
//!
//! 5.x and 6.x are JavaScript compilers; 7.x is the Go port, whose
//! `lib/tsc.js` launches the platform binary from
//! `@typescript/typescript-<os>-<arch>`. All three take `-p`, `--noEmit`,
//! `--incremental --tsBuildInfoFile`, `--declaration --emitDeclarationOnly`
//! and `--pretty`, so one command line serves them. They differ in the
//! tsconfig options they accept: 6.x errors on the ones it deprecates —
//! TS5101 for an option (`baseUrl`), TS5107 for an option's value
//! (`moduleResolution: node`, reported as `node10`; `target: es5`) — unless
//! `ignoreDeprecations` is `"6.0"`, a value 5.x rejects; 7.x has removed
//! them (TS5102 and TS5108, the same split) and takes `paths` targets
//! relative to the tsconfig (TS5090).

use std::path::PathBuf;

use anyhow::Result;

use crate::scene::Project;

pub struct Compiler {
    /// `typescript/lib/tsc.js`; node runs it on every line.
    pub tsc: PathBuf,
    pub version: String,
}

impl Compiler {
    pub fn major(&self) -> u32 {
        major_of(&self.version)
    }
}

/// The scene's own `typescript`, from its `node_modules`.
pub fn locate(project: &Project) -> Result<Compiler> {
    let tsc = project.require_node_module("typescript/lib/tsc.js")?;
    let version = tsc
        .ancestors()
        .nth(2)
        .and_then(|pkg| std::fs::read_to_string(pkg.join("package.json")).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|manifest| Some(manifest.get("version")?.as_str()?.to_string()))
        .unwrap_or_default();
    Ok(Compiler { tsc, version })
}

pub fn major_of(version: &str) -> u32 {
    version
        .split('.')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// What a 6.x run is repeated with when its report carries a deprecation.
pub const IGNORE_DEPRECATIONS_6: [&str; 2] = ["--ignoreDeprecations", "6.0"];

/// 6.x's deprecation codes: an option, and an option's value
/// (`Option 'moduleResolution=node10' is deprecated`).
const DEPRECATED: [&str; 2] = ["TS5101", "TS5107"];
/// 7.x's codes for the same two, removed.
const REMOVED: [&str; 2] = ["TS5102", "TS5108"];

pub fn retry_ignoring_deprecations(compiler: &Compiler, report: &str) -> bool {
    compiler.major() == 6 && {
        let plain = plain(report);
        DEPRECATED
            .iter()
            .any(|code| plain.contains(&format!("error {code}:")))
    }
}

/// Pretty output colours "error" and " TS5101: " separately.
fn plain(report: &str) -> String {
    crate::start::ansi::strip(report)
}

/// The options a report flags with any of `codes`, as tsc writes them:
/// `error TS5101: Option 'baseUrl' is deprecated` gives `baseUrl`,
/// `error TS5107: Option 'target=ES5' is deprecated` gives `target=ES5`.
fn flagged_options(report: &str, codes: &[&str]) -> Vec<String> {
    let plain = plain(report);
    let mut names: Vec<String> = Vec::new();
    for code in codes {
        let marker = format!("error {code}: Option '");
        for name in plain
            .split(&marker)
            .skip(1)
            .filter_map(|rest| rest.split('\'').next())
        {
            if !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Said when a 6.x check needed [`IGNORE_DEPRECATIONS_6`].
pub fn deprecation_message(compiler: &Compiler, report: &str) -> String {
    let names = flagged_options(report, &DEPRECATED);
    let what = match names.is_empty() {
        true => "options".to_string(),
        false => names.join(", "),
    };
    format!(
        "tsconfig uses {what}, which TypeScript {} deprecates and TypeScript 7 removes; checked \
         with --ignoreDeprecations 6.0 this time (migration: https://aka.ms/ts6)",
        compiler.version
    )
}

/// Steps for the options and values a 7.x report says are gone (TS5102,
/// TS5108) and the `paths` targets it wants `./`-relative (TS5090).
pub fn config_steps(report: &str) -> Vec<String> {
    let mut steps: Vec<String> = flagged_options(report, &REMOVED)
        .into_iter()
        .map(|name| match (name.as_str(), name.split_once('=')) {
            ("baseUrl", _) => "remove baseUrl from tsconfig.json: TypeScript 7 removed it; start each \
                               paths target with ./ and add \"*\": [\"./*\"] to paths if bare imports \
                               of project files relied on it"
                .to_string(),
            (_, Some(("moduleResolution", value))) => format!(
                "set moduleResolution to bundler in tsconfig.json, or drop it (the SDK's base config \
                 sets it): TypeScript 7 removed {value}"
            ),
            (_, Some(("target", value))) => format!(
                "set target to es2020 or later in tsconfig.json, or drop it (the SDK's base config \
                 sets it): TypeScript 7 removed {value}"
            ),
            (_, Some((option, value))) => format!(
                "change {option} in tsconfig.json: TypeScript 7 removed the value {value} \
                 (https://aka.ms/ts6)"
            ),
            (other, None) => format!(
                "remove {other} from tsconfig.json: TypeScript 7 removed it (https://aka.ms/ts6)"
            ),
        })
        .collect();
    if plain(report).contains("error TS5090:") {
        steps.push(
            "start every paths target in tsconfig.json with ./ (TypeScript 7 resolves them from \
             the tsconfig's folder)"
                .to_string(),
        );
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiler(version: &str) -> Compiler {
        Compiler {
            tsc: PathBuf::from("node_modules/typescript/lib/tsc.js"),
            version: version.to_string(),
        }
    }

    const TS6: &str = "tsconfig.json(8,5): error TS5101: Option 'baseUrl' is deprecated and will \
                       stop functioning in TypeScript 7.0. Specify compilerOption \
                       '\"ignoreDeprecations\": \"6.0\"' to silence this error.\n  Visit \
                       https://aka.ms/ts6 for migration information.\n";
    const TS7: &str = "tsconfig.json(8,5): error TS5102: Option 'baseUrl' has been removed. Please \
                       remove it from your configuration.\n  Use '\"paths\": {\"*\": [\"./*\"]}' \
                       instead.\ntsconfig.json(11,9): error TS5090: Non-relative paths are not \
                       allowed. Did you forget a leading './'?\n";
    const TYPES: &str = "src/index.ts(3,7): error TS2322: Type 'string' is not assignable to type \
                         'number'.\n";
    /// TypeScript 6.0.3 on `"moduleResolution": "node"` and `"target": "ES5"`:
    /// a value's deprecation is TS5107, and `node` is reported as `node10`.
    const TS6_VALUES: &str = "tsconfig.json(2,50): error TS5107: Option 'target=ES5' is deprecated \
                              and will stop functioning in TypeScript 7.0. Specify compilerOption \
                              '\"ignoreDeprecations\": \"6.0\"' to silence this error.\n\
                              tsconfig.json(2,77): error TS5107: Option 'moduleResolution=node10' \
                              is deprecated and will stop functioning in TypeScript 7.0. Specify \
                              compilerOption '\"ignoreDeprecations\": \"6.0\"' to silence this \
                              error.\n  Visit https://aka.ms/ts6 for migration information.\n";
    /// TypeScript 7.0.2 on the same tsconfig.
    const TS7_VALUES: &str = "tsconfig.json(2,50): error TS5108: Option 'target=ES5' has been \
                              removed. Please remove it from your configuration.\n\
                              tsconfig.json(2,77): error TS5108: Option 'moduleResolution=node10' \
                              has been removed. Please remove it from your configuration.\n";

    #[test]
    fn only_a_6x_run_with_a_deprecation_error_is_repeated() {
        assert!(retry_ignoring_deprecations(&compiler("6.0.3"), TS6));
        assert!(!retry_ignoring_deprecations(&compiler("5.9.3"), TS6));
        assert!(!retry_ignoring_deprecations(&compiler("7.0.2"), TS7));
        assert!(!retry_ignoring_deprecations(&compiler("6.0.3"), TYPES));
    }

    #[test]
    fn a_deprecated_option_value_is_repeated_and_named_too() {
        assert!(retry_ignoring_deprecations(&compiler("6.0.3"), TS6_VALUES));
        assert!(!retry_ignoring_deprecations(&compiler("5.9.3"), TS6_VALUES));
        assert!(!retry_ignoring_deprecations(&compiler("7.0.2"), TS7_VALUES));
        assert_eq!(
            flagged_options(TS6_VALUES, &DEPRECATED),
            ["target=ES5", "moduleResolution=node10"]
        );
        let note = deprecation_message(&compiler("6.0.3"), TS6_VALUES);
        assert!(note.contains("moduleResolution=node10"), "{note}");
        assert!(note.contains("target=ES5"), "{note}");
        let steps = config_steps(TS7_VALUES);
        assert_eq!(steps.len(), 2, "{steps:?}");
        assert!(
            steps[0].starts_with("set target to es2020") && steps[0].contains("removed ES5"),
            "{steps:?}"
        );
        assert!(
            steps[1].starts_with("set moduleResolution to bundler")
                && steps[1].contains("removed node10"),
            "{steps:?}"
        );
        assert!(config_steps(TS6_VALUES).is_empty());
    }

    #[test]
    fn pretty_output_is_read_through_its_colours() {
        let coloured = TS6.replace(
            "error TS5101:",
            "\x1b[91merror\x1b[0m\x1b[90m TS5101:\x1b[0m",
        );
        assert!(retry_ignoring_deprecations(&compiler("6.0.3"), &coloured));
        assert_eq!(flagged_options(&coloured, &DEPRECATED), ["baseUrl"]);
    }

    #[test]
    fn the_deprecation_note_names_the_options_and_the_flag() {
        let note = deprecation_message(&compiler("6.0.3"), TS6);
        assert!(note.contains("baseUrl"), "{note}");
        assert!(note.contains("TypeScript 6.0.3"), "{note}");
        assert!(note.contains("--ignoreDeprecations 6.0"), "{note}");
    }

    #[test]
    fn a_7x_report_turns_into_tsconfig_steps() {
        let steps = config_steps(TS7);
        assert_eq!(steps.len(), 2, "{steps:?}");
        assert!(
            steps[0].starts_with("remove baseUrl from tsconfig.json"),
            "{steps:?}"
        );
        assert!(steps[1].contains("./"), "{steps:?}");
        assert!(config_steps(TS6).is_empty());
        assert!(config_steps(TYPES).is_empty());
    }

    #[test]
    fn major_of_reads_every_line() {
        assert_eq!(major_of("5.9.3"), 5);
        assert_eq!(major_of("6.0.3"), 6);
        assert_eq!(major_of("7.0.2"), 7);
        assert_eq!(major_of("7.1.0-beta"), 7);
        assert_eq!(major_of(""), 0);
    }
}
