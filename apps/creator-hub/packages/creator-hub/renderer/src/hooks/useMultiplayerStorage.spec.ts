import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { scene } from '#preload';

import { useMultiplayerStorage } from './useMultiplayerStorage';

afterEach(() => vi.mocked(scene.getScene).mockReset());

describe('useMultiplayerStorage', () => {
  it('is true when scene.json turns authoritativeMultiplayer on', async () => {
    vi.mocked(scene.getScene).mockResolvedValue({ authoritativeMultiplayer: true } as never);
    const { result } = renderHook(() => useMultiplayerStorage('/project'));
    await waitFor(() => expect(result.current).toBe(true));
    expect(scene.getScene).toHaveBeenCalledWith('/project');
  });

  it('is false when scene.json has it off or has no such key', async () => {
    vi.mocked(scene.getScene).mockResolvedValueOnce({ authoritativeMultiplayer: false } as never);
    const off = renderHook(() => useMultiplayerStorage('/off'));
    vi.mocked(scene.getScene).mockResolvedValueOnce({} as never);
    const absent = renderHook(() => useMultiplayerStorage('/absent'));
    await waitFor(() => expect(scene.getScene).toHaveBeenCalledTimes(2));
    expect(off.result.current).toBe(false);
    expect(absent.result.current).toBe(false);
  });

  it('is false when scene.json cannot be read, and without a project', async () => {
    vi.mocked(scene.getScene).mockRejectedValue(new Error('ENOENT'));
    const { result } = renderHook(() => useMultiplayerStorage('/missing'));
    await waitFor(() => expect(scene.getScene).toHaveBeenCalled());
    expect(result.current).toBe(false);
    expect(renderHook(() => useMultiplayerStorage(undefined)).result.current).toBe(false);
  });
});
