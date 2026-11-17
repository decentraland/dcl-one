# ADR: The component generation record binds the install process's import closure

- Status: Accepted
- Date: 2026-11-11

## Context

`library/ui/components/generation.json` records what a component install read: the capture, the configuration, the dependency manifests and a list of generator modules, each with its digest. The lock's public component table pins that record's bytes, and the offline guard (`lock-digests.mjs`, a required check) validates the list against the source lock ([shared UI library](2026-10-30-shared-ui-library.md)).

The module list was a conservative superset: a fixed list, every code file at the top of `library/ui/figma` and every non-test file under `library/ui/figma/src`, 111 modules. It was chosen so that no module that could influence generated output could be left out without an analysis of what the installer actually loads. The cost is that the resolver, the other CLI commands, the MCP server, the port commands and the guards were generation inputs although an install never runs them. Any edit to one of them required a new `pnpm figma install components`, a new public component table and, between two branches, a conflict on both files. Most lock conflicts in the recent UI tooling pull requests came from this.

The guarantee the superset gave must stay: any byte that can change generated output is bound by the record, and the check that enforces it needs no installed dependency.

## Decision

The record's module list is the read-set of the install process:

1. the fixed inputs: `component-generation.mjs`, `module-graph.mjs`, `source-identity.mjs`, the retirement ledger, `figma/tsconfig.json`, `src/install-main.ts`, `src/install.ts`, `src/install-components.ts`, the two generators and the four runtime sources, which are copied as bytes;
2. the static import closure of the code among them, computed by `module-graph.mjs`;
3. every non-code file under `library/ui/figma/src` that is not a test or a fixture, because code can read such a file at run time without importing it. There is none today; the rule keeps the old behavior for data.

`componentModulePaths(source, read)` computes the set from the source lock and from bytes it checks against the lock, so the set is a function of the lock alone. The installer, the record validator and the offline guard all call it. The record moves to schema 2; its shape is unchanged.

The alternatives, measured on the tree that carries this decision:

| Roots of the closure                                    | Modules bound | Why not                                                                                                               |
| ------------------------------------------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------- |
| `install.ts`, `install-components.ts`, the generators   | 41            | Unsound: the capture reader and configuration loader reach the installer through its caller, not through its imports. |
| `cli.ts`, the real entry point of an in-process install | 98            | Sound, but binds the resolver, port and guard commands again, because the CLI imports every command.                  |
| `install-main.ts` with `createApp` left in `app.ts`     | 79            | Sound, but binds the sync tooling `app.ts` imports for `runSync`.                                                     |
| `install-main.ts` with `createApp` in `app-context.ts`  | 53            | Chosen.                                                                                                               |

Two small structural changes make the closure equal what an install runs, and neither changes behavior:

- `dcl-figma install` runs in its own process. `cli.ts` validates the command as before and starts `src/install-main.ts` with the tsx loader flags only, forwarding the target, `--source`, `--out` and `--config`; the child parses them again, prints the same result and exits with the same status. In one process the CLI would load every command before the installer runs, so the modules able to affect output would be the CLI's closure, not the installer's. A process boundary leaves the CLI one channel, the arguments, and the bound entry validates them. The bound entry also refuses a process it was not started for: any `execArgv` other than the tsx loader flags (so the CLI cannot add a preload, a condition or another loader), a non-empty `NODE_OPTIONS`, and a `TSX_TSCONFIG_PATH` other than `library/ui/figma/tsconfig.json`, which the CLI sets so tsx resolves through the bound configuration whatever the working directory. The CLI passes `SIGINT` and `SIGTERM` on to the install process.
- `createApp` and its types move from `app.ts` to `app-context.ts`, and `app.ts` re-exports them. `createApp` builds the configuration and the capture reader the installer uses, so it must be bound; `runSync` and `runDiff`, which share its file today, must not be.

### Fail closed

`module-graph.mjs` reads JavaScript and TypeScript with its own tokenizer (strings, templates and their substitutions, comments, regular expressions), so text that looks like an import inside a generated template is not an edge and an import inside a template substitution is. It follows static imports, re-exports and dynamic imports of one string literal. `import type` and `export type` statements are skipped because they are erased before the module runs; an import whose bindings are only used as types still counts. Everything it cannot resolve to exactly one tracked file or a sanctioned package is an error, never a skipped edge:

