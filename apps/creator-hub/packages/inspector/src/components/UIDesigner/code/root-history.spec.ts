import { parseSync } from 'oxc-parser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { setStorage } from '../../../lib/data-layer/client/storage';
import type { Storage } from '../../../lib/logic/storage/types';
import { defaultMobileHudConfig } from '../MobileHud/mobile-hud-config';
import { MOBILE_HUD_MODULE } from '../MobileHud/mobile-hud-emit';
import { removeMobileHud, writeMobileHudConfig } from '../MobileHud/mobile-hud-store';
import {
  applySourceEdits,
  duplicateRootUnlocked,
  redoCodeUnlocked,
  refreshRoots,
  removeRootUnlocked,
  rootRedoStack,
  rootUndoStack,
  selectRootFile,
  SCENE_ENTRY,
  set,
  state,
  UI_INDEX,
  undoCodeUnlocked,
} from './store-core';

vi.mock('../../../lib/logic/code-parser', async importOriginal => ({
  ...(await importOriginal<object>()),
  getCodeParser: () => ({
    parse: async (filename: string, source: string) => parseSync(filename, source),
  }),
}));

class MemStorage implements Storage {
  files = new Map<string, string>();

  constructor(seed: Record<string, string>) {
    for (const [k, v] of Object.entries(seed)) this.files.set(k, v);
  }

  async readFile(path: string): Promise<Buffer> {
    const v = this.files.get(path);
    if (v === undefined) throw new Error(`ENOENT: ${path}`);
    return new TextEncoder().encode(v) as unknown as Buffer;
  }
  async writeFile(path: string, content: Buffer): Promise<void> {
    this.files.set(path, new TextDecoder().decode(content as unknown as Uint8Array));
  }
  async exists(path: string): Promise<boolean> {
    return this.files.has(path);
  }
  async delete(path: string): Promise<void> {
    this.files.delete(path);
  }
  async rmdir(): Promise<void> {}
  async list(dir: string): Promise<{ name: string; isDirectory: boolean }[]> {
    return [...this.files.keys()]
      .filter(k => k.startsWith(`${dir}/`) && !k.slice(dir.length + 1).includes('/'))
      .map(k => ({ name: k.slice(dir.length + 1), isDirectory: false }));
  }
  async stat(path: string): Promise<{ size: number }> {
    return { size: this.files.get(path)?.length ?? 0 };
  }
}

const gui = (name: string, body = '<UiEntity uiTransform={{ width: 100 }} />', imports = '') =>
  `/** @jsx ReactEcs.createElement */
import ReactEcs, { UiEntity } from '@dcl/sdk/react-ecs'
${imports}
export function ${name}() {
  return (
    ${body}
  )
}
`;

const HUD = 'src/ui/Hud.tsx';
const MENU = 'src/ui/Menu.tsx';
const CARD = 'src/ui/Card.tsx';

let storage: MemStorage;

const open = async (seed: Record<string, string>, selected: string) => {
  storage = new MemStorage(seed);
  setStorage(storage);
  await refreshRoots();
  await selectRootFile(selected);
};

const rootNames = () => state.roots.map(r => r.name);

beforeEach(() => {
  rootUndoStack.length = 0;
  rootRedoStack.length = 0;
  set({ roots: [], filename: null, source: '', parsed: null, error: null });
});

afterEach(() => setStorage(undefined as unknown as Storage));

