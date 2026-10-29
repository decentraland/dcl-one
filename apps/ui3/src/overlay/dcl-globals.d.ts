import type { BridgeApi, DeployIdentity } from "./bridge";
import type { NativeHostMessage } from "../generated/bridge/NativeHostMessage";

declare global {
  type DclGridStep = [label: string, state: "done" | "active" | "waiting" | "warn" | "failed", detail?: string];
  interface DclGridData { title?: string; seed?: string; steps?: DclGridStep[]; rows?: [string, string][]; bytes?: number; rate?: number; done?: number; failed?: number }
  interface DclGridHandle { progress(percent: number): void; data(data: DclGridData): void; release(): void }
  interface Window {
    dclBridge?: BridgeApi;
    __dclNativeHost?: { post: (msg: NativeHostMessage) => void };
    dclDeployIdentity?: DeployIdentity;
    __dclHasIdentity?: boolean;
    __dclConsumedOpenPanelNonce?: number;

    dclDeferStart?: boolean;
    dclEngineReady?: boolean;
    dclBootstrapSnapshot?: { phase: "Idle" | "Prerequisites" | "Initializing" | "Ready" | "Starting" | "Lobby" | "EnteringWorld" | "World" | "Failed"; error: string | null };
    dclEngineConnectLobby?: () => void | Promise<void>;
    dclEngineStart?: (destination?: { realm?: string; parcel?: [number, number] }) => void | Promise<void>;

    dclLoadingProgress?: number;
    __DCL_STARTUP_DETAILS_OPEN__?: boolean;
    dclGridLoader?: { claim(): DclGridHandle };

    __emojiFontBytes?: Uint8Array;
    __assetsBundle?: Uint8Array;
    __DCL_PLAY_SCREEN__?: { url: string; response: Promise<Response>; used?: boolean };

    engine_console_command?: (command: string) => Promise<string>;

    dclDracoReady?: Promise<boolean>;
    dclDracoDecode?: (
      srcBytes: Uint8Array,
      attrMap: Record<string, string>,
    ) => Promise<{
      indices: Uint32Array;
      numPoints: number;
      attributes: Record<string, { data: Float32Array; components: number }>;
    }>;

    __DCL_PUBLIC__?: { walletConnectProjectId?: string; thirdwebClientId?: string; thirdwebSignProxy?: string; docsBase?: string };
    __DCL_AUTH_HEADERS__?: Record<string, string>;

    __srch?: unknown;
  }
}
