import type { SceneToPageMessage } from './bridge-protocol'

/** The pointer and selection may change while renderer writes/export complete.
 * Keep this completion immutable; the following tick reports the next drag. */
export async function prepareDragTransition(
  dragging: boolean,
  playing: boolean,
  transforms: Record<string, unknown>,
  checkpoint: () => Promise<unknown>
): Promise<{ dragging: boolean; message: Extract<SceneToPageMessage, { type: 'drag-start' | 'drag-end' }> }> {
  const message: Extract<SceneToPageMessage, { type: 'drag-start' | 'drag-end' }> = dragging
    ? { type: 'drag-start' }
    : { type: 'drag-end', transforms: JSON.parse(JSON.stringify(transforms)) as Record<string, unknown> }
  if (!dragging && !playing) await checkpoint()
  return { dragging, message }
}