- a dynamic import whose argument is not one plain string literal; `require` and `getBuiltinModule` in any form (a call, a member, `require.call`, `import x = require(...)`); `createRequire` used for anything but an immediate `createRequire(...).resolve(...)`, which only resolves a path; `node:module` imported other than as `{ createRequire }`; an identifier written with a Unicode escape;
- a relative specifier without a `.js`, `.mjs` or `.json` extension, with a query, fragment, percent escape (Node decodes `%61` to `a`) or backslash escape, that resolves outside the top of `library/ui/figma` and its `src` tree, to a test, fixture or Markdown file, to no tracked file, or to more than one candidate (`x.js` beside `x.ts`);
- a bare specifier that is not a `node:` built-in or one of the six packages the installer already sanctions (`componentPackages`), which covers `#` subpath imports, the package's own name, a new dependency and an unprefixed built-in;
- a module that is JSX, a module it cannot tokenize to the end, and unbalanced brackets;
- a `/` the rules cannot classify. A `/` is a regular expression only at the start of a module, after a template substitution opening, after a punctuator that ends no expression, after the condition of `if`, `while`, `for`, `for await` or `with`, after a prefix `!`, or after a reserved word on an explicit list (`return`, `typeof`, `default`, `await`...). It is a division after a property name, a literal, a value word (`this`, `null`...), another name, `)` or `]`. Every other case fails: after a closing brace, `++`/`--`, a non-null `!`, `>`, `>>` or `>>>`, a contextual keyword, or any reserved word not on either list (`continue`, `break`, `debugger`...). Code outside literals and comments must be ASCII, and a `//` comment or a hashbang ends at any line terminator.

These rules aim to make a misread `/`, the one place a hand-written tokenizer can hide an import inside a literal, an error in the dependency-free guard itself. They are checked against the TypeScript parser on the real tree; an input that defeats them would also need to survive that comparison in CI.

- a `figma/tsconfig.json` that is not plain JSON or that sets `paths`, `baseUrl`, `rootDirs` or `extends`, since tsx would resolve through them.

A generation module can therefore never be a file that port verification exempts, and [that record](2026-11-10-port-verification-binds-port-inputs.md) keeps refusing to exempt any candidate.

The inventories that protect resolution keep their wider class (`isComponentModulePath`: the fixed inputs, top-level code and every non-test file under `src`). The installer and the offline guard still require every such candidate on disk to be tracked and pinned in the source lock, so an untracked, ignored or unpinned file cannot shadow a resolved import. They no longer require every candidate to be in the read-set.

### How a later edit is caught

- An edit to a bound module changes its digest, so the record no longer matches the lock and the offline guard fails until the components are installed again.
- An edit that adds an import to a bound module is such an edit. The next install recomputes the closure and records the newly reached module; until then the guard, which recomputes the same closure, reports `Incomplete component module identities`. No list is maintained by hand.
- An edit to an unbound module changes neither the closure nor any bound digest. It needs resealed lock digests and no install.
- Deleting or renaming a bound module leaves an import that resolves to no tracked file, which fails.

Three tests hold the analysis to the real tree. One compares the specifiers the tokenizer finds in every candidate module with the TypeScript parser's. A third checks with the TypeScript parser that no candidate module names a loader the reader refuses. The other runs `install-main.ts` under a hook that reports each module the process loads and requires that list to equal the code in the closure.

## What stays outside the guarantee

- A bound module that reads a file under `library/ui/figma` at run time without importing it (outside `src`), evaluates text, reaches a loader through computed property access (`process['getBuiltin' + 'Module']`), or starts another program is not followed. The reader refuses every loader it can name (`require`, `getBuiltinModule`, `createRequire` other than `.resolve`, any key or destructure with those names); computed access is left to review. The retirement ledger and the runtime sources are bound by name for this reason; a new run-time read needs a new fixed input. Review owned this boundary before and still does.
- Installed dependency contents are bound through `package.json` and `pnpm-lock.yaml` and checked against the sanctioned installation, as before.
- The process environment (`NODE_OPTIONS`, a different loader) is not an input of the record.
- The port verification record still binds every tracked file under `library/ui/figma` that is not a test, a fixture or Markdown. An edit to unbound tooling still needs a new port verification record; it no longer needs a component install or a new public component table.

## Consequences

- The record binds 53 modules instead of 111: 61 left and 3 new ones (`module-graph.mjs`, `src/install-main.ts`, `src/app-context.ts`) joined. The ones that left are the CLI, the sync, retirement and orphan tooling, the resolver (`src/contract`), the MCP server and its tools, the port commands, the probes, the guards under `src/guard` that only sync uses, and the top-level guards (`key-guard`, `lfs-prefetch`, `lock-digests`, `port-verification`, the `public-*` checkers).
- One-time cost: this change regenerates `components/generation.json` and the public component table, and writes a new port verification record. Every open branch that touches `library/ui/figma` conflicts once on those files and regenerates after merging main.
- A new package import in generation code must be added to `componentPackages`, and a new run-time data read to the fixed inputs, in the same change.
- `dcl-figma install` starts one more process.
