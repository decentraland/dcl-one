// The place page's In-World Chat: joins the scene's room read-only and keeps the chat it hears.
// from: sites@17b3535a:src/hooks/useSceneRoom.ts, src/components/places/SceneLiveWatcher (SceneRoomMount), src/hooks/useChat.ts
import { useEffect, useState, useSyncExternalStore } from "react";
import type { SceneChat, SceneChatMessage } from "@ui/explorer/pages/PlaceDetail";

import { getIdentity, getServerSnapshot, subscribe } from "@/data/lib/auth/session";
import { decodeChatPacket, fetchSceneRoom, type SceneRoomCredentials, type SceneRoomTarget } from "@/data/lib/comms/scene-chat";

const IDLE: SceneChat = { status: "loading", messages: [] };

type Participant = { identity: string; metadata?: string };

/** The sender as the room names it: a wallet, else the display name its metadata carries, else its identity. */
export function senderOf(participant: Participant | undefined): string {
  const identity = participant?.identity;
  if (!identity) return "Unknown";
  if (identity.startsWith("0x")) return identity;
  try {
    const metadata = participant?.metadata ? (JSON.parse(participant.metadata) as { displayName?: unknown }) : null;
    if (typeof metadata?.displayName === "string") return metadata.displayName;
  } catch {
    // not JSON: the identity names the sender
  }
  return identity;
}

/**
 * With a target, asks the gatekeeper for the room (signed as the wallet when signed in, else as a guest) and listens to
 * its chat; the panel is `loading` until the room is known, `ready` once it is (connecting or connected) and
 * `unavailable` when there is none. Without a target nothing is asked and the panel stays `loading`.
 */
export function useSceneChat(target: SceneRoomTarget | null): SceneChat {
  const identity = useSyncExternalStore(subscribe, getIdentity, getServerSnapshot);
  const [credentials, setCredentials] = useState<SceneRoomCredentials | null>(null);
  const [status, setStatus] = useState<SceneChat["status"]>("loading");
  const [messages, setMessages] = useState<SceneChatMessage[]>([]);
  const location = target?.location ?? "";
  const parcel = target?.parcel;
  const sceneId = target?.sceneId;

  useEffect(() => {
    setStatus("loading");
    setCredentials(null);
    setMessages([]);
    if (!location) return;
    const controller = new AbortController();
    void fetchSceneRoom({ location, parcel, sceneId }, identity, controller.signal).then((room) => {
      if (controller.signal.aborted) return;
      setCredentials(room?.credentials ?? null);
      setStatus(room ? "ready" : "unavailable");
    });
    return () => controller.abort();
  }, [location, parcel, sceneId, identity]);

  useEffect(() => {
    if (!credentials) return;
    let cancelled = false;
    let leave = () => {};
    void (async () => {
      const { Room, RoomEvent } = await import("livekit-client");
      if (cancelled) return;
      const room = new Room();
      room.on(RoomEvent.DataReceived, (payload: Uint8Array, participant?: Participant) => {
        const message = decodeChatPacket(payload);
        if (message === null) return;
        setMessages((prev) => [...prev, { sender: senderOf(participant), timestamp: Date.now(), message }]);
      });
      leave = () => void room.disconnect();
      // A room that cannot be joined keeps the empty panel, as the reference's does; the failure is only logged.
      await room.connect(credentials.url, credentials.token).catch((error: unknown) => console.warn("[scene-chat] room connect failed", error));
    })();
    return () => {
      cancelled = true;
      leave();
    };
  }, [credentials]);

  return location ? { status, messages } : IDLE;
}
