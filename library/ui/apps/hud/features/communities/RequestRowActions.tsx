// The buttons on a pending request's, invite's or ban's row, shared by the community card's Members
// sections and the Communities page's Requests Received: ACCEPT and REJECT on a join request, CANCEL
// on an invite sent, UNBAN on a banned member. Disabled while the row's own answer runs.

import { Button } from '../../design'
import type { CommunityMemberSection } from '@explorer-core/engine/protocol'

export type RequestAnswer = 'accepted' | 'rejected' | 'cancelled'

export function RequestRowActions({
  section,
  busy,
  onAnswer,
  onUnban
}: {
  section: CommunityMemberSection
  busy: boolean
  onAnswer: (intention: RequestAnswer) => void
  onUnban?: () => void
}): React.JSX.Element {
  if (section === 'requests') {
    return (
      <>
        <Button size="sm" disabled={busy} onClick={() => onAnswer('accepted')}>ACCEPT</Button>
        <Button size="sm" variant="secondary" disabled={busy} onClick={() => onAnswer('rejected')}>REJECT</Button>
      </>
    )
  }
  if (section === 'invites') {
    return <Button size="sm" variant="secondary" disabled={busy} onClick={() => onAnswer('cancelled')}>CANCEL</Button>
  }
  return <Button size="sm" variant="secondary" disabled={busy} onClick={onUnban}>UNBAN</Button>
}
