use deno_core::{ascii_str_include, Extension, ExtensionFileSource};

// Every script a runtime loads is compiled into the binary here.
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
        "ext:deno_webstorage/01_webstorage.js" = "vendor/deno_webstorage/01_webstorage.js",
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

    /// Every vendored script is compiled in with the bytes `upstream.lock.json` records, and
    /// the lock lists every file under `vendor/`. This checks the copies against the lock, not
    /// the lock against crates.io: the deno_web and deno_webidl copies are compared with the
    /// locked crates above, while the deno_fetch and deno_webstorage crates are not dependencies,
    /// so their `upstream_sha256` is checked by hand when they are updated.
    #[test]
    fn vendored_copies_match_the_lock() {
        use sha2::{Digest, Sha256};
        use std::collections::BTreeSet;
        let sha256 = |bytes: &[u8]| -> String {
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        };
        let vendor = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/js/vendor");
        let lock: serde_json::Value =
            serde_json::from_slice(&std::fs::read(vendor.join("upstream.lock.json")).unwrap())
                .unwrap();

        let mut locked = BTreeSet::from(["LICENSE.md".to_owned(), "upstream.lock.json".to_owned()]);
        assert_eq!(
            lock["license"]["sha256"].as_str().unwrap(),
            sha256(include_bytes!("vendor/LICENSE.md"))
        );
        let embedded: Vec<_> = [own(), deno_fetch(), deno_web(), deno_webidl()]
            .into_iter()
            .flatten()
            .filter(|file| file.specifier.starts_with("ext:deno_"))
            .collect();
        for file in lock["packages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|package| package["files"].as_array().unwrap())
        {
            let path = file["path"].as_str().unwrap();
            let specifier = format!("ext:{path}");
            let copy = embedded
                .iter()
                .find(|copy| copy.specifier == specifier)
                .unwrap_or_else(|| panic!("{path} is locked but not compiled in"));
            assert_eq!(
                sha256(copy.load().unwrap().as_str().as_bytes()),
                file["sha256"].as_str().unwrap(),
                "{path} differs from upstream.lock.json"
            );
            locked.insert(path.to_owned());
        }
        assert_eq!(
            locked.len() - 2,
            embedded.len(),
            "a compiled-in copy is not locked"
        );

        let mut on_disk = BTreeSet::new();
        let mut dirs = vec![vendor.clone()];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else {
                    let relative = path.strip_prefix(&vendor).unwrap();
                    on_disk.insert(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        assert_eq!(
            on_disk, locked,
            "files under vendor/ and upstream.lock.json differ"
        );
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
