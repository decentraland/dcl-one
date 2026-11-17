// The place page's In-World Chat: joins the scene's room read-only and keeps the chat it hears.
import { useEffect, useState } from "react";
import type { SceneChat, SceneChatMessage } from "@ui/explorer/pages/PlaceDetail";

import { decodeChatPacket, fetchSceneRoom, type SceneRoomCredentials, type SceneRoomTarget } from "@/data/lib/comms/scene-chat";

const IDLE: SceneChat = { status: "loading", messages: [] };

// Any signer can join a scene room and publish, so the page bounds what it keeps. Both bounds are the Explorer
// chat's own: it keeps 200 lines (library/ui/apps/hud/features/session/useEngineSession.ts) and lets a player
// type 500 characters (library/ui/apps/hud/features/chat/Chat.tsx).
export const MAX_CHAT_LINES = 200;
export const MAX_CHAT_CHARS = 500;

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
 * With a target, asks the gatekeeper for the room, always as a guest (never the visitor's wallet) and never for a
 * world (see `fetchSceneRoom`), and listens to its chat without the room's media tracks; the panel is `loading`
 * until the room is known, `ready` once it is (connecting or connected) and `unavailable` when there is none.
 * Without a target nothing is asked and the panel stays `loading`.
 */
export function useSceneChat(target: SceneRoomTarget | null): SceneChat {
  const [credentials, setCredentials] = useState<SceneRoomCredentials | null>(null);
  const [status, setStatus] = useState<SceneChat["status"]>("loading");
  const [messages, setMessages] = useState<SceneChatMessage[]>([]);
  const location = target?.location ?? "";

  useEffect(() => {
    setStatus("loading");
    setCredentials(null);
    setMessages([]);
    if (!location) return;
    const controller = new AbortController();
    void fetchSceneRoom({ location }, controller.signal).then((room) => {
      if (controller.signal.aborted) return;
      setCredentials(room?.credentials ?? null);
      setStatus(room ? "ready" : "unavailable");
    });
    return () => controller.abort();
  }, [location]);

  useEffect(() => {
    if (!credentials) return;
    let cancelled = false;
    let leave = () => {};
    void (async () => {
      const { Room, RoomEvent } = await import("livekit-client");
      if (cancelled) return;
      const room = new Room();
      room.on(RoomEvent.DataReceived, (payload: Uint8Array, participant?: Participant) => {
        const text = decodeChatPacket(payload);
        if (text === null) return;
        const message = [...text].slice(0, MAX_CHAT_CHARS).join("");
        setMessages((prev) => [...prev, { sender: senderOf(participant), timestamp: Date.now(), message }].slice(-MAX_CHAT_LINES));
      });
      leave = () => void room.disconnect();
      // A room that cannot be joined keeps the empty panel, as the reference's does; the failure is only logged.
      // The panel shows text only: without autoSubscribe the room's voice and video tracks are not received.
      await room.connect(credentials.url, credentials.token, { autoSubscribe: false }).catch((error: unknown) => console.warn("[scene-chat] room connect failed", error));
    })();
    return () => {
      cancelled = true;
      leave();
    };
  }, [credentials]);

  return location ? { status, messages } : IDLE;
}
