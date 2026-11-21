use super::AppState;
use std::path::PathBuf;

fn runtime_dir(st: &AppState) -> Option<PathBuf> {
    let public = st.data_layer.as_ref()?.public_dir.as_ref()?;
    let dir = public.join("sdk-ui-designer");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).ok()?).ok()?;
    (manifest["oxc"] == "0.60.0"
        && manifest["miniRpc"] == "1.0.7"
        && manifest["uiDesigner"] == true
        && dir.join("runtime.js").is_file()
        && dir.join("oxc_parser_wasm_bg.wasm").is_file()
        && (public.join("bundle.js").is_file() || public.join("bundle.js.gz").is_file()))
    .then_some(dir)
}

pub(super) fn available(st: &AppState) -> bool {
    runtime_dir(st).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start::testkit::{Tmp, scene, state};

    #[test]
    fn designer_requires_exact_runtime_and_real_bundle() {
        let temp = Tmp::new("ui-designer");
        let mut st = state(vec![scene(&temp.0.join("scene"), "Scene", &["0,0"], "")]);
        let public = temp.0.join("public");
        let runtime = public.join("sdk-ui-designer");
        std::fs::create_dir_all(&runtime).unwrap();
        let (_, host_rx) = tokio::sync::watch::channel(Default::default());
        st.data_layer = Some(crate::data_layer::DataLayerState {
            host_rx,
            public_dir: Some(public.clone()),
        });
        assert!(!available(&st));
        for path in [
            runtime.join("runtime.js"),
            runtime.join("oxc_parser_wasm_bg.wasm"),
            public.join("bundle.js"),
        ] {
            std::fs::write(path, b"fixture").unwrap();
        }
        std::fs::write(
            runtime.join("manifest.json"),
            r#"{"oxc":"0.61.0","miniRpc":"1.0.7","uiDesigner":true}"#,
        )
        .unwrap();
        assert!(!available(&st));
        std::fs::write(
            runtime.join("manifest.json"),
            r#"{"oxc":"0.60.0","miniRpc":"1.0.7","uiDesigner":true}"#,
        )
        .unwrap();
        assert!(available(&st));
    }
}
