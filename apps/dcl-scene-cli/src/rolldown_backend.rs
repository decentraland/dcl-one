use crate::esbuild::EsbuildOptions;
use crate::scene::Project;
use crate::ux::{TrySteps, UserError};
use anyhow::{Result, anyhow};
use rolldown::Bundler;
use rolldown_common::{
    BundlerOptions, BundlerTransformOptions, ChecksOptions, CodeSplittingMode, CommentsOptions,
    Either, InputItem, IsExternal, OutputFormat, Platform, RawMinifyOptions, ResolveOptions,
    SourceMapType, TsConfig,
};
use rolldown_utils::indexmap::FxIndexMap;
use rolldown_utils::pattern_filter::StringOrRegex;
use std::path::PathBuf;

/// Bundle to `opts.outfile`; the files the bundler loaded come back. An import
/// the bundler cannot resolve fails the build and writes nothing, so the last
/// good bundle stays in place.
pub async fn run(project: &Project, opts: &EsbuildOptions) -> Result<Vec<PathBuf>> {
    let mut bundler = Bundler::new(bundler_options(project, opts)?).map_err(|e| {
        anyhow::Error::from(
            UserError::new(
                "the rolldown backend failed to initialize",
                TrySteps::one("re-run with --verbose for the backend report"),
            )
            .why(format!("{e:?}")),
        )
    })?;
    let output = bundler.generate().await.map_err(|e| {
        anyhow::Error::from(
            UserError::new(
                "build failed \u{2014} rolldown reported errors",
                TrySteps::one("fix the errors above"),
            )
            .why(
                e.into_vec()
                    .iter()
                    .map(|diagnostic| diagnostic.to_diagnostic().convert_to_string(false))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        )
    })?;
    let mut unresolved = Vec::new();
    for warning in &output.warnings {
        let kind = warning.kind().to_string();
        match kind.as_str() {
            "INVALID_ANNOTATION" | "IMPORT_IS_UNDEFINED" => {}
            "UNRESOLVED_IMPORT" => unresolved.push(warning.to_string()),
            _ => tracing::warn!("rolldown {kind}: {warning}"),
        }
    }
    if !unresolved.is_empty() {
        return Err(UserError::new(
            "build failed \u{2014} the scene imports modules that cannot be found",
            TrySteps::one("install the missing package: npm install <package>")
                .and("or fix the import path in the file named below"),
        )
        .why(unresolved.join("\n"))
        .into());
    }
    let dir = opts.outfile.parent().unwrap_or(&project.root);
    for asset in &output.assets {
        let dest = dir.join(asset.filename());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::ux::write_error(parent, e))?;
        }
        std::fs::write(&dest, asset.content_as_bytes())
            .map_err(|e| crate::ux::write_error(&dest, e))?;
    }
    Ok(bundler
        .watch_files()
        .iter()
        .map(|file| PathBuf::from(file.as_str()))
        .collect())
}

fn bundler_options(project: &Project, opts: &EsbuildOptions) -> Result<BundlerOptions> {
    Ok(BundlerOptions {
        input: Some(vec![InputItem {
            name: Some("index".into()),
            import: opts.entrypoint.display().to_string(),
        }]),
        cwd: Some(project.root.clone()),
        file: Some(opts.outfile.display().to_string()),
        code_splitting: Some(CodeSplittingMode::Bool(false)),
        format: Some(OutputFormat::Cjs),
        platform: Some(Platform::Browser),
        external: Some(externals(&opts.externals)?),
        define: Some(defines(opts.production)),
        resolve: Some(ResolveOptions {
            alias: Some(aliases(&opts.aliases)),
            ..Default::default()
        }),
        tsconfig: Some(TsConfig::Manual(opts.tsconfig.clone())),
        transform: Some(BundlerTransformOptions {
            target: Some(Either::Left("es2020".to_string())),
            ..Default::default()
        }),
        checks: Some(ChecksOptions {
            invalid_annotation: Some(false),
            import_is_undefined: Some(false),
            ..Default::default()
        }),
        minify: Some(RawMinifyOptions::Bool(opts.production)),
        // A minified bundle is not fed to another bundler or a coverage
        // tool, the only readers of `@__PURE__` and `istanbul ignore`.
        comments: Some(CommentsOptions {
            legal: true,
            annotation: !opts.production,
            jsdoc: !opts.production,
        }),
        sourcemap: if opts.production {
            None
        } else {
            Some(SourceMapType::Inline)
        },
        ..Default::default()
    })
}

