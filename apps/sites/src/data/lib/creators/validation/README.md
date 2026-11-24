# Item validation

One interface for checking a wearable or emote against Decentraland's requirements, shared by the item editor, the add-items modal, the collection page, the publish modal and the Blender live preview.

```ts
import { getValidator, itemValidationContext } from '~/lib/validation'

const { issues } = await getValidator().validate(
  { kind: 'item', item }, // or { kind: 'blob', contents, mainFile, representations? }
  itemValidationContext(item), // { type, category, hides, loop }
  { signal }
)
```

- **Sources**: `blob` (in-memory files: imports, unsaved edits, live preview) or `item` (a saved item; files read from public storage). A blob's `representations` (`{ mainFile, bodyShapes, contents? }`) list its models; without them `mainFile` is checked for both body shapes. A `.gltf` is not parsed and shows a single file-format error.
- **Both models**: every distinct main file is checked once (a unisex item sharing one file runs once). When an item has more than one model, each issue carries the `bodyShapes` of the model it was found in; the same issue in both shows once per shape. Texture-only representations are skipped.
- **Context**: item type plus what the category-dependent checks (triangle budget, material/texture caps, material naming) read, and `loop` for emotes (the loop-seam check only runs on looping ones). Re-validate whenever any of them changes.
- **Result**: a list of `ValidationIssue`. A check that crashed inside the rule book, or one that could not run at all, is a warning: infrastructure never blocks publishing. `code` is the validator's check name (`triangle-count`, `skeleton`, …). Every issue carries the English `message` written by the rule book.
- **Backends**: `localValidator` runs the `model` and `emote` groups (and, through `validateThumbnail`, the `thumbnail` rule) of [`@dcl-regenesislabs/wearable-validator`](https://github.com/decentraland/wearable-validator), the same rule book curation uses. Rule changes go to that repo, not here. `setValidator()` swaps in another implementation (a remote service, a test double) without touching callers.
- **Off the main thread**: `runner.ts` runs the rule book in a module worker. Production serves the bundle from the CDN, so the worker is a same-origin blob module that imports the CDN chunk. Without `Worker` (jsdom), or when it fails to spawn or crashes, runs go inline; a crash rejects the runs in flight and is reported once (`flow: 'validation_worker'`). A run still going after 60s is taken as hung: the worker is replaced and the runs in flight fail (`step: 'timeout'`). `queue.ts` keeps at most 2 downloads + runs going, models and thumbnails alike.
- **Persistence**: `cache.ts` (`idb-keyval`, store `item-validation/results`) holds saved items' results across reloads. The hooks own the keys (the JSON of their query keys) and never persist a run that threw. A stamp of `[CACHE_REVISION, validator version]` is part of every key and clears the store when either changes: bump `CACHE_REVISION` whenever the finding → issue mapping changes. Without IndexedDB everything still works, just unpersisted.
- **React**: `hooks/useModelValidation` and `hooks/useThumbnailValidation` export their query-option builders, so the editor, the collection page (`hooks/useCollectionValidation`) and the publish modal share one cache. `hooks/useRerunValidation` drops an item's entries and checks it again.
