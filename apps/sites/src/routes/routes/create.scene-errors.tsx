import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router";

import Button from "@ui/atoms/Button";
import { PageHeader } from "@ui/components/Surface";
import CreatorHubChrome from "@ui/creatorhub/frames/CreatorHubChrome";
import "@ui/creatorhub/pages/chmanage.css";
import { useAuth } from "@/data/lib/auth/index";
import { openSignIn } from "@/features/components/auth/signin-store";
import { fetchSceneErrors, type SceneError } from "@/data/lib/catalyst/creator-hub/scene-errors";
import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";

export const meta = () => creatorHubMeta("Scene errors");

export default function CreatorHubSceneErrors() {
  const { identity } = useAuth();
  const [params] = useSearchParams();
  const type = params.get("type");
  const scene = params.get("scene") ?? "";
  const [errors, setErrors] = useState<SceneError[] | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const selectedType = type === "genesis" || type === "world" ? type : null;

  useEffect(() => {
    setErrors(null);
    setFailure(null);
    if (!identity || !selectedType || !scene) return;
    const controller = new AbortController();
    fetchSceneErrors(identity, selectedType, scene, controller.signal)
      .then((rows) => { if (!controller.signal.aborted) setErrors(rows); })
      .catch((err: unknown) => {
        if (!controller.signal.aborted)
          setFailure(err instanceof Error ? err.message : "Diagnostics unavailable");
      });
    return () => controller.abort();
  }, [identity, selectedType, scene, refresh]);

  return (
    <CreatorHubChrome active="metrics">
      <section className="chm">
        <div className="chm__container">
          <PageHeader title="Scene errors" />
          {!selectedType || !scene ? (
            <p>Choose a scene from <Link to="/create/land">your LAND</Link> or Worlds.</p>
          ) : !identity ? (
            <div><p>Sign in to see errors for your scene.</p><Button variant="secondary" onClick={() => openSignIn()}>Sign in</Button></div>
          ) : failure ? (
            <div role="alert"><p>{failure}</p><Button variant="secondary" onClick={() => setRefresh((n) => n + 1)}>Try again</Button></div>
          ) : errors === null ? (
            <p role="status">Loading scene errors…</p>
          ) : (
            <>
              <p>{selectedType === "genesis" ? `Genesis ${scene}` : scene} · latest 100 reports</p>
              {errors.length === 0 ? <p>No scene errors have been reported.</p> : (
                <ul>
                  {errors.map((entry, index) => (
                    <li key={`${entry.received_at}:${entry.scene_hash}:${index}`}>
                      <time dateTime={entry.received_at}>{new Date(entry.received_at).toLocaleString()}</time>
                      {` · ${entry.error_kind} · ${entry.error_message}`}
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
      </section>
    </CreatorHubChrome>
  );
}
