//! The desktop [`SceneFiles`]: a project directory, walked the way the
//! composite walker always was (dot-entries, `node_modules`, `bin` and `dist`
//! pruned; oversize composites refused with a warning).

use dcl_scene_authoring::{COMPOSITE_FILE_MAX_BYTES, SceneFiles, is_pruned_dir};
use std::path::Path;

pub struct SceneDir<'a> {
    root: &'a Path,
}

impl<'a> SceneDir<'a> {
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }
}

impl SceneFiles for SceneDir<'_> {
    fn list(&self) -> Vec<String> {
        let mut out = Vec::new();
        walk(self.root, self.root, &mut out);
        out
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.root.join(rel)).ok()
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !is_pruned_dir(&name) {
                walk(root, &path, out);
            }
            continue;
        }
        if name.ends_with(".composite")
            && !name.starts_with('.')
            && path.metadata().map(|m| m.len()).unwrap_or(0) > COMPOSITE_FILE_MAX_BYTES
        {
            tracing::warn!(
                "composite '{}' exceeds the {COMPOSITE_FILE_MAX_BYTES}-byte cap; refusing to parse",
                path.display()
            );
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(rel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt_gen::TestDir;

    #[test]
    fn walks_like_the_composite_walker_did() {
        let tmp = TestDir::new("scene-files-walk");
        tmp.write("main.composite", "{}");
        tmp.write("assets/scene/main.composite", "{}");
        tmp.write("assets/scene/.hidden.composite", "{}");
        tmp.write("node_modules/pkg/x.composite", "{}");
        tmp.write("bin/x.composite", "{}");
        tmp.write("dist/x.composite", "{}");
        tmp.write(".dcl-one/x.composite", "{}");
        tmp.write("assets/other.json", "{}");
        let dir = SceneDir::new(&tmp.0);
        let mut listed = dir.list();
        listed.sort();
        assert_eq!(
            listed,
            vec![
                "assets/other.json",
                "assets/scene/.hidden.composite",
                "assets/scene/main.composite",
                "main.composite"
            ]
        );
        assert_eq!(
            dcl_scene_authoring::find_composites(&dir),
            vec!["assets/scene/main.composite", "main.composite"]
        );
        assert_eq!(dir.read("assets/other.json").as_deref(), Some(&b"{}"[..]));
        assert_eq!(dir.read("missing.json"), None);
    }
}
