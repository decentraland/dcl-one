import { useEffect, useState } from "react";
import type { AvatarStage } from "../../explorer/components/AvatarPreview";
import Button from "../../atoms/Button";

/** Keep the optional 3D renderer out of the item-selection screen's startup. */
export default function WearableEditorPreview({
  enabled,
  editing,
}: {
  enabled: boolean;
  editing: boolean;
}) {
  const [requested, setRequested] = useState(false);
  const [Preview, setPreview] = useState<typeof AvatarStage | null>(null);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const showPreview = enabled && (editing || requested);

  useEffect(() => {
    if (!showPreview || Preview) return;
    let cancelled = false;
    setFailed(false);
    void import("../../explorer/components/AvatarPreview").then(
      ({ AvatarStage }) => {
        if (!cancelled) setPreview(() => AvatarStage);
      },
      () => {
        if (!cancelled) setFailed(true);
      },
    );
    return () => { cancelled = true; };
  }, [showPreview, Preview, attempt]);

  if (showPreview && Preview) return <Preview className="cwie-wizard__avatar" />;

  return (
    <div className="cwie-wizard__preview-prompt">
      {!showPreview ? (
        <Button variant="secondary" disabled={!enabled} onClick={() => setRequested(true)}>
          Load avatar preview
        </Button>
      ) : failed ? (
        <>
          <p role="status">Avatar preview could not load.</p>
          <Button variant="secondary" onClick={() => setAttempt((n) => n + 1)}>Retry preview</Button>
        </>
      ) : <p role="status">Loading avatar preview…</p>}
    </div>
  );
}
