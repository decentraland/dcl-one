//! `dcl_scene_authoring::entity_names` over a project directory, plus
//! the write side: `assets/scene/entity-names.ts` is regenerated on disk here.

use dcl_scene_authoring::entity_names::{OUTPUT_PATH, render};

use crate::scene_files::SceneDir;
use std::collections::BTreeMap;
use std::path::Path;

/// Collect entity names from every composite under `root`.
pub fn collect(root: &Path) -> BTreeMap<String, String> {
    dcl_scene_authoring::entity_names::collect(&SceneDir::new(root))
}

/// Regenerate `assets/scene/entity-names.ts` when the composites imply a
/// different file than the one on disk.
pub fn write_if_changed(root: &Path) -> std::io::Result<Option<usize>> {
    write(root, &collect(root))
}

/// Write the file for a map the caller already collected (`build` collects
/// first so it can number its steps). Returns the number of names written, or
/// None when no composite names anything — an existing file is then left
/// alone, since a scene may carry a hand-written one. Identical bytes are not
/// rewritten: `watch` rebuilds off mtimes, so touching the file every build
/// would loop.
pub fn write(root: &Path, names: &BTreeMap<String, String>) -> std::io::Result<Option<usize>> {
    if names.is_empty() {
        return Ok(None);
    }
    let rendered = render(names);
    let path = root.join(OUTPUT_PATH);
    if let Ok(existing) = std::fs::read_to_string(&path)
        && existing == rendered
    {
        return Ok(Some(names.len()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, rendered)?;
    Ok(Some(names.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt_gen::TestDir;

    fn composite_with(names: &[(u32, &str)]) -> String {
        let data: Vec<String> = names
            .iter()
            .map(|(id, n)| format!(r#""{id}":{{"json":{{"value":"{n}"}}}}"#))
            .collect();
        format!(
            r#"{{"version":1,"components":[{{"name":"core-schema::Name","data":{{{}}}}}]}}"#,
            data.join(",")
        )
    }

    #[test]
    fn no_names_leaves_an_existing_file_alone() {
        let tmp = TestDir::new("names-empty");
        tmp.write(
            "assets/scene/main.composite",
            r#"{"version":1,"components":[]}"#,
        );
        tmp.write("assets/scene/entity-names.ts", "hand written\n");
        assert_eq!(write_if_changed(&tmp.0).unwrap(), None);
        assert_eq!(
            std::fs::read_to_string(tmp.0.join(OUTPUT_PATH)).unwrap(),
            "hand written\n"
        );
    }

    #[test]
    fn rewrites_only_when_the_bytes_change() {
        let tmp = TestDir::new("names-stable");
        tmp.write(
            "assets/scene/main.composite",
            composite_with(&[(512, "Coil")]),
        );

        assert_eq!(write_if_changed(&tmp.0).unwrap(), Some(1));
        let path = tmp.0.join(OUTPUT_PATH);
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();

        assert_eq!(write_if_changed(&tmp.0).unwrap(), Some(1));
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
    }
}
