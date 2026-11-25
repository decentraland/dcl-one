import { useCallback, useEffect, useRef, useState } from 'react';
import type { Entity, TransformType } from '@dcl/ecs';

import { mergeComponentValues } from '../../../hooks/sdk/useComponentInput';
import { useChange } from '../../../hooks/sdk/useChange';
import { useSdk } from '../../../hooks/sdk/useSdk';
import type { RendererEvents } from '../../../lib/renderer/types';
import { fromTransform } from './utils';
import type { TransformInput } from './types';

/**
 * The Transform values of a gizmo drag in progress, for display only.
 *
 * A drag reaches the engine's change stream once, when it ends: that single
 * dispatch is what saves the scene and reloads a running preview. Until then the
 * renderer reports the drag over its event bus (`previewTransforms` from a
 * renderer that previews out of process, `gizmoCommit` from one that writes the
 * undispatched Transform every frame), and this hook turns those reports into
 * the panel's input values without writing anything.
 *
 * Call it after the hook that owns the inputs: the preview is released by an
 * effect, which has to run after that hook's own engine→inputs effect so the
 * committed value is on screen before the preview goes away.
 */
export function useTransformPreview(entities: Entity[]) {
  const sdk = useSdk();
  const [preview, setPreview] = useState<TransformInput | null>(null);
  const [committed, setCommitted] = useState(0);
  const live = useRef(new Map<Entity, TransformType>());
  const entitiesKey = entities.join(',');

  const clearPreview = useCallback(() => {
    live.current.clear();
    setPreview(null);
  }, []);

  useEffect(() => {
    if (!sdk) return;
    const { Transform } = sdk.components;
    const events = sdk.renderer.events;
    const readEngine = (entity: Entity) => Transform.getOrNull(entity) as TransformType | null;

    const publish = () => {
      if (live.current.size === 0) return;
      const values = entities
        .map(entity => live.current.get(entity) ?? readEngine(entity))
        .filter((value): value is TransformType => !!value);
      setPreview(values.length > 0 ? mergeComponentValues(values, fromTransform) : null);
    };
    const trackEngineValue = (entity: Entity) => {
      const value = entities.includes(entity) ? readEngine(entity) : null;
      if (value) live.current.set(entity, value);
    };

    const onPreview = ({ transforms }: RendererEvents['previewTransforms']) => {
      for (const transform of transforms) {
        if (entities.includes(transform.entity)) live.current.set(transform.entity, transform);
      }
      publish();
    };
    // The reverse channel subscribed first, so the engine already holds the
    // merged, snapped value of this commit.
    const onCommit = ({ transforms }: RendererEvents['gizmoCommit']) => {
      for (const transform of transforms) trackEngineValue(transform.entity);
      publish();
    };
    const onCommitEnd = () => {
      for (const entity of [...live.current.keys()]) trackEngineValue(entity);
      publish();
    };

    events.on('previewTransforms', onPreview);
    events.on('gizmoCommit', onCommit);
    events.on('gizmoCommitEnd', onCommitEnd);
    return () => {
      events.off('previewTransforms', onPreview);
      events.off('gizmoCommit', onCommit);
      events.off('gizmoCommitEnd', onCommitEnd);
      clearPreview();
    };
  }, [sdk, entitiesKey]);

  useChange(
    (event, { components }) => {
      if (
        entities.includes(event.entity) &&
        event.component?.componentId === components.Transform.componentId
      ) {
        setCommitted(count => count + 1);
      }
    },
    [entitiesKey],
  );

  useEffect(() => {
    if (committed > 0) clearPreview();
  }, [committed]);

  return { preview, clearPreview };
}
