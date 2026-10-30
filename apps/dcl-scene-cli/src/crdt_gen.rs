//! `dcl_scene_authoring::crdt_gen` over a project directory: the same
//! path-taking signatures the toolchain always called.

use dcl_scene_authoring::crdt_gen::{GenError, Generated};

use crate::scene_files::SceneDir;
use std::path::Path;

/// Instance every `.composite` under `root` as main.crdt PUT_COMPONENT
/// messages. None when the scene has no composites.
pub fn generate(root: &Path) -> Result<Option<Generated>, GenError> {
    dcl_scene_authoring::crdt_gen::generate(&SceneDir::new(root))
}

/// Describe the first difference between two main.crdt streams, naming
/// components through the composites under `root`.
pub fn describe_difference(native: &[u8], reference: &[u8], root: &Path) -> Option<String> {
    dcl_scene_authoring::crdt_gen::describe_difference(native, reference, &SceneDir::new(root))
}

/// A fresh scratch directory named after `tag` (keep tags unique across the
/// crate: tests share one process), removed on drop.
#[cfg(test)]
pub(crate) struct TestDir(pub std::path::PathBuf);

#[cfg(test)]
impl TestDir {
    pub fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("dcl-scene-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TestDir(dir)
    }

    pub fn write(&self, rel: &str, contents: impl AsRef<[u8]>) {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_from_composites_on_disk() {
        let tmp = TestDir::new("crdtgen-disk");
        assert!(generate(&tmp.0).unwrap().is_none());
        let transform = |y: f64| {
            serde_json::json!({
                "version": 1,
                "components": [{
                    "name": "core::Transform",
                    "data": { "512": { "json": { "position": { "x": 0, "y": y, "z": 0 } } } }
                }]
            })
            .to_string()
        };
        tmp.write("a.composite", transform(1.0));
        tmp.write("b.composite", transform(2.0));
        let generated = generate(&tmp.0).unwrap().unwrap();
        assert_eq!(generated.composites, 2);
        assert_eq!(generated.bytes.len(), 24 + 44);
        let y = f32::from_le_bytes(generated.bytes[24 + 4..24 + 8].try_into().unwrap());
        assert_eq!(y, 2.0);
        assert!(describe_difference(&generated.bytes, &generated.bytes, &tmp.0).is_none());
    }
}
