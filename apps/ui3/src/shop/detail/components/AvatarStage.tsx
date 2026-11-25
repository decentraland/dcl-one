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
  const [unityFailed, setUnityFailed] = useState(false);
  // What the preview drew and what it failed on, each for the frame and options it was on.
  const [drawn, setDrawn] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);

  // A lighter device draws with Babylon whatever the flag says, so it does not wait for it.
  const settled = device === "lite" || (device === "full" && unityFlag !== null);
  const unity = !unityFailed && device === "full" && unityFlag === true;
  const frame = `${unity ? "unity" : "babylon"}|${previewBase(options.urns)}`;
  const scene = `${frame}|${JSON.stringify(options)}`;
  // A Babylon failure keeps the stage down for those options only: the next change tries again.
  const mounted = settled && failed !== scene;
  const ready = mounted && drawn === scene;

  if (device === "none") return <>{unavailable ?? cover}</>;
  return (
    <>
      {mounted ? (
        <PreviewFrame
          key={frame}
          options={{ ...options, unity: unity || undefined, unityMode: unity ? unityMode : undefined }}
          loadTimeoutMs={unity ? UNITY_LOAD_TIMEOUT_MS : undefined}
          onLoad={() => setDrawn(scene)}
          onError={() => (unity ? setUnityFailed(true) : setFailed(scene))}
        />
      ) : null}
      {ready ? null : settled && !mounted ? (unavailable ?? cover) : cover}
    </>
  );
}
