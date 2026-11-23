import { useEffect, useState } from 'react';

import { scene } from '#preload';

/**
 * True when the scene.json of the project turns `authoritativeMultiplayer` on. Such a scene can keep
 * data in Multiplayer Server storage, which a deployment as a new place may erase.
 */
export function useMultiplayerStorage(projectPath: string | undefined): boolean {
  const [storage, setStorage] = useState(false);
  useEffect(() => {
    setStorage(false);
    if (!projectPath) return;
    let current = true;
    void (async () => {
      try {
        const json = (await scene.getScene(projectPath)) as
          | { authoritativeMultiplayer?: unknown }
          | undefined;
        if (current) setStorage(json?.authoritativeMultiplayer === true);
      } catch {
        // Without scene.json the warning leaves the line out.
      }
    })();
    return () => {
      current = false;
    };
  }, [projectPath]);
  return storage;
}
