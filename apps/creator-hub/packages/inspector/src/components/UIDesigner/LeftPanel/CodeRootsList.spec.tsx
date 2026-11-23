import React from 'react';
import { readFileSync } from 'fs';
import { join } from 'path';
import { contextMenu } from 'react-contexify';
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';

import { GUIRoot, Mobile } from '../../Icons/figma';
import { CodeRootsList } from './CodeRootsList';

const mocks = vi.hoisted(() => ({
  removeRoot: vi.fn(),
  duplicateRoot: vi.fn(),
  menuOpen: false,
  toggleTopLevel: vi.fn(),
  selectRootFile: vi.fn(),
  renameRoot: vi.fn(),
  roots: [] as { name: string; filename: string; topLevel: boolean }[],
  filename: null as string | null,
}));

vi.mock('../code/store', () => ({
  removeRoot: mocks.removeRoot,
  duplicateRoot: mocks.duplicateRoot,
  renameRoot: mocks.renameRoot,
  selectRootFile: mocks.selectRootFile,
  toggleTopLevel: mocks.toggleTopLevel,
  useCodeState: () => ({
    roots: mocks.roots,
    filename: mocks.filename,
    parsed: null,
    error: null,
  }),
}));

vi.mock('../../../hooks/useHotkey', () => ({ isMenuOpen: () => mocks.menuOpen }));
vi.mock('react-dnd', () => ({ useDrag: () => [{ isDragging: false }, () => {}] }));
vi.mock('../../../redux/hooks', () => ({
  useAppDispatch: () => vi.fn(),
  useAppSelector: () => null,
}));

const root = (name: string, topLevel = true) => ({
  name,
  filename: `src/ui/${name}.tsx`,
  topLevel,
});

const rowNames = () =>
  [...document.querySelectorAll('.ui-designer-code-root-name')].map(n => n.textContent);

const rowFor = (name: string) =>
  [...document.querySelectorAll<HTMLElement>('.ui-designer-code-root-row')].find(
    r => r.querySelector('.ui-designer-code-root-name')?.textContent === name,
  )!;

beforeEach(() => {
  mocks.removeRoot.mockClear();
  mocks.duplicateRoot.mockClear();
  mocks.renameRoot.mockClear();
  mocks.menuOpen = false;
  mocks.toggleTopLevel.mockClear();
  mocks.selectRootFile.mockClear();
  mocks.roots = [root('MainUI'), root('Hud'), root('Card', false)];
  mocks.filename = 'src/ui/MainUI.tsx';
});

describe('when the rail passes no search term', () => {
  it('should list every root', () => {
    render(<CodeRootsList />);

    expect(rowNames()).toEqual(['MainUI', 'Hud', 'Card']);
  });
});

describe('when the rail passes a search term', () => {
  it('should keep only the roots whose name matches, case-insensitively', () => {
    render(<CodeRootsList filter="hu" />);

    expect(rowNames()).toEqual(['Hud']);
  });

  it('should match on any part of the name', () => {
    render(<CodeRootsList filter="ui" />);

    expect(rowNames()).toEqual(['MainUI']);
  });

  it('should list nothing when no root matches', () => {
    render(<CodeRootsList filter="zzz" />);

    expect(rowNames()).toEqual([]);
  });

  it('should ignore surrounding whitespace', () => {
    render(<CodeRootsList filter="  hud  " />);

    expect(rowNames()).toEqual(['Hud']);
  });
});

describe('when a root is a nested-only component', () => {
  it('should gray its row, since it renders nowhere on its own', () => {
    render(<CodeRootsList />);

    expect(rowFor('Card').className).toContain('is-hidden');
    expect(rowFor('MainUI').className).not.toContain('is-hidden');
  });
});

describe('when using a row action', () => {
  it('should offer no delete button on the row, since delete lives in the menu and on the key', () => {
    render(<CodeRootsList />);

    expect(screen.queryByLabelText('Delete Hud')).toBeNull();
  });

  it('should toggle top-level from the eye button', () => {
    render(<CodeRootsList />);
    fireEvent.click(screen.getByLabelText('Hide Card'));

    expect(mocks.toggleTopLevel).toHaveBeenCalledWith('src/ui/Card.tsx');
  });

  it('should not select the root when a row action is used', () => {
    render(<CodeRootsList />);
    fireEvent.click(screen.getByLabelText('Hide Card'));

    expect(mocks.selectRootFile).not.toHaveBeenCalled();
  });

  it('should press the eye when the GUI is hidden, as every other eye', () => {
    render(<CodeRootsList />);

    expect(screen.getByLabelText('Hide Card').getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByLabelText('Hide MainUI').getAttribute('aria-pressed')).toBe('false');
  });
});

