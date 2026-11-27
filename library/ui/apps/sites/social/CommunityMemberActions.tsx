import { useRef, useState } from 'react'

import { KebabMenu } from '../../../index'

import { HubConfirm } from './SocialHub'

export type MemberChange = 'moderator' | 'member' | 'remove'

const TITLES: Record<MemberChange, string> = {
  moderator: 'Make moderator?',
  member: 'Remove moderator role?',
  remove: 'Remove member?'
}
const EFFECTS: Record<MemberChange, string> = {
  moderator: 'allow moderation of members, join requests and announcements.',
  member: 'keep membership and remove moderation permissions.',
  remove: 'membership and access to private community content will be removed.'
}

/**
 * What a community's owner or moderator can do to one member: change their role (owners only) or
 * remove them, each after a confirmation. A refused change keeps the dialog open and says why.
 */
export function CommunityMemberActions({
  name,
  moderator,
  canChangeRole,
  onChange
}: {
  /** The member's name or address, for the confirmation. */
  name: string
  /** The member is a moderator now. */
  moderator: boolean
  /** The reader owns the community, so they also assign roles. */
  canChangeRole: boolean
  onChange: (change: MemberChange) => Promise<void>
}) {
  const [change, setChange] = useState<MemberChange | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const sending = useRef(false)
  const choose = (next: MemberChange) => {
    setError('')
    setChange(next)
  }
  async function confirm() {
    if (sending.current || !change) return
    sending.current = true
    setBusy(true)
    setError('')
    try {
      await onChange(change)
      setChange(null)
    } catch (failure) {
      setError(
        failure instanceof Error && failure.message
          ? failure.message
          : 'The member could not be updated. Try again.'
      )
    } finally {
      sending.current = false
      setBusy(false)
    }
  }
  return (
    <>
      <KebabMenu
        label="Member actions"
        size="sm"
        items={(close) => [
          ...(canChangeRole
            ? [
                {
                  label: moderator ? 'Make member' : 'Make moderator',
                  onClick: () => {
                    close()
                    choose(moderator ? 'member' : 'moderator')
                  }
                }
              ]
            : []),
          {
            label: 'Remove member',
            danger: true,
            onClick: () => {
              close()
              choose('remove')
            }
          }
        ]}
      />
      {change ? (
        <HubConfirm
          title={TITLES[change]}
          body={`${name}: ${EFFECTS[change]}`}
          error={error || undefined}
          danger={change === 'remove'}
          busy={busy}
          confirmLabel={busy ? 'Saving…' : change === 'remove' ? 'Remove member' : 'Save role'}
          onConfirm={() => void confirm()}
          onCancel={() => {
            if (!sending.current) setChange(null)
          }}
        />
      ) : null}
    </>
  )
}
