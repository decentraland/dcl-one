import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { CreateButton } from '.';

vi.mock('/@/hooks/useWorkspace', () => ({
  useWorkspace: () => ({
    createProject: vi.fn(),
    getAvailableProject: vi.fn(async () => [new Error('unused'), null]),
    importProject: vi.fn(),
  }),
}));
vi.mock('../Modals/CreateProject', () => ({ CreateProject: () => null }));

function stubNarrow(matches: boolean) {
  vi.spyOn(window, 'matchMedia').mockImplementation(
    query =>
      ({
        matches: query === '(max-width: 1147px)' ? matches : false,
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
      }) as unknown as MediaQueryList,
  );
}

const renderButton = () =>
  render(
    <TestProviders>
      <CreateButton />
    </TestProviders>,
  );

const labelHalf = () => screen.getByRole('button', { name: 'Create' });

async function hover(element: HTMLElement) {
  await act(async () => {
    fireEvent.mouseOver(element);
    await new Promise(resolve => setTimeout(resolve, 150));
  });
}

describe('CreateButton', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  describe('when it renders', () => {
    it('should name the label half "Create" and the menu half "Create options"', () => {
      stubNarrow(false);
      renderButton();

      expect(labelHalf().getAttribute('data-testid')).toBe('create-button');
      const menuHalf = screen.getByRole('button', { name: 'Create options' });
      expect(menuHalf.getAttribute('data-testid')).toBe('create-button-toggle');
      expect(menuHalf.getAttribute('aria-haspopup')).toBe('menu');
    });
  });

  describe('when the menu half is clicked twice', () => {
    it('should open the menu, then close it', () => {
      stubNarrow(false);
      renderButton();
      const menuHalf = screen.getByRole('button', { name: 'Create options' });

      fireEvent.click(menuHalf);
      expect(screen.getByRole('menu')).toBeTruthy();
      expect(menuHalf.getAttribute('aria-expanded')).toBe('true');

      fireEvent.click(menuHalf);
      expect(screen.queryByRole('menu')).toBeNull();
      expect(menuHalf.getAttribute('aria-expanded')).toBeNull();
    });
  });

  describe('when the window is wide', () => {
    it('should show no tooltip on the label half', async () => {
      stubNarrow(false);
      renderButton();

      await hover(labelHalf());

      expect(screen.queryByRole('tooltip')).toBeNull();
    });
  });

  describe('when the window is narrow enough for the icon-only label', () => {
    it('should keep the name "Create" and show it as a tooltip on hover', async () => {
      stubNarrow(true);
      renderButton();

      await hover(labelHalf());

      expect((await screen.findByRole('tooltip')).textContent).toBe('Create');
      expect(labelHalf()).toBeTruthy();
    });
  });
});
