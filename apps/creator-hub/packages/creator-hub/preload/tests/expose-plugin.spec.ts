import { describe, it, expect } from 'vitest';

import {
  exposeExports,
  listExports,
  preloadExpose,
  preloadModule,
  rendererExpose,
} from '../expose-plugin.js';

const ENTRY = '/app/preload/src/index.ts';

const SOURCE = `import * as fsService from './services/fs';
import type { Shape } from './types';
const hidden = 1;
function helper(): number {
  return hidden;
}
export const workspace = helper();
export const first = 1,
  second: number = 2;
export function run(): void {}
export async function load(): Promise<void> {}
export { helper, hidden as shown };
export { read, write as save, default as main } from './services/io';
export * as ai from './modules/ai';
export type Local = string;
export interface Options {}
export type { Shape };
export { type Shape as Form };
export type * as types from './types';
export declare const ambient: string;
`;

const NAMES = [
  'workspace',
  'first',
  'second',
  'run',
  'load',
  'helper',
  'shown',
  'read',
  'save',
  'main',
  'ai',
];

describe('listExports', () => {
  it('should list each value export in source order', () => {
    expect(listExports(SOURCE, ENTRY).map(({ name }) => name)).toEqual(NAMES);
  });

  it('should return no name for a module that has only type exports', () => {
    expect(listExports('export type A = string;\nexport interface B {}\n', ENTRY)).toEqual([]);
  });

  it.each([
    ["export * from './services/fs';", "`export * from './services/fs'`"],
    ["export * from 'electron';", "`export * from 'electron'`"],
    ['export default 1;', 'A default export'],
    ['const a = 1;\nexport { a as default };', 'A default export'],
    ['export class Service {}', 'An export of a ClassDeclaration'],
    ['export enum Kind { A }', 'An export of a TSEnumDeclaration'],
    ['export const { a, b } = { a: 1, b: 2 };', 'A destructuring export'],
    ["const a = 1;\nexport { a as 'a-b' };", 'The export name "a-b"'],
  ])('should stop on %s', (source, form) => {
    expect(() => listExports(source, ENTRY)).toThrow(
      `${ENTRY}:${source.split('\n').length}: ${form} is not supported.`,
    );
  });

  it('should stop on a source that does not parse', () => {
    expect(() => listExports('export const = ;', ENTRY)).toThrow(`[preload-expose] ${ENTRY}: `);
  });
});

describe('exposeExports', () => {
  it('should keep the source and add one exposure call for each value export', () => {
    expect(exposeExports(SOURCE, ENTRY)).toBe(
      SOURCE +
        `
import { contextBridge as __exposeBridge } from 'electron';
import { read as __exposed7 } from "./services/io";
import { write as __exposed8 } from "./services/io";
import { default as __exposed9 } from "./services/io";
import * as __exposed10 from "./modules/ai";
__exposeBridge.exposeInMainWorld('__electron_preload__workspace', workspace);
__exposeBridge.exposeInMainWorld('__electron_preload__first', first);
__exposeBridge.exposeInMainWorld('__electron_preload__second', second);
__exposeBridge.exposeInMainWorld('__electron_preload__run', run);
__exposeBridge.exposeInMainWorld('__electron_preload__load', load);
__exposeBridge.exposeInMainWorld('__electron_preload__helper', helper);
__exposeBridge.exposeInMainWorld('__electron_preload__shown', hidden);
__exposeBridge.exposeInMainWorld('__electron_preload__read', __exposed7);
__exposeBridge.exposeInMainWorld('__electron_preload__save', __exposed8);
__exposeBridge.exposeInMainWorld('__electron_preload__main', __exposed9);
__exposeBridge.exposeInMainWorld('__electron_preload__ai', __exposed10);
`,
    );
  });

  it('should not expose a binding that the module does not export', () => {
    const code = exposeExports(SOURCE, ENTRY);
    expect(code).not.toContain('__electron_preload__hidden');
    expect(code).not.toContain('__electron_preload__fsService');
    expect(code).not.toContain('__electron_preload__ambient');
    expect(code).not.toContain('__electron_preload__Form');
  });
});

describe('preloadModule', () => {
  it('should read each exposed key from globalThis', () => {
    expect(preloadModule(SOURCE, ENTRY)).toBe(
      NAMES.map(name => `export const ${name} = globalThis.__electron_preload__${name};\n`).join(
        '',
      ),
    );
  });

  it('should name the same keys as the preload transform', () => {
    const keys = (code: string) =>
      [...code.matchAll(/__electron_preload__\w+/g)].map(([key]) => key);
    expect(keys(preloadModule(SOURCE, ENTRY))).toEqual(keys(exposeExports(SOURCE, ENTRY)));
  });
});

describe('preloadExpose', () => {
  it('should change only the entry module', () => {
    const plugin = preloadExpose() as any;
    expect(plugin.resolveId(ENTRY, undefined, { isEntry: true })).toBeNull();
    expect(plugin.resolveId('./other', ENTRY, { isEntry: false })).toBeNull();
    expect(plugin.transform('export const a = 1;', '/app/preload/src/other.ts')).toBeNull();
    expect(plugin.transform('export const a = 1;', ENTRY)).toEqual({
      code: exposeExports('export const a = 1;', ENTRY),
      map: null,
    });
  });

  it('should stop a build that has no entry module', () => {
    const plugin = preloadExpose() as any;
    const error = (message: string) => {
      throw new Error(message);
    };
    expect(() => plugin.buildEnd.call({ error })).toThrow('No entry module was found');
  });
});

describe('rendererExpose', () => {
  it('should resolve only the #preload module', () => {
    const plugin = rendererExpose({ preloadEntry: ENTRY }) as any;
    expect(plugin.resolveId('#preload')).toBe('\0@preload');
    expect(plugin.resolveId('#store')).toBeNull();
  });
});