describe('when the list is read by assistive technology', () => {
  afterEach(cleanup);

  const list = () => document.querySelector<HTMLElement>('.ui-designer-roots-tree')!;
  const hudRow = () => document.querySelector<HTMLElement>('.ui-designer-mobile-hud-row')!;

  it('should be a single-select tree named GUIs, so the row eyes stay exposed', () => {
    render(<CodeRootsList />);

    expect(list().getAttribute('role')).toBe('tree');
    expect(list().getAttribute('aria-label')).toBe('GUIs');
    expect(list().getAttribute('aria-multiselectable')).toBeNull();
  });

  it('should make MobileHUD and every GUI a level-1 tree item, selected only on the active GUI', () => {
    render(<CodeRootsList />);
    const items = [...list().querySelectorAll('[role="treeitem"]')];

    expect(items).toEqual([hudRow(), rowFor('MainUI'), rowFor('Hud'), rowFor('Card')]);
    expect(items.map(o => o.getAttribute('aria-level'))).toEqual(['1', '1', '1', '1']);
    expect(items.map(o => o.getAttribute('aria-selected'))).toEqual([
      'false',
      'true',
      'false',
      'false',
    ]);
    expect(items.some(o => o.hasAttribute('aria-current'))).toBe(false);
  });

  it("should name every row, MobileHUD included, by its label alone, as the trees' rows, not by its eye", () => {
    render(<CodeRootsList />);
    const labelOf = (row: HTMLElement) =>
      document.getElementById(row.getAttribute('aria-labelledby') ?? '')?.textContent;

    expect([hudRow(), rowFor('MainUI'), rowFor('Hud'), rowFor('Card')].map(labelOf)).toEqual([
      'MobileHUD',
      'MainUI',
      'Hud',
      'Card',
    ]);
  });

  it('should leave every row without aria-expanded, since GUIs do not nest', () => {
    render(<CodeRootsList />);

    expect(
      [hudRow(), rowFor('MainUI'), rowFor('Hud'), rowFor('Card')].some(r =>
        r.hasAttribute('aria-expanded'),
      ),
    ).toBe(false);
  });

  it('should put the one Tab stop on the selected GUI', () => {
    render(<CodeRootsList />);

    expect(
      [hudRow(), rowFor('MainUI'), rowFor('Hud'), rowFor('Card')].map(o => o.tabIndex),
    ).toEqual([-1, 0, -1, -1]);
  });

  it("should keep the other GUIs' eyes out of the Tab order, as the trees' actions", () => {
    render(<CodeRootsList />);

    expect(['MainUI', 'Hud', 'Card'].map(n => screen.getByLabelText(`Hide ${n}`).tabIndex)).toEqual(
      [0, -1, -1],
    );
  });

  it('should put the Tab stop on the first option when nothing is selected', () => {
    mocks.filename = null;
    render(<CodeRootsList />);

    expect(
      [hudRow(), rowFor('MainUI'), rowFor('Hud'), rowFor('Card')].map(o => o.tabIndex),
    ).toEqual([0, -1, -1, -1]);
  });

  it('should put the Tab stop on the first option when the selected GUI is filtered out', () => {
    render(<CodeRootsList filter="hud" />);

    expect([hudRow(), rowFor('Hud')].map(o => o.tabIndex)).toEqual([0, -1]);
  });
});

