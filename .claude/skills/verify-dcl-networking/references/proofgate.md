# Reproducible proof gate

Use Python 3 and the proof project's compiler and standard library. The [retrieval helper](../scripts/fetch_proofgate.py) records the reviewed source revision and SHA-256. It retrieves only `proofgate.py`, verifies its hash, and refuses a differing existing file. It accepts an offline copy with `--source` and applies the same hash check. Inspect the gate's toolchain configuration before running it.

```text
python <skill>/scripts/fetch_proofgate.py --dest <absolute-task-tools-directory>
python <task-tools>/proofgate.py --help
python <task-tools>/proofgate.py -C <proof-directory> init --name NetworkDecision --root <repository-root>
```

Use the initialized manifest as the schema. `roots` maps names to paths or `{path, rev}`. An `anchors` entry is `[id, root, relative_path, unique_declaration_prefix, responsibility]`. Claims carry `id`, qualified `theorem`, `anchors`, and plain-language `says`. Keep `scope`, `uncovered`, `trusted`, `generate`, `checks`, `mutants` and a small `budget.lines` explicit. `_CoqProject` lists modules in dependency order.

Generators and witnesses resolve every source input through `PROOF_ROOT_<ROOT>`. Pinned roots (`rev`) point to staged anchored files; unpinned roots point directly to their configured directory. Snapshot reviewed dirty files into task storage and retain their hashes when verifying an uncommitted state. A check whose `cwd` names a root runs in the original configured directory even for a pinned root, so its working directory is not a source pin. For separately executed source mutations, override `PROOF_ROOT_<ROOT>` with the isolated mutated source tree and confirm the witness reads it.

Every source input needs an anchor. `PROOF_BUILD` is fresh build output: a generator writes its declared `<module>.v` there, and an implementation witness compiles only there. Paths in the manifest are portable and relative; credentials and captured user data never enter proof inputs.

For source extraction, compare the complete bounded decision against a reviewed grammar and reject unknown shapes. Bind decisions in the containing function as well as helpers: a correct retry helper cannot prevent its caller from resetting it at the wrong time. State whether the extraction is merely reviewed code or a mechanized refinement.

```text
python <task-tools>/proofgate.py -C <proof-directory> record --because "Reviewed <specific source/model correspondence>"
python <task-tools>/proofgate.py -C <proof-directory> check
python <task-tools>/proofgate.py -C <proof-directory> mutants
python <task-tools>/proofgate.py -C <proof-directory> mutants --auto --module <small-model-module>
python <task-tools>/proofgate.py -C <proof-directory> report --write
```

`record` does not run implementation checks. `check` does. The pinned tool's mutation pass also skips external `checks`; execute source-mutated implementation witnesses separately when claiming behavioral mutation coverage. A source-grammar rejection demonstrates binding sensitivity, not that a Rust assertion detected the defect. Automatic model mutants test claims against modeled operators/constants; inspect survivors and avoid inflating counts with duplicate declared mutants.

Keep the final `verification.json`, witness results, command exit codes, mutation reasons and source/tool pins outside `_build`. Before claiming a pass, inspect these outputs. Validate the gate itself with one source drift and one deliberately false claim in isolated copies; the failure must occur at the intended layer.

For containerized verification, record the immutable image digest in the task's verification manifest. Follow the machine's storage rules before creating directories, pulling or running it. Bind-mount task sources and build storage, disable container networking, and translate host paths consistently in the compiler adapter. Check the compiler version first. Container setup or missing tooling is an environment failure, never a rejected mutant. A local proof toolchain is equally suitable; this skill does not change global configuration.
