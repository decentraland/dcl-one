import { useRef, useState } from "react";
import type { SocialMember } from "../explorer/pages/Communities";
import CompactNavigation from "../explorer/components/CompactNavigation";
import ConfirmDialog from "../explorer/components/ConfirmDialog";

type Action = "moderator" | "member" | "remove";
export default function CommunityMemberActions({ member, role, viewer, write }: {
  member: SocialMember;
  role?: string | null;
  viewer: string | null;
  write: (method: "PATCH" | "DELETE", address: string, body?: { role: string }) => Promise<void>;
}) {
  const [action, setAction] = useState<Action | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const pending = useRef(false);
  const owner = role === "owner";
  const moderator = role === "moderator" || role === "mod";
  const targetModerator = member.role === "moderator" || member.role === "mod";
  const canAct = !!viewer && viewer.toLowerCase() !== member.memberAddress.toLowerCase()
    && (owner && (targetModerator || member.role === "member") || moderator && member.role === "member");
  if (!canAct) return null;
  const choose = (next: Action) => { setError(""); setAction(next); };
  async function confirm() {
    if (pending.current || !action) return;
    pending.current = true; setBusy(true); setError("");
    try {
      await write(action === "remove" ? "DELETE" : "PATCH", member.memberAddress, action === "remove" ? undefined : { role: action });
      setAction(null);
    } catch (error) {
      setError(error instanceof Error && "status" in error && (error.status === 401 || error.status === 403)
        ? "You don't have permission to change this member."
        : error instanceof Error ? error.message : "The member could not be updated. Try again.");
    } finally { pending.current = false; setBusy(false); }
  }
  const title = action === "remove" ? "Remove member?" : action === "moderator" ? "Make moderator?" : "Remove moderator role?";
  return <>
    <CompactNavigation label="Manage" name="Member actions" items={[
      ...(owner ? [{ kind: "button" as const, label: targetModerator ? "Make member" : "Make moderator", disabled: busy,
        onClick: () => choose(targetModerator ? "member" : "moderator") }] : []),
      { kind: "button", label: "Remove member", danger: true, disabled: busy, onClick: () => choose("remove") },
    ]} />
    {action && <ConfirmDialog title={title} danger={action === "remove"}
      body={error ? <span role="alert">{error}</span> : `${member.name || member.memberAddress}: ${action === "remove" ? "membership and access to private community content will be removed." : action === "moderator" ? "allow moderation of members, join requests and announcements." : "keep membership and remove moderation permissions."}`}
      confirmLabel={busy ? "Saving…" : action === "remove" ? "Remove member" : "Save role"} cancelLabel="Cancel"
      onConfirm={() => void confirm()} onCancel={() => { if (!pending.current) setAction(null); }} />}
  </>;
}
