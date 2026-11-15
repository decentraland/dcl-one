import { readFile } from 'node:fs/promises';
import { normalize } from 'node:path';
import { parseSync } from 'vite';

/**
 * The preload bridge, as two Vite plugins.
 *
 * `preloadExpose` adds one `contextBridge.exposeInMainWorld('__electron_preload__<name>', <value>)`
 * call to the preload entry for each value that the entry exports. `rendererExpose` gives the
 * renderer the `#preload` module, which reads the same keys from `globalThis`. The two plugins get
 * the names from `listExports`, so the two sides always agree.
 *
 * An export form that `listExports` cannot name at build time stops the build.
 */

const KEY_PREFIX = '__electron_preload__';
const PRELOAD_MODULE = '#preload';
const RESOLVED_PRELOAD_MODULE = '\0@preload';
const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;
const SUPPORTED =
  'Use `export const`, `export function`, `export { a }`, `export { a } from` or `export * as a from`.';

/**
 * Lists the value exports of a module, in source order. Type-only exports have no value and are
 * not in the list.
 *
 * @param {string} code
 * @param {string} id
 * @returns {{ name: string, local?: string, from?: string, imported?: string }[]}
 */
export function listExports(code, id) {
  const fail = (node, form) => {
    const line = code.slice(0, node.start).split('\n').length;
    throw new Error(`[preload-expose] ${id}:${line}: ${form} is not supported. ${SUPPORTED}`);
  };
  const { program, errors } = parseSync(id, code);
  if (errors.length > 0) {
    throw new Error(`[preload-expose] ${id}: ${errors[0].message}`);
  }
  const found = [];
  const nameOf = node => (node.type === 'Identifier' ? node.name : String(node.value));
  const add = (node, entry) => {
    if (entry.name === 'default') fail(node, 'A default export');
    for (const name of [entry.name, entry.local, entry.imported]) {
      if (name !== undefined && name !== '*' && !IDENTIFIER.test(name)) {
        fail(node, `The export name ${JSON.stringify(name)}`);
      }
    }
    found.push(entry);
  };
  for (const node of program.body) {
    if (node.type === 'ExportDefaultDeclaration' || node.type === 'TSExportAssignment') {
      fail(node, 'A default export');
    }
    if (node.exportKind === 'type') continue;
    if (node.type === 'ExportAllDeclaration') {
      if (!node.exported) fail(node, `\`export * from '${node.source.value}'\``);
      add(node, { name: nameOf(node.exported), from: node.source.value, imported: '*' });
    }
    if (node.type !== 'ExportNamedDeclaration') continue;
    const { declaration, source, specifiers } = node;
    if (declaration?.declare || declaration?.type === 'TSDeclareFunction') continue;
    if (declaration?.type === 'VariableDeclaration') {
      for (const { id: target } of declaration.declarations) {
        if (target.type !== 'Identifier') fail(node, 'A destructuring export');
        add(node, { name: target.name, local: target.name });
      }
    } else if (declaration?.type === 'FunctionDeclaration') {
      add(node, { name: declaration.id.name, local: declaration.id.name });
    } else if (declaration) {
      fail(node, `An export of a ${declaration.type}`);
    }
    for (const specifier of specifiers) {
      if (specifier.exportKind === 'type') continue;
      const name = nameOf(specifier.exported);
      const local = nameOf(specifier.local);
      add(node, source ? { name, from: source.value, imported: local } : { name, local });
    }
  }
  return found;
}

/**
 * Returns the preload entry with the exposure calls added after the last line. The source lines
 * do not move, so the source map of the entry stays correct.
 *
 * @param {string} code
 * @param {string} id
 */
export function exposeExports(code, id) {
  const imports = ["import { contextBridge as __exposeBridge } from 'electron';"];
  const calls = [];
  listExports(code, id).forEach(({ name, local, from, imported }, index) => {
    let value = local;
    if (from) {
      value = `__exposed${index}`;
      const binding = imported === '*' ? `* as ${value}` : `{ ${imported} as ${value} }`;
      imports.push(`import ${binding} from ${JSON.stringify(from)};`);
    }
    calls.push(`__exposeBridge.exposeInMainWorld('${KEY_PREFIX}${name}', ${value});`);
  });
  return `${code}\n${[...imports, ...calls].join('\n')}\n`;
}

/**
 * Returns the source of the `#preload` module for the renderer.
 *
 * @param {string} code The source of the preload entry.
 * @param {string} id
 */
export function preloadModule(code, id) {
  return listExports(code, id)
    .map(({ name }) => `export const ${name} = globalThis.${KEY_PREFIX}${name};\n`)
    .join('');
}

/**
 * Exposes the exports of the entry module of the preload build.
 *
 * @returns {import('vite').Plugin}
 */
export function preloadExpose() {
  const entries = new Set();
  let exposed = false;
  return {
    name: 'preload-expose',
    enforce: 'pre',
    apply: 'build',
    resolveId(source, _importer, options) {
      if (options.isEntry) entries.add(normalize(source));
      return null;
    },
    transform(code, id) {
      if (!entries.has(normalize(id))) return null;
      exposed = true;
      return { code: exposeExports(code, id), map: null };
    },
    buildEnd(error) {
      if (!error && !exposed) {
        this.error(
          '[preload-expose] No entry module was found, so the build exposes no preload API.',
        );
      }
    },
  };
}

/**
 * Gives the renderer the `#preload` module.
 *
 * @param {{ preloadEntry: string }} options The absolute path of the preload entry.
 * @returns {import('vite').Plugin}
 */
export function rendererExpose({ preloadEntry }) {
  return {
    name: 'renderer-expose',
    resolveId(source) {
      return source === PRELOAD_MODULE ? RESOLVED_PRELOAD_MODULE : null;
    },
    async load(id) {
      if (id !== RESOLVED_PRELOAD_MODULE) return null;
      return preloadModule(await readFile(preloadEntry, 'utf8'), preloadEntry);
    },
  };
}
