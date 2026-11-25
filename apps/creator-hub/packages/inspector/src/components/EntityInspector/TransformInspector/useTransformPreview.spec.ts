import { renderHook, act } from '@testing-library/react';
import type { Entity, TransformType } from '@dcl/ecs';

import { createEmitter } from '../../../lib/utils/emitter';
import type { RendererEvents } from '../../../lib/renderer/types';
import { useTransformPreview } from './useTransformPreview';

const TRANSFORM_ID = 1;
const entity = 512 as Entity;
const other = 513 as Entity;

const transformAt = (x: number): TransformType => ({
  position: { x, y: 0, z: 0 },
  rotation: { x: 0, y: 0, z: 0, w: 1 },
  scale: { x: 1, y: 1, z: 1 },
});

let events: ReturnType<typeof createEmitter<RendererEvents>>;
let stored: Map<Entity, TransformType>;
let triggerChange: (event: any, sdk: any) => void;
let mockSdk: any;

vi.mock('../../../hooks/sdk/useSdk', () => ({ useSdk: () => mockSdk }));
vi.mock('../../../hooks/sdk/useChange', () => ({
  useChange: (callback: any) => {
    triggerChange = callback;
  },
}));

describe('useTransformPreview', () => {
  beforeEach(() => {
    events = createEmitter<RendererEvents>();
    stored = new Map([
      [entity, transformAt(1)],
      [other, transformAt(5)],
    ]);
    mockSdk = {
      components: {
        Transform: {
          componentId: TRANSFORM_ID,
          getOrNull: (target: Entity) => stored.get(target) ?? null,
        },
      },
      renderer: { events },
    };
  });

  describe('when no drag is in progress', () => {
    it('should have no preview', () => {
      const { result } = renderHook(() => useTransformPreview([entity]));

      expect(result.current.preview).toBeNull();
    });
  });

  describe('when the renderer previews a drag', () => {
    it('should show the previewed values', () => {
      const { result } = renderHook(() => useTransformPreview([entity]));

      act(() =>
        events.emit('previewTransforms', { transforms: [{ entity, ...transformAt(2.5) }] }),
      );

      expect(result.current.preview?.position.x).toBe('2.50');
    });

    it('should ignore an entity outside the selection', () => {
      const { result } = renderHook(() => useTransformPreview([entity]));

      act(() =>
        events.emit('previewTransforms', { transforms: [{ entity: other, ...transformAt(9) }] }),
      );

      expect(result.current.preview).toBeNull();
    });
  });

  describe('when the renderer writes an undispatched commit', () => {
    it('should show the value the engine holds', () => {
      const { result } = renderHook(() => useTransformPreview([entity]));

      stored.set(entity, transformAt(3));
      act(() => events.emit('gizmoCommit', { transforms: [{ entity }] }));

      expect(result.current.preview?.position.x).toBe('3.00');
    });
  });

  describe('when the drag ends', () => {
    it('should keep the committed value until the engine reports the change', () => {
      const { result } = renderHook(() => useTransformPreview([entity]));
      act(() =>
        events.emit('previewTransforms', { transforms: [{ entity, ...transformAt(2.5) }] }),
      );

      stored.set(entity, transformAt(2));
      act(() => events.emit('gizmoCommitEnd', undefined));
      expect(result.current.preview?.position.x).toBe('2.00');

      act(() => triggerChange({ entity, component: { componentId: TRANSFORM_ID } }, mockSdk));
      expect(result.current.preview).toBeNull();
    });
  });

  describe('when a field takes focus', () => {
    it('should drop the preview', () => {
      const { result } = renderHook(() => useTransformPreview([entity]));
      act(() =>
        events.emit('previewTransforms', { transforms: [{ entity, ...transformAt(2.5) }] }),
      );

      act(() => result.current.clearPreview());

      expect(result.current.preview).toBeNull();
    });
  });
});
