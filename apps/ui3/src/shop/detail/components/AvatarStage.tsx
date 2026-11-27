import { useEffect, useState } from "react";
import { AvatarStage as Stage, type AvatarStageProps } from "@dcl/one-ui/apps/shop/preview";
import { REMOTE_FLAG, useRemoteFlagState } from "../../discovery/lib/remoteFlags";
import { pickRenderer } from "../lib/pickRenderer";

export default function AvatarStage(props: Omit<AvatarStageProps, "device" | "unityFlag" | "bevyFlag">) {
  // Read on the client only: the device decides the renderer, and whether to draw at all.
  const [device, setDevice] = useState<"full" | "lite" | "none" | null>(null);
  useEffect(() => {
    const decision = pickRenderer();
    setDevice(decision.reason === "no-webgl" ? "none" : decision.renderer);
  }, []);
  const full = device === "full";
  const unityFlag = useRemoteFlagState(REMOTE_FLAG.unityWearablePreview, full);
  const bevyFlag = useRemoteFlagState(REMOTE_FLAG.bevyAvatarPreview, full);
  return <Stage {...props} device={device} unityFlag={unityFlag} bevyFlag={bevyFlag} />;
}
