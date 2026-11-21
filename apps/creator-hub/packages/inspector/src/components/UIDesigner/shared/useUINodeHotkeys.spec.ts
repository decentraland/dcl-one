import { renderHook } from '@testing-library/react';

import { useUINodeHotkeys } from './useUINodeHotkeys';

const remove = vi.fn();
const duplicate = vi.fn();
let explorerActive = false;

vi.mock('../../../redux/hooks', () => ({
  useAppSelector: (selector: { name?: string }) =>
    selector.name === 'isAssetExplorerActive' ? explorerActive : 7,
}));
vi.mock('./useUINodeActions', () => ({ useUINodeActions: () => ({ remove, duplicate }) }));

describe('useUINodeHotkeys', () => {
  const press = (key: string, init: KeyboardEventInit = {}) =>
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, ...init }));
  const openMenu = () => {
    const menu = document.createElement('div');
    menu.setAttribute('role', 'menu');
    document.body.appendChild(menu);
    return menu;
  };

  let container: HTMLDivElement;
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    Object.defineProperty(container, 'offsetParent', { get: () => document.body });
    remove.mockClear();
    duplicate.mockClear();
  });
  afterEach(() => {
    document.body.innerHTML = '';
    explorerActive = false;
  });

  describe('when the Local Assets explorer owns the keyboard', () => {
    it('should leave Delete to the explorer', () => {
      explorerActive = true;
      renderHook(() => useUINodeHotkeys({ current: container }));
      press('Delete');
      expect(remove).not.toHaveBeenCalled();
    });
  });

  describe('when the explorer does not own the keyboard', () => {
    it('should delete the selected UI node', () => {
      renderHook(() => useUINodeHotkeys({ current: container }));
      press('Delete');
      expect(remove).toHaveBeenCalledWith(7);
    });
  });

  describe('when a context menu is open', () => {
    it('should leave Delete and Backspace to the menu', () => {
      openMenu();
      renderHook(() => useUINodeHotkeys({ current: container }));
      press('Delete');
      press('Backspace');
      expect(remove).not.toHaveBeenCalled();
    });

    it('should neither duplicate nor copy the selected node', () => {
      const menu = openMenu();
      renderHook(() => useUINodeHotkeys({ current: container }));
      press('d', { ctrlKey: true });
      press('c', { metaKey: true });
      menu.remove();
      press('v', { metaKey: true });
      expect(duplicate).not.toHaveBeenCalled();
    });

    it('should act on the node again once the menu closes', () => {
      const menu = openMenu();
      renderHook(() => useUINodeHotkeys({ current: container }));
      menu.remove();
      press('d', { ctrlKey: true });
      expect(duplicate).toHaveBeenCalledWith(7);
    });
  });
});
