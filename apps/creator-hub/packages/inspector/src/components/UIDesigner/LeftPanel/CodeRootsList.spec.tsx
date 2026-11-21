import React from 'react';
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

// Rows are queried through the name span rather than getByRole: under happy-dom
// the accessibility-tree filter reports this subtree inaccessible (same reason
// CallbackField.spec reaches for querySelector).
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
    fireEvent.click(screen.getByLabelText('Toggle top-level for Card'));

    expect(mocks.toggleTopLevel).toHaveBeenCalledWith('src/ui/Card.tsx');
  });

  it('should not select the root when a row action is used', () => {
    render(<CodeRootsList />);
    fireEvent.click(screen.getByLabelText('Toggle top-level for Card'));

    expect(mocks.selectRootFile).not.toHaveBeenCalled();
  });
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
