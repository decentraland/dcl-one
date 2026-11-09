use deno_core::{ascii_str_include, Extension, ExtensionFileSource};

// Without a startup snapshot, deno_core reads an extension's declared JS from the path it had
// on the build machine, so every script a runtime loads is compiled in here instead.
macro_rules! embedded {
    ($($specifier:literal = $path:literal),* $(,)?) => {
        vec![$(ExtensionFileSource::new(
            $specifier,
            ascii_str_include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/js/", $path)),
        )),*]
    };
}

pub(super) fn own() -> Vec<ExtensionFileSource> {
    embedded!(
        "ext:BevyExplorer/init.js" = "modules/init.js",
        "ext:BevyExplorer/websocket.js" = "modules/websocket.js",
    )
}

/// Upstream deno_fetch 0.284.0 JS (see `vendor/upstream.lock.json`), loaded as `ext:deno_fetch/*`.
pub(super) fn deno_fetch() -> Vec<ExtensionFileSource> {
    embedded!(
        "ext:deno_fetch/20_headers.js" = "vendor/deno_fetch/20_headers.js",
        "ext:deno_fetch/21_formdata.js" = "vendor/deno_fetch/21_formdata.js",
        "ext:deno_fetch/22_body.js" = "vendor/deno_fetch/22_body.js",
        "ext:deno_fetch/22_http_client.js" = "vendor/deno_fetch/22_http_client.js",
        "ext:deno_fetch/23_request.js" = "vendor/deno_fetch/23_request.js",
        "ext:deno_fetch/23_response.js" = "vendor/deno_fetch/23_response.js",
        "ext:deno_fetch/26_fetch.js" = "vendor/deno_fetch/26_fetch.js",
        "ext:deno_fetch/27_eventsource.js" = "vendor/deno_fetch/27_eventsource.js",
    )
}

/// Unmodified copies of the deno_web and deno_webidl scripts the runtime loads.
fn deno_web() -> Vec<ExtensionFileSource> {
    embedded!(
        "ext:deno_web/00_infra.js" = "vendor/deno_web/00_infra.js",
        "ext:deno_web/00_url.js" = "vendor/deno_web/00_url.js",
        "ext:deno_web/01_console.js" = "vendor/deno_web/01_console.js",
        "ext:deno_web/01_dom_exception.js" = "vendor/deno_web/01_dom_exception.js",
        "ext:deno_web/01_mimesniff.js" = "vendor/deno_web/01_mimesniff.js",
        "ext:deno_web/02_event.js" = "vendor/deno_web/02_event.js",
        "ext:deno_web/02_structured_clone.js" = "vendor/deno_web/02_structured_clone.js",
        "ext:deno_web/02_timers.js" = "vendor/deno_web/02_timers.js",
        "ext:deno_web/03_abort_signal.js" = "vendor/deno_web/03_abort_signal.js",
        "ext:deno_web/06_streams.js" = "vendor/deno_web/06_streams.js",
        "ext:deno_web/08_text_encoding.js" = "vendor/deno_web/08_text_encoding.js",
        "ext:deno_web/09_file.js" = "vendor/deno_web/09_file.js",
        "ext:deno_web/12_location.js" = "vendor/deno_web/12_location.js",
        "ext:deno_web/13_message_port.js" = "vendor/deno_web/13_message_port.js",
        "ext:deno_web/15_performance.js" = "vendor/deno_web/15_performance.js",
    )
}

fn deno_webidl() -> Vec<ExtensionFileSource> {
    embedded!("ext:deno_webidl/00_webidl.js" = "vendor/deno_webidl/00_webidl.js")
}

/// Replaces an upstream extension's scripts with the embedded copies. Scripts with no copy
/// are dropped, so loading one is an error rather than a disk read.
pub(super) fn embed_upstream(extension: &mut Extension) {
    let embedded = match extension.name {
        "deno_web" => deno_web(),
        "deno_webidl" => deno_webidl(),
        _ => Vec::new(),
    };
    let pick = |files: &[ExtensionFileSource]| -> Vec<ExtensionFileSource> {
        files
            .iter()
            .filter_map(|file| {
                embedded
                    .iter()
                    .find(|copy| copy.specifier == file.specifier)
                    .cloned()
            })
            .collect()
    };
    extension.js_files = pick(&extension.js_files).into();
    extension.esm_files = pick(&extension.esm_files).into();
    extension.lazy_loaded_js_files = pick(&extension.lazy_loaded_js_files).into();
    extension.lazy_loaded_esm_files = pick(&extension.lazy_loaded_esm_files).into();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream() -> [Extension; 2] {
        [
            deno_webidl::deno_webidl::init(),
            deno_web::deno_web::init(
                deno_web::BlobStore::default_arc(),
                None,
                false,
                deno_web::InMemoryBroadcastChannel::default(),
            ),
        ]
    }

    /// The copies must be the exact scripts of the locked deno_web and deno_webidl: their
    /// ops are the ones those crates register.
    #[test]
    fn embedded_upstream_scripts_match_the_locked_crates() {
        let mut compared = 0;
        for extension in upstream() {
            let mut embedded = extension;
            let original = std::mem::take(&mut embedded.lazy_loaded_js_files);
            embedded.lazy_loaded_js_files = original.clone();
            embed_upstream(&mut embedded);
            for copy in embedded.lazy_loaded_js_files.iter() {
                let file = original
                    .iter()
                    .find(|file| file.specifier == copy.specifier)
                    .unwrap();
                assert_eq!(
                    copy.load().unwrap().as_str(),
                    file.load().unwrap().as_str(),
                    "{} differs from the locked crate",
                    copy.specifier
                );
                compared += 1;
            }
        }
        assert_eq!(compared, deno_web().len() + deno_webidl().len());
    }

    #[test]
    fn every_runtime_script_is_compiled_in() {
        let extensions = super::super::web_extensions();
        let own = own();
        let files = extensions.iter().flat_map(|extension| {
            extension
                .js_files
                .iter()
                .chain(extension.esm_files.iter())
                .chain(extension.lazy_loaded_js_files.iter())
                .chain(extension.lazy_loaded_esm_files.iter())
        });
        for file in files.chain(own.iter()) {
            assert!(
                file.is_runtime_loadable(),
                "{} is read from disk",
                file.specifier
            );
        }
    }
}
