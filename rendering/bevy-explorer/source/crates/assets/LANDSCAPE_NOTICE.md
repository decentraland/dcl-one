# Landscape assets

The files under `src/assets/landscape/` come from the Decentraland Unity explorer,
[decentraland/unity-explorer](https://github.com/decentraland/unity-explorer), and are embedded
into the build as `embedded://landscape/...`. They are stored in Git LFS (see the repository's
`.gitattributes`). Every CI job that builds the Explorer pulls only this folder from LFS after its
checkout (`git lfs pull --include="rendering/bevy-explorer/source/crates/assets/src/assets/landscape/**"`),
so the build embeds the files rather than their pointers; a local checkout needs `git lfs pull`.

That repository is licensed under the Apache License 2.0 (its `LICENSE` file).

Unity paths are relative to `Explorer/Assets/` in that repository.

| Files here | Unity repository and commit | Unity source path | Licence | Conversion | Tool and version |
| --- | --- | --- | --- | --- | --- |
