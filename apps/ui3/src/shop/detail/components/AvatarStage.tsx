// An avatar dressed in a set of items, in the wearable-preview iframe: the fitting room and the outfit pages, as
// decentraland.org's Shop draws them (shop@32377959 FittingRoom, OutfitPreview). Unity where the
// `unity-wearable-preview` flag and the device allow it, Babylon otherwise, and Babylon for the rest of the stage's life
// once Unity fails or stays silent. Covered until the preview has drawn what it was last sent.

import { useEffect, useState, type ReactNode } from "react";

import { REMOTE_FLAG, useRemoteFlagState } from "../../discovery/lib/remoteFlags";
import { pickRenderer } from "../lib/pickRenderer";
import PreviewFrame, { previewBase, UNITY_LOAD_TIMEOUT_MS, type PreviewFrameOptions } from "./PreviewFrame";

type Props = {
  /** What to draw. The renderer is the stage's to pick. */
  options: Omit<PreviewFrameOptions, "unity" | "unityMode">;
  /** Unity's mode: `builder`, the default, draws a mannequin in exactly these items. */
  unityMode?: "builder" | "marketplace";
  /** Over the stage until the preview has drawn what it was last sent. */
  cover?: ReactNode;
  /** In the preview's place when it cannot draw: no WebGL, or Babylon failed. The cover when absent. */
  unavailable?: ReactNode;
};

export default function AvatarStage({ options, unityMode = "builder", cover = null, unavailable }: Props) {
  // Read on the client only: the device decides between Unity and Babylon, and whether to draw at all.
  const [device, setDevice] = useState<"full" | "lite" | "none" | null>(null);
  useEffect(() => {
    const decision = pickRenderer();
    setDevice(decision.reason === "no-webgl" ? "none" : decision.renderer);
  }, []);
  const unityFlag = useRemoteFlagState(REMOTE_FLAG.unityWearablePreview, device === "full");
  // Unity failing sends the stage to Babylon for the rest of its life.
  const [unityFailed, setUnityFailed] = useState(false);

  // A lighter device draws with Babylon whatever the flag says, so it does not wait for it.
  const settled = device === "lite" || (device === "full" && unityFlag !== null);
  const unity = !unityFailed && device === "full" && unityFlag === true;
  // Unity's mode is read once, from the mount URL: a new one is a new frame, drawn again from the start.
  const frame = `${unity ? `unity:${unityMode}` : "babylon"}|${previewBase(options.urns)}`;
  const scene = `${frame}|${JSON.stringify(options)}`;
  // Whether the preview has drawn or failed belongs to this visit to these options: coming back to options drawn or
  // failed before starts over, covered and mounted. Reset during render, so no frame shows the last visit's answer.
  const [visit, setVisit] = useState({ scene, drawn: false, failed: false });
  const current = visit.scene === scene ? visit : { scene, drawn: false, failed: false };
  if (current !== visit) setVisit(current);
  const answer = (change: { drawn: true } | { failed: true }) => setVisit((v) => (v.scene === scene ? { ...v, ...change } : v));
  // A Babylon failure keeps the stage down for this visit only: the next change tries again.
  const mounted = settled && !current.failed;
  const ready = mounted && current.drawn;

  if (device === "none") return <>{unavailable ?? cover}</>;
  return (
    <>
      {mounted ? (
        <PreviewFrame
          key={frame}
          options={{ ...options, unity: unity || undefined, unityMode: unity ? unityMode : undefined }}
          loadTimeoutMs={unity ? UNITY_LOAD_TIMEOUT_MS : undefined}
          onLoad={() => answer({ drawn: true })}
          // A fresh iframe has drawn nothing yet, even for the same items (Unity booting again).
          onReplaced={() => setVisit((v) => (v.scene === scene ? { ...v, drawn: false } : v))}
          onError={() => (unity ? setUnityFailed(true) : answer({ failed: true }))}
        />
      ) : null}
      {ready ? null : settled && !mounted ? (unavailable ?? cover) : cover}
    </>
  );
}
