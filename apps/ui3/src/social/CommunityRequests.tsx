import { useState } from "react";
import ContentStatus from "../components/ContentStatus";
import { truncateAddress } from "../data/format";

export type CommunityRequest = {
  id: string; communityId: string; memberAddress: string;
  type: "invite" | "request_to_join";
  status: "pending" | "accepted" | "rejected" | "cancelled";
  name?: string;
};
export type RequestDecision = "accepted" | "rejected" | "cancelled";

type Props = {
  requests: CommunityRequest[];
  managing?: boolean;
  loading: boolean;
  error?: string;
  busy: boolean;
  onRetry: () => void;
  onDecide: (request: CommunityRequest, decision: RequestDecision) => void;
  onOpen: (id: string) => void;
  onInvite?: (address: string) => void;
};

export default function CommunityRequests({ requests, managing, loading, error, busy, onRetry, onDecide, onOpen, onInvite }: Props) {
  const [address, setAddress] = useState("");
  const pending = requests.filter(row => row.status === "pending");
  return <div className="soc__page soc-requests">
    <h1>{managing ? "Membership requests" : "Invitations & requests"}</h1>
    {error && <div role="alert">{error} <button className="soc__outline" onClick={onRetry}>Retry</button></div>}
    {onInvite && <form onSubmit={event => { event.preventDefault(); if (!busy && /^0x[0-9a-fA-F]{40}$/.test(address.trim())) onInvite(address.trim()); }}>
      <label>Member wallet address<input className="soc__search" value={address} onChange={event => setAddress(event.target.value)} placeholder="0x…" required pattern="0x[0-9a-fA-F]{40}" /></label>
      <button className="soc__join" disabled={busy || !/^0x[0-9a-fA-F]{40}$/.test(address.trim())}>Send invitation</button>
    </form>}
    {loading ? <ContentStatus pending message="Loading membership requests…" /> : <>
      {!pending.length && !error && <p className="soc__muted">No pending invitations or requests.</p>}
      <ul className="soc__list">
        {pending.map(row => <li className="soc-request" key={row.id} aria-label={`${managing ? row.memberAddress : row.name || row.communityId}: ${row.type === "invite" ? "invitation" : "join request"}`}>
          <div className="soc__rowtext"><strong>{row.name || truncateAddress(row.memberAddress)}</strong>
            <small title={row.memberAddress}>{managing ? row.memberAddress : row.type === "invite" ? "You are invited to join" : "Waiting for approval"}</small>
          </div>
          <div className="soc-request__actions">
            {!managing && <button className="soc__outline" onClick={() => onOpen(row.communityId)}>Open community</button>}
            {(managing ? row.type === "request_to_join" : row.type === "invite") ? <>
              <button className="soc__join" disabled={busy} onClick={() => onDecide(row, "accepted")}>{managing ? "Approve" : "Accept invitation"}</button>
              <button className="soc__outline" disabled={busy} onClick={() => onDecide(row, "rejected")}>{managing ? "Reject" : "Decline invitation"}</button>
            </> : <button className="soc__outline" disabled={busy} onClick={() => onDecide(row, "cancelled")}>{managing ? "Cancel invitation" : "Cancel request"}</button>}
          </div>
        </li>)}
      </ul>
    </>}
  </div>;
}