describe('when moving through the list with the keyboard', () => {
  afterEach(cleanup);

  const hudRow = () => document.querySelector<HTMLElement>('.ui-designer-mobile-hud-row')!;
  const press = (key: string) => fireEvent.keyDown(document.activeElement!, { key });

  it.each([
    ['ArrowDown', 'Hud'],
    ['ArrowUp', 'MobileHUD'],
    ['Home', 'MobileHUD'],
    ['End', 'Card'],
  ])('should move focus from the selected GUI on %s to %s', (key, expected) => {
    render(<CodeRootsList />);
    rowFor('MainUI').focus();
    press(key);

    expect(document.activeElement!.textContent).toBe(expected);
  });

  it('should stay on the last option on ArrowDown and the first on ArrowUp', () => {
    render(<CodeRootsList />);
    rowFor('Card').focus();
    press('ArrowDown');
    expect(document.activeElement).toBe(rowFor('Card'));

    hudRow().focus();
    press('ArrowUp');
    expect(document.activeElement).toBe(hudRow());
  });

  it.each(['ArrowRight', 'ArrowLeft'])('should do nothing on %s', key => {
    render(<CodeRootsList />);
    rowFor('Hud').focus();
    const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
    rowFor('Hud').dispatchEvent(event);

    expect(document.activeElement).toBe(rowFor('Hud'));
    expect(event.defaultPrevented).toBe(false);
    expect(mocks.selectRootFile).not.toHaveBeenCalled();
  });

  it('should keep the navigation keys from reaching the document', () => {
    const onDocumentKey = vi.fn();
    document.addEventListener('keydown', onDocumentKey);
    render(<CodeRootsList />);
    rowFor('MainUI').focus();
    press('ArrowDown');
    press('End');
    document.removeEventListener('keydown', onDocumentKey);

    expect(onDocumentKey).not.toHaveBeenCalled();
  });

  it.each(['Enter', ' '])('should select the focused GUI on %j', key => {
    render(<CodeRootsList />);
    rowFor('Hud').focus();
    press(key);

    expect(mocks.selectRootFile).toHaveBeenCalledWith('src/ui/Hud.tsx');
  });

  it.each(['.ui-designer-code-root-row', '.ui-designer-mobile-hud-row'])(
    'should keep Space on %s from reaching document hotkeys such as the camera reset',
    selector => {
      const onDocumentKey = vi.fn();
      document.addEventListener('keydown', onDocumentKey);
      render(<CodeRootsList />);
      fireEvent.keyDown(document.querySelector(selector)!, { key: ' ' });
      document.removeEventListener('keydown', onDocumentKey);

      expect(onDocumentKey).not.toHaveBeenCalled();
    },
  );
});

describe('when a GUI row is right-clicked', () => {
  const openMenu = (name: string) => {
    render(<CodeRootsList />);
    fireEvent.contextMenu(rowFor(name));
    return screen.getByRole('menu', { hidden: true });
  };
  const item = (menu: HTMLElement, label: string) =>
    [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      i => i.textContent?.trim() === label,
    )!;

  afterEach(() => {
    act(() => contextMenu.hideAll());
    cleanup();
  });

  it('should open the GUI Options menu with Rename, Duplicate and Delete', () => {
    const menu = openMenu('Hud');

    expect(menu.getAttribute('aria-label')).toBe('GUI Options');
    expect(
      [...menu.querySelectorAll('[role="menuitem"]')]
        .filter(i => !(i as HTMLElement).hidden)
        .map(i => i.textContent?.trim()),
    ).toEqual(['Rename', 'Duplicate', 'Delete']);
  });

  it('should duplicate that GUI from Duplicate', () => {
    fireEvent.click(item(openMenu('Hud'), 'Duplicate'));

    expect(mocks.duplicateRoot).toHaveBeenCalledWith('src/ui/Hud.tsx');
  });

  it('should delete that GUI from Delete', () => {
    fireEvent.click(item(openMenu('Hud'), 'Delete'));

    expect(mocks.removeRoot).toHaveBeenCalledWith('src/ui/Hud.tsx');
  });

  it('should open the rename field from Rename', () => {
    fireEvent.click(item(openMenu('Hud'), 'Rename'));

    expect(screen.getByLabelText('Rename Hud')).toBeTruthy();
  });
});

