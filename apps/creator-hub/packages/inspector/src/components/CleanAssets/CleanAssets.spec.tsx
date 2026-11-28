import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import CleanAssets, { getCodeNote } from './CleanAssets';
import type { AssetFile } from './types';

vi.mock('../../redux/hooks', () => ({ useAppDispatch: () => vi.fn() }));
vi.mock('../../hooks/useSnackbar', () => ({ useSnackbar: () => ({ pushNotification: vi.fn() }) }));

const assets: AssetFile[] = [
  { path: 'assets/unused.png', size: 1, unused: true },
  { path: 'assets/in-code.png', size: 1, unused: false, usedByCode: true },
  { path: 'assets/in-scene.glb', size: 1, unused: false },
];

function renderModal(selected: string[], codeScanned = true) {
  render(
    <CleanAssets
      isOpen
      assets={assets}
      codeScanned={codeScanned}
      isScanning={false}
      selectedAssets={new Set(selected)}
      onClose={vi.fn()}
      onScan={vi.fn()}
      onSelect={vi.fn()}
      onSelectAll={vi.fn()}
    />,
  );
}

describe('CleanAssets', () => {
  describe('when only unused files are ticked', () => {
    it('should show no in-use warning and say that named files are kept', () => {
      renderModal(['assets/unused.png']);
      expect(screen.queryByTitle(/uses this file/)).toBeNull();
      expect(screen.getByText(/Files that your code names are kept\./)).toBeTruthy();
    });
  });

  describe('when a file the code names is ticked', () => {
    it('should warn on that row and change the note', () => {
      renderModal(['assets/in-code.png']);
      expect(screen.getByTitle('Your code uses this file')).toBeTruthy();
      expect(screen.getByText(/At least one of the files you marked for deletion/)).toBeTruthy();
      expect(screen.queryByText(/Files that your code names are kept\./)).toBeNull();
    });
  });

  describe('when a file the scene uses is ticked', () => {
    it('should warn on that row', () => {
      renderModal(['assets/in-scene.glb']);
      expect(screen.getByTitle('Your scene uses this file')).toBeTruthy();
    });
  });

  describe('when the code was not scanned', () => {
    it('should not say that named files are kept', () => {
      renderModal(['assets/unused.png'], false);
      expect(screen.queryByText(/Files that your code names are kept\./)).toBeNull();
      expect(screen.getByText(/No scene code was read/)).toBeTruthy();
    });
  });
});

describe('getCodeNote', () => {
  it('should keep the unread-code caveat when an in-use file is ticked', () => {
    expect(getCodeNote(true, false)).toMatch(
      /used by your scene or its code\. No scene code was read/,
    );
  });
});
