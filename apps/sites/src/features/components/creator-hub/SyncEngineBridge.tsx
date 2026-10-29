import { useEffect } from "react";

import { getSharedSyncEngine } from "@ui/creatorhub/lib/shared-sync";

import { putServerDraft } from "@/data/lib/catalyst/creator-hub/scene-drafts-client";

export default function SyncEngineBridge() {
  useEffect(() => {
    const engine = getSharedSyncEngine();

    engine.setTransport(async (id, body) => {
      const result = await putServerDraft(id, body);
      if (result.ok) return { ok: true, version: result.version };
      if (result.conflict) {
        return { ok: false, conflict: true, version: result.version, server: result.server?.blob };
      }
      throw new Error(`sync ${result.status}`);
    });

    engine.start();
    return () => engine.stop();
  }, []);

  return null;
}
