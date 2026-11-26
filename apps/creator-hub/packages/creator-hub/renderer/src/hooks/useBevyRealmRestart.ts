import { useEffect, useRef } from 'react';

import type { BevyRealmSupervisionEvent } from '/shared/types/ipc';

import { editor } from '#preload';

type StoppedEvent = Extract<BevyRealmSupervisionEvent, { kind: 'stopped' }>;

export type BevyRealmRestartHandlers = {
  /** The realm or its Multiplayer Server stopped; main restarts it unless `restartInMs` is null. */
  onStopped: (event: StoppedEvent) => void;
  /** Main is replacing the realm now; the current realm URLs stop working. */
  onRestarting: () => void;
  onRestarted: (realm: { url: string; wsUrl: string }) => void;
};

/**
 * Follows main's restarts of the project's Bevy realm (bevy-realm.ts owns the restart, its
 * backoff and its retry cap), so the editor can tell the user and reload on the new realm.
 */
export function useBevyRealmRestart(
  projectPath: string | undefined,
  handlers: BevyRealmRestartHandlers,
): void {
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    if (!projectPath) return;
    return editor.onBevyRealmSupervision(event => {
      if (event.path !== projectPath) return;
      const current = handlersRef.current;
      if (event.kind === 'stopped') current.onStopped(event);
      else if (event.kind === 'restarting') current.onRestarting();
      else current.onRestarted({ url: event.url, wsUrl: event.wsUrl });
    });
  }, [projectPath]);
}
