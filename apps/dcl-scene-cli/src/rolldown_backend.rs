use crate::esbuild::EsbuildOptions;
use crate::scene::Project;
use crate::ux::{TrySteps, UserError};
use anyhow::{Result, anyhow};
use rolldown::Bundler;
use rolldown_common::{
    BundlerOptions, BundlerTransformOptions, ChecksOptions, CodeSplittingMode, Either, InputItem,
    IsExternal, OutputFormat, Platform, RawMinifyOptions, ResolveOptions, SourceMapType, TsConfig,
};
use rolldown_utils::indexmap::FxIndexMap;
use rolldown_utils::pattern_filter::StringOrRegex;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

pub async fn run(project: &Project, opts: &EsbuildOptions) -> Result<()> {
    let mut bundler = Bundler::new(bundler_options(project, opts)?).map_err(|e| {
        anyhow::Error::from(
            UserError::new(
                "the rolldown backend failed to initialize",
                TrySteps::one("re-run with --verbose for the backend report"),
            )
            .why(format!("{e:?}")),
        )
    })?;
    let output = bundler.write().await.map_err(|e| {
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
    for warning in &output.warnings {
        let kind = warning.kind().to_string();
        if kind == "INVALID_ANNOTATION" || kind == "IMPORT_IS_UNDEFINED" {
            continue;
        }
        // An import rolldown cannot resolve stays a bare `require` in the chunk, and the split
        // loader's require serves only `~system/*` and the registry: the scene throws "not in the
        // sdk runtime registry" the moment that module loads. Say so at build time, not only
        // under --verbose.
        if kind == "UNRESOLVED_IMPORT" {
            crate::ux::note(format!(
                "\u{26a0} {warning} \u{2014} the scene will throw when this module loads; check that package is fully installed in node_modules"
            ));
            continue;
        }
        tracing::warn!("rolldown {kind}: {warning}");
    }
    Ok(())
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
        external: Some(externals(&opts.externals, &opts.bundled)?),
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

/// The fixed head plus the caller's patterns; a `bundled` match wins over both,
/// which is how the smart-item chunk keeps `@dcl/sdk/*` external and still
/// bundles its own `@dcl/sdk/smart-items`.
fn externals(extra: &[String], bundled: &[String]) -> Result<IsExternal> {
    let external = ["~system/*", "@dcl/inspector", "@dcl/inspector/*"]
        .into_iter()
        .chain(extra.iter().map(String::as_str))
        .map(external_pattern)
        .collect::<Result<Vec<_>>>()?;
    if bundled.is_empty() {
        return Ok(IsExternal::StringOrRegex(external));
    }
    let bundled = bundled
        .iter()
        .map(|p| external_pattern(p))
        .collect::<Result<Vec<_>>>()?;
    Ok(IsExternal::Fn(Some(Arc::new(
        move |specifier: &str, _importer: Option<&str>, _is_resolved: bool| {
            let is_external =
                !matches_any(&bundled, specifier) && matches_any(&external, specifier);
            let done: Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'static>> =
                Box::pin(std::future::ready(Ok(is_external)));
            done
        },
    ))))
}

fn matches_any(patterns: &[StringOrRegex], specifier: &str) -> bool {
    patterns.iter().any(|p| match p {
        StringOrRegex::String(s) => s == specifier,
        StringOrRegex::Regex(r) => r.matches(specifier),
    })
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
