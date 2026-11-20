import AccessRestored from '../assets/notifications/AccessRestored.svg'
import AccessRestricted from '../assets/notifications/AccessRestricted.svg'
import BidAccepted from '../assets/notifications/BidAccepted.svg'
import Coauthor from '../assets/notifications/Coauthor.svg'
import Credits from '../assets/notifications/Credits.svg'
import CreditsCompleteYourGoalsReminder from '../assets/notifications/CreditsCompleteYourGoalsReminder.svg'
import CreditsExpireSoonReminder from '../assets/notifications/CreditsExpireSoonReminder.svg'
import EventApproved from '../assets/notifications/EventApproved.svg'
import EventDeleted from '../assets/notifications/EventDeleted.svg'
import EventRejected from '../assets/notifications/EventRejected.svg'
import EventStarted from '../assets/notifications/EventStarted.svg'
import EventStartsSoon from '../assets/notifications/EventStartsSoon.svg'
import Gift from '../assets/notifications/Gift.svg'
import Governance from '../assets/notifications/Governance.svg'
import ItemSold from '../assets/notifications/ItemSold.svg'
import LandRented from '../assets/notifications/LandRented.svg'
import ManaMainnet from '../assets/notifications/ManaMainnet.svg'
import ManaPolygon from '../assets/notifications/ManaPolygon.svg'
import MissingResources from '../assets/notifications/MissingResources.svg'
import NewComment from '../assets/notifications/NewComment.svg'
import PermissionGranted from '../assets/notifications/PermissionGranted.svg'
import PermissionRevoked from '../assets/notifications/PermissionRevoked.svg'
import Pitch from '../assets/notifications/Pitch.svg'
import ProjectEnacted from '../assets/notifications/ProjectEnacted.svg'
import ProposalFinished from '../assets/notifications/ProposalFinished.svg'
import ReferralInviteFriends from '../assets/notifications/ReferralInviteFriends.svg'
import ReferralInvitedUserAccepted from '../assets/notifications/ReferralInvitedUserAccepted.svg'
import RentPeriodEnding from '../assets/notifications/RentPeriodEnding.svg'
import Reward from '../assets/notifications/Reward.svg'
import Streaming from '../assets/notifications/Streaming.svg'
import Tender from '../assets/notifications/Tender.svg'
import Tip from '../assets/notifications/Tip.svg'
import VotingEnded from '../assets/notifications/VotingEnded.svg'

export const notificationArtwork = {
  bid_accepted: BidAccepted,
  bid_received: BidAccepted,
  credits_on_demand_granted: Credits,
  credits_reminder_claim_credits: Credits,
  credits_reminder_complete_goals: CreditsCompleteYourGoalsReminder,
  credits_reminder_do_not_miss_out: Credits,
  credits_reminder_usage: CreditsExpireSoonReminder,
  credits_reminder_usage_24_hours: CreditsExpireSoonReminder,
  event_approved: EventApproved,
  event_deleted: EventDeleted,
  event_rejected: EventRejected,
  events_started: EventStarted,
  events_starts_soon: EventStartsSoon,
  governance_announcement: Governance,
  governance_authored_proposal_finished: ProposalFinished,
  governance_coauthor_requested: Coauthor,
  governance_new_comment_on_project_update: NewComment,
  governance_new_comment_on_proposal: NewComment,
  governance_pitch_passed: Pitch,
  governance_proposal_enacted: ProjectEnacted,
  governance_tender_passed: Tender,
  governance_voting_ended_voter: VotingEnded,
  item_sold: ItemSold,
  rental_ended: RentPeriodEnding,
  rental_started: LandRented,
  referral_invited_users_accepted: ReferralInvitedUserAccepted,
  referral_new_tier_reached: Reward,
  reward_assignment: Reward,
  reward_campaign_gas_price_higher_than_expected: Reward,
  reward_campaign_out_of_funds: Reward,
  reward_campaign_out_of_stock: Reward,
  reward_delayed: Reward,
  reward_in_progress: Reward,
  royalties_earned: ManaPolygon,
  social_service_friendship_accepted: ReferralInviteFriends,
  social_service_friendship_request: ReferralInviteFriends,
  streaming_key_expired: Streaming,
  streaming_key_reset: Streaming,
  streaming_key_revoke: Streaming,
  streaming_place_updated: Streaming,
  streaming_time_exceeded: Streaming,
  tip_received: Tip,
  transfer_received: Gift,
  worlds_access_restored: AccessRestored,
  worlds_access_restricted: AccessRestricted,
  worlds_missing_resources: MissingResources,
  worlds_permission_granted: PermissionGranted,
  worlds_permission_revoked: PermissionRevoked
} as const

const itemImages = new Set([
  'bid_accepted',
  'bid_received',
  'item_sold',
  'royalties_earned',
  'referral_new_tier_reached',
  'reward_assignment',
  'reward_delayed',
  'reward_in_progress'
])

export function notificationMedia(
  type: string,
  metadata: Record<string, unknown>,
  image: string | null
) {
  let artwork: string | undefined = Object.hasOwn(notificationArtwork, type)
    ? notificationArtwork[type as keyof typeof notificationArtwork]
    : undefined
  if (type === 'royalties_earned' && metadata.network === 'ethereum') artwork = ManaMainnet
  if (Array.isArray(metadata.permissions) && metadata.permissions[0] === 'access') {
    if (type === 'worlds_permission_granted') artwork = AccessRestored
    if (type === 'worlds_permission_revoked') artwork = AccessRestricted
  }
  const thumbnail = image && /^https:\/\//i.test(image) ? image : undefined
  if (artwork && thumbnail && itemImages.has(type)) return { image: thumbnail, badge: artwork }
  return { image: artwork ?? thumbnail, standalone: Boolean(artwork) }
}
