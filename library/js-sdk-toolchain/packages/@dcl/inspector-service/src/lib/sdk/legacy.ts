import type { CompositeBlock } from '@dcl/smart-items/legacy';
import {
  renameLegacyComponentName,
  renameLegacyComposite as renameLegacyBlocks,
} from '@dcl/smart-items/legacy';
import { CoreComponents, EditorComponentNames } from './components/types';

export { renameLegacyComponentName };

type Json = Record<string, unknown>;

const CONFIG = EditorComponentNames.Config.replace(/-v\d+$/, '');

function payloads(data: CompositeBlock['data']): Json[] {
  const entries = data instanceof Map ? [...data.values()] : Object.values(data ?? {});
  return entries.flatMap(entry => {
    const { data: inner, json } = (entry ?? {}) as {
      data?: { $case?: string; json?: unknown };
      json?: unknown;
    };
    const payload = inner?.$case === 'json' ? inner.json : json;
    return payload && typeof payload === 'object' ? [payload as Json] : [];
  });
}

function list(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function renameIn(object: unknown, key: string) {
  if (!object || typeof object !== 'object') return;
  const record = object as Json;
  const value = record[key];
  if (typeof value === 'string') record[key] = renameLegacyComponentName(value);
}

// The basic-view config names the components it edits: v1 `fields[].type`, v2
// `sections[].items[].component` and the legacy `componentName`.
function renameConfig(config: Json) {
  renameIn(config, 'componentName');
  for (const field of list(config.fields)) renameIn(field, 'type');
  for (const section of list(config.sections)) {
    for (const item of list((section as Json | null)?.items)) renameIn(item, 'component');
  }
}

// Catalog composites list the synced components by name; the engine turns them into numbers.
function renameSyncComponents(sync: Json) {
  if (Array.isArray(sync.value)) {
    sync.value = sync.value.map(name =>
      typeof name === 'string' ? renameLegacyComponentName(name) : name,
    );
  }
}

/**
 * The runtime's remap of the component blocks and synced component numbers, plus the component
 * names the inspector keeps inside component data. Applied to every composite that arrives from
 * outside: the scene's composite files, the catalog and saved custom assets.
 */
export function renameLegacyComposite<T extends { components?: CompositeBlock[] }>(json: T): T {
  renameLegacyBlocks(json);
  for (const block of json.components ?? []) {
    const rename =
      block.name.replace(/-v\d+$/, '') === CONFIG
        ? renameConfig
        : block.name === CoreComponents.SYNC_COMPONENTS
          ? renameSyncComponents
          : undefined;
    if (rename) payloads(block.data).forEach(rename);
  }
  return json;
}