describe('when a key is pressed on a focused GUI row', () => {
  afterEach(cleanup);

  it('should delete that GUI on Delete', () => {
    render(<CodeRootsList />);
    fireEvent.keyDown(rowFor('Hud'), { key: 'Delete' });

    expect(mocks.removeRoot).toHaveBeenCalledWith('src/ui/Hud.tsx');
  });

  it('should delete that GUI on Backspace', () => {
    render(<CodeRootsList />);
    fireEvent.keyDown(rowFor('Hud'), { key: 'Backspace' });

    expect(mocks.removeRoot).toHaveBeenCalledWith('src/ui/Hud.tsx');
  });

  it('should duplicate that GUI on Cmd/Ctrl + D, and keep the browser from bookmarking', () => {
    render(<CodeRootsList />);
    const event = new KeyboardEvent('keydown', {
      key: 'd',
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    rowFor('Hud').dispatchEvent(event);

    expect(mocks.duplicateRoot).toHaveBeenCalledWith('src/ui/Hud.tsx');
    expect(event.defaultPrevented).toBe(true);
  });

  it('should keep the key from reaching the node shortcuts', () => {
    const onDocumentKey = vi.fn();
    document.addEventListener('keydown', onDocumentKey);
    render(<CodeRootsList />);
    fireEvent.keyDown(rowFor('Hud'), { key: 'Delete' });
    document.removeEventListener('keydown', onDocumentKey);

    expect(onDocumentKey).not.toHaveBeenCalled();
  });

  describe('and a menu is open', () => {
    it('should do nothing', () => {
      mocks.menuOpen = true;
      render(<CodeRootsList />);
      fireEvent.keyDown(rowFor('Hud'), { key: 'Delete' });
      fireEvent.keyDown(rowFor('Hud'), { key: 'd', ctrlKey: true });

      expect(mocks.removeRoot).not.toHaveBeenCalled();
      expect(mocks.duplicateRoot).not.toHaveBeenCalled();
    });
  });

  describe('and the row is being renamed', () => {
    it('should leave Delete and Backspace to the name field', () => {
      render(<CodeRootsList />);
      fireEvent.doubleClick(rowFor('Hud').querySelector('.ui-designer-code-root-name')!);
      fireEvent.keyDown(screen.getByLabelText('Rename Hud'), { key: 'Backspace' });

      expect(mocks.removeRoot).not.toHaveBeenCalled();
    });
  });
});

describe('when a GUI is renamed', () => {
  afterEach(cleanup);

  it('should use the rail rename field and return focus to the row on Escape', () => {
    render(<CodeRootsList />);
    fireEvent.doubleClick(rowFor('Hud').querySelector('.ui-designer-code-root-name')!);
    const field = screen.getByLabelText('Rename Hud');

    expect(field.className).toMatch(/field/);
    fireEvent.keyDown(field, { key: 'Escape' });

    expect(screen.queryByLabelText('Rename Hud')).toBeNull();
    expect(document.activeElement).toBe(rowFor('Hud'));
  });

  it('should drop the eye while the name is being edited', () => {
    render(<CodeRootsList />);
    fireEvent.doubleClick(rowFor('Card').querySelector('.ui-designer-code-root-name')!);

    expect(screen.queryByLabelText('Hide Card')).toBeNull();
    expect(screen.getByLabelText('Hide Hud')).toBeTruthy();
  });
});

describe('when the rows draw their glyphs', () => {
  afterEach(cleanup);

  const svgOf = (ui: React.ReactElement) => {
    const html = render(ui).container.querySelector('svg')!.outerHTML;
    cleanup();
    return html;
  };

  it('should draw GUIRoot on a GUI row and the small Mobile on the MobileHUD row, at 16', () => {
    const gui = svgOf(<GUIRoot fontSize="small" />);
    const mobile = svgOf(<Mobile fontSize="small" />);
    render(<CodeRootsList />);

    expect(rowFor('Hud').querySelector('svg')!.outerHTML).toBe(gui);
    expect(document.querySelector('.ui-designer-mobile-hud-row svg')!.outerHTML).toBe(mobile);
  });
});

describe('when the search term filters the MobileHUD row', () => {
  afterEach(cleanup);

  const hudRow = () => document.querySelector('.ui-designer-mobile-hud-row');

  it.each(['zzz', 'ma'])('should hide it when "mobilehud" does not contain %j', term => {
    render(<CodeRootsList filter={term} />);

    expect(hudRow()).toBeNull();
  });

  it.each(['mobile', 'HUD', ' bileh '])('should keep it when "mobilehud" contains %j', term => {
    render(<CodeRootsList filter={term} />);

    expect(hudRow()).not.toBeNull();
  });
});

describe('when the stylesheet places the GUI eye', () => {
  const stylesheet = readFileSync(join(__dirname, 'CodeRootsList.css'), 'utf8');
  const rules = Array.from(stylesheet.matchAll(/([^{}]+)\{([^}]*)\}/g), ([, selector, body]) => ({
    selector: selector.replace(/\s+/g, ' ').trim(),
    body: body.replace(/\s+/g, ' ').trim(),
  }));
  const KEYBOARD_FOCUS_WITHIN =
    ':focus-within:not(:focus:not(:focus-visible)):not(:has(:focus:not(:focus-visible)))';

  it('should take the eye out of the layout at rest, so the label runs to the row padding', () => {
    const rest = rules.find(r => r.selector === '.ui-designer-code-root-actions');

    expect(rest?.body).toContain('display: none;');
    expect(rest?.body).not.toContain('visibility');
  });

  it('should bring it back on hover, on keyboard focus within the row, and while pressed', () => {
    const shown = rules.find(
      r =>
        r.body.includes('display: flex;') && r.selector.includes('.ui-designer-code-root-actions'),
    );

    expect(shown?.selector).toContain(':hover');
    expect(shown?.selector).toContain(KEYBOARD_FOCUS_WITHIN);
    expect(shown?.selector).toContain(
      '.ui-designer-code-root-actions:has(.ui-designer-code-root-toplevel[aria-pressed="true"])',
    );
  });

  it('should never show it on mouse focus, which a plain :focus-within would', () => {
    const plain = stylesheet.replaceAll(KEYBOARD_FOCUS_WITHIN, '');

    expect(plain).not.toContain(':focus-within');
  });
});