describe('when a GUI is deleted', () => {
  beforeEach(async () => {
    await open({ [HUD]: gui('Hud'), [MENU]: gui('Menu') }, MENU);
    set({
      roots: state.roots.map(r => (r.name === 'Menu' ? { ...r, screenInset: 'safe-area' } : r)),
    } as Partial<typeof state>);
    await removeRootUnlocked(MENU);
  });

  it('should be undoable', () => {
    expect(state.canUndo).toBe(true);
  });

  describe('and the delete is undone', () => {
    beforeEach(async () => {
      await undoCodeUnlocked();
    });

    it('should recreate the file with its source', () => {
      expect(storage.files.get(MENU)).toBe(gui('Menu'));
    });

    it('should list it again, in its place, and select it', () => {
      expect(rootNames()).toEqual(['Hud', 'Menu']);
      expect(state.filename).toBe(MENU);
    });

    it('should keep its scene inset in the aggregator', () => {
      expect(state.roots.find(r => r.name === 'Menu')?.screenInset).toBe('safe-area');
      expect(storage.files.get(UI_INDEX)).toContain('Menu');
    });

    describe('and then redone', () => {
      it('should delete it again', async () => {
        await redoCodeUnlocked();

        expect(storage.files.has(MENU)).toBe(false);
        expect(rootNames()).toEqual(['Hud']);
      });
    });
  });

  describe('and the active GUI is then edited', () => {
    it('should undo that edit, not the delete, since the delete is no longer the last action', async () => {
      const source = state.source;
      await applySourceEdits([{ start: 0, end: 0, text: '\n' }], { format: false });

      await undoCodeUnlocked();

      expect(state.source).toBe(source);
      expect(storage.files.has(MENU)).toBe(false);
    });
  });
});

describe('when the last GUI is deleted', () => {
  it('should restore it on undo', async () => {
    await open({ [HUD]: gui('Hud') }, HUD);
    await removeRootUnlocked(HUD);
    expect(rootNames()).toEqual([]);

    await undoCodeUnlocked();

    expect(storage.files.get(HUD)).toBe(gui('Hud'));
    expect(rootNames()).toEqual(['Hud']);
    expect(state.filename).toBe(HUD);
  });
});

describe('when the last GUI is deleted with a customised MobileHUD', () => {
  it('should restore the MobileHUD module and its wiring along with the GUI on undo', async () => {
    await open(
      {
        [HUD]: gui('Hud'),
        [SCENE_ENTRY]:
          "import { setupUi } from './ui'\n\nexport function main() {\n  setupUi()\n}\n",
      },
      HUD,
    );
    const config = defaultMobileHudConfig();
    config.hideJoystick = true;
    await writeMobileHudConfig(config);
    const hudModule = storage.files.get(MOBILE_HUD_MODULE);

    await removeRootUnlocked(HUD);
    await removeMobileHud();
    expect(storage.files.has(MOBILE_HUD_MODULE)).toBe(false);
    expect(storage.files.get(SCENE_ENTRY)).not.toContain('setupMobileHud');

    await undoCodeUnlocked();

    expect(storage.files.get(HUD)).toBe(gui('Hud'));
    expect(storage.files.get(MOBILE_HUD_MODULE)).toBe(hudModule);
    expect(storage.files.get(SCENE_ENTRY)).toContain(
      "import { setupMobileHud } from './mobile-hud'",
    );
    expect(storage.files.get(SCENE_ENTRY)).toContain('setupMobileHud()');
  });
});

describe('when a GUI another GUI nests is deleted', () => {
  it('should refuse, leave the file, and record nothing to undo', async () => {
    await open(
      {
        [CARD]: gui('Card'),
        [HUD]: gui('Hud', '<UiEntity><Card /></UiEntity>', "import { Card } from './Card'"),
      },
      CARD,
    );

    await expect(removeRootUnlocked(CARD)).resolves.toEqual(['Hud']);

    expect(storage.files.has(CARD)).toBe(true);
    expect(rootUndoStack).toHaveLength(0);
  });
});

describe('when a GUI is duplicated', () => {
  beforeEach(async () => {
    await open({ [HUD]: gui('Hud') }, HUD);
    await duplicateRootUnlocked(HUD);
  });

  it('should delete the copy on undo', async () => {
    expect(rootNames()).toEqual(['Hud', 'Hud1']);

    await undoCodeUnlocked();

    expect(storage.files.has('src/ui/Hud1.tsx')).toBe(false);
    expect(rootNames()).toEqual(['Hud']);
  });

  it('should recreate the copy on redo', async () => {
    await undoCodeUnlocked();
    await redoCodeUnlocked();

    expect(rootNames()).toEqual(['Hud', 'Hud1']);
    expect(state.filename).toBe('src/ui/Hud1.tsx');
  });
});