fn aliases(list: &[(String, PathBuf)]) -> Vec<(String, Vec<Option<String>>)> {
    list.iter()
        .map(|(name, path)| (name.clone(), vec![Some(path.display().to_string())]))
        .collect()
}

fn externals(extra: &[String]) -> Result<IsExternal> {
    ["~system/*", "@dcl/inspector", "@dcl/inspector/*"]
        .into_iter()
        .chain(extra.iter().map(String::as_str))
        .map(external_pattern)
        .collect::<Result<Vec<_>>>()
        .map(IsExternal::StringOrRegex)
}

fn external_pattern(pattern: &str) -> Result<StringOrRegex> {
    if !pattern.contains('*') {
        return Ok(StringOrRegex::String(pattern.to_string()));
    }
    let mut rx = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => rx.push_str(".*"),
            c if "\\^$.|?+()[]{}".contains(c) => {
                rx.push('\\');
                rx.push(c);
            }
            c => rx.push(c),
        }
    }
    rx.push('$');
    StringOrRegex::new(rx, Some(&String::new()))
        .map_err(|e| anyhow!("external pattern {pattern}: {e}"))
}

fn defines(production: bool) -> FxIndexMap<String, String> {
    let (debug, env) = match production {
        true => ("false", "\"production\""),
        false => ("true", "\"development\""),
    };
    [
        ("document", "undefined"),
        ("window", "undefined"),
        ("DEBUG", debug),
        ("globalThis.DEBUG", debug),
        ("process.env.NODE_ENV", env),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::esbuild::EsbuildOptions;
    use crate::scene::{Project, Tmp};

    fn scene(tag: &str, source: &str) -> (Tmp, Project, EsbuildOptions) {
        let t = Tmp::new(tag);
        t.write("tsconfig.json", "{}");
        t.write("src/index.ts", source);
        t.write("src/helper.ts", "export const helper = 'helped'\n");
        t.write("bin/index.js", "previous bundle\n");
        let project = Project {
            root: t.0.clone(),
            scene_json: serde_json::json!({"main": "bin/index.js"}),
        };
        let opts = EsbuildOptions {
            production: false,
            entrypoint: t.0.join("src/index.ts"),
            outfile: t.0.join("bin/index.js"),
            tsconfig: t.0.join("tsconfig.json"),
            aliases: vec![],
            externals: vec![],
        };
        (t, project, opts)
    }

    #[tokio::test]
    async fn an_import_of_a_missing_package_fails_the_build_and_keeps_the_last_bundle() {
        let (t, project, opts) = scene(
            "unresolved",
            "import { Tween } from 'not-an-installed-package'\nexport function main() { Tween() }\n",
        );
        let err = crate::ux::render(&run(&project, &opts).await.unwrap_err(), false, false);
        assert!(err.contains("not-an-installed-package"), "{err}");
        assert!(err.contains("npm install"), "{err}");
        assert_eq!(
            std::fs::read_to_string(t.0.join("bin/index.js")).unwrap(),
            "previous bundle\n"
        );
    }

    #[tokio::test]
    async fn a_scene_whose_imports_resolve_is_written() {
        let (t, project, opts) = scene(
            "resolved",
            "import { helper } from './helper'\nexport function main() { console.log(helper) }\n",
        );
        run(&project, &opts).await.unwrap();
        let bundle = std::fs::read_to_string(t.0.join("bin/index.js")).unwrap();
        assert!(bundle.contains("helped"), "{bundle}");
        assert!(bundle.contains("sourceMappingURL=data:"), "{bundle}");
    }
}
