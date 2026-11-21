import { describe, expect, it } from 'vitest';

import { DEFAULT_ROOT_NAME, toComponentName, uniqueName } from './root-naming';

describe('when deriving a UI-root component name', () => {
  it('should PascalCase words and strip non-identifier characters', () => {
    expect(toComponentName('main menu')).toBe('MainMenu');
    expect(toComponentName('my-cool_screen 2')).toBe('MyCoolScreen2');
    expect(toComponentName('HUD')).toBe('HUD');
  });

  it('should guarantee a valid leading character', () => {
    expect(toComponentName('123 panel')).toBe('Panel');
    expect(toComponentName('99')).toBe('GUI');
  });

  it('should fall back to GUI for empty/garbage input', () => {
    expect(toComponentName('')).toBe('GUI');
    expect(toComponentName('   ')).toBe('GUI');
    expect(toComponentName('!@#$')).toBe('GUI');
  });

  it('should name new GUIs GUI, GUI1, GUI2 by default', () => {
    expect(DEFAULT_ROOT_NAME).toBe('GUI');
    expect(uniqueName(DEFAULT_ROOT_NAME, [])).toBe('GUI');
    expect(uniqueName(DEFAULT_ROOT_NAME, ['GUI'])).toBe('GUI1');
    expect(uniqueName(DEFAULT_ROOT_NAME, ['GUI', 'GUI1'])).toBe('GUI2');
  });
});

describe('when ensuring a unique name', () => {
  it('should return the base name when unused', () => {
    expect(uniqueName('MainUI', [])).toBe('MainUI');
    expect(uniqueName('MainUI', ['Hud', 'Menu'])).toBe('MainUI');
  });

  it('should append the smallest free numeric suffix on collision', () => {
    expect(uniqueName('MainUI', ['MainUI'])).toBe('MainUI1');
    expect(uniqueName('MainUI', ['MainUI', 'MainUI1', 'MainUI2'])).toBe('MainUI3');
    expect(uniqueName('MainUI', ['MainUI', 'MainUI2'])).toBe('MainUI1');
  });

  it('should number per-node names the same way', () => {
    expect(uniqueName('Container', ['Container'])).toBe('Container1');
    expect(uniqueName('Container', ['Container', 'Container1'])).toBe('Container2');
    expect(uniqueName('Label', ['Container', 'Container1'])).toBe('Label');
  });
});
