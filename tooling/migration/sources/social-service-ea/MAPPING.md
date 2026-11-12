# social-service-ea → `social`

Upstream: `github.com/decentraland/social-service-ea` at the `revision` in
`source.toml`; table shapes are the result of every file in `src/migrations`.
All wallet columns are lowercased. Rows whose wallet is not `0x` + 40 hex, self-pairs
and self-referrals are skipped. When case variants of one key exist, the lowercase spelling
wins. Upstream `timestamp` columns hold UTC; referral timestamps are epoch milliseconds.

| Step | Upstream | Target | Cursor |
| --- | --- | --- | --- |
| user_settings | social_settings | social.user_settings | address (full re-scan) |
| friendships | friendships + latest friendship_actions row | social.friendships | updated_at |
| friendship_actions | friendship_actions | social.friendship_actions | timestamp |
| blocks | blocks (id dropped) | social.blocks | blocked_at |
| user_mutes | user_mutes | social.user_mutes | muted_at |
| communities | communities | social.communities (+ deletes social.community_mirror rows) | updated_at |
| community_members | community_members (+ owner from communities) | social.community_members | key (full re-scan) |
| community_bans | community_bans | social.community_bans (+ deletes banned member rows) | key (full re-scan) |
| community_requests | community_requests | social.community_requests | updated_at |
| community_places | community_places (id → place_id) | social.community_places | added_at |
| community_posts | community_posts | social.community_posts | created_at |
| community_post_likes | community_post_likes | social.community_post_likes | liked_at |
| referral_progress | referral_progress | social.referral_progress | updated_at |
| referral_progress_viewed | referral_progress_viewed | social.referral_progress_viewed | updated_at |
| referral_reward_images | referral_reward_images | social.referral_reward_images | created_at |
| referral_emails | referral_emails | social.referral_emails | updated_at |

Catch-up runs use `--since last`: member roles and ban lifts move no upstream timestamp, so
those two steps only see changes on a full walk.

## Friendships

The node stores the latest action as a projection on the friendship (`last_action`,
`last_actor`, `last_action_at`, and `is_active = last_action = 'accept'`) and checks every
new action against it. The extract derives it from upstream's log (latest by timestamp,
then id), not from upstream's `is_active`, so the imported state and the imported history
agree. A friendship without actions becomes `accept` by the requested wallet when active,
`delete` otherwise. Upstream's pair key is ordered: of `A→B` and `B→A`, the most recently
updated row and its history are kept. A pair the node already holds under another id stays
the node's, and its upstream history is skipped. A re-run never rolls back a newer local
action.

Upstream skipped its friendship state-machine check for a while (fixed in
social-service-ea #427), so `accept` rows can exist with no prior `request`
from the other wallet. Such a friendship is neutralized: its latest action is
an `accept` whose previous action is not the other wallet's `request`, so the
import keeps the forged accept in the history and appends a `delete` by the
forger one millisecond later (id `md5('dcl-migration:forged-accept:' ||
friendship id)`, NULL metadata). The friendship projects to that delete
(inactive), so the node's state machine and its private-message and voice
gates see two non-friends, who can only become friends again through a real
request and accept. To list them on the source:

```sql
SELECT f.id FROM friendships f
JOIN LATERAL (SELECT * FROM friendship_actions x WHERE x.friendship_id = f.id
              ORDER BY x."timestamp" DESC NULLS LAST, x.id DESC LIMIT 1) a ON true
LEFT JOIN LATERAL (SELECT * FROM friendship_actions x WHERE x.friendship_id = f.id
                   AND (x."timestamp", x.id) < (a."timestamp", a.id)
                   ORDER BY x."timestamp" DESC, x.id DESC LIMIT 1) p ON true
WHERE a.action = 'accept'
  AND (p.action IS DISTINCT FROM 'request' OR lower(p.acting_user) = lower(a.acting_user));
```

## Communities: local authority and the mirror

This database is the upstream that `COMMUNITIES_MIRROR_UPSTREAM` copies from, so imported
communities become local authority: `origin = 'local'`, with owner and moderator rows. A
community the mirror already holds is taken over. Its origin flips to `local` and its
`social.community_mirror` row is deleted, or the listing would add upstream's member count on
top of the imported members. Members who joined on this node while it was mirrored keep
their rows, unless upstream actively bans them. From then on the mirror reports such ids as
local conflicts and leaves them alone. After the cutover the mirror should not point at this
upstream. Private and unlisted communities, which the mirror never sees, are imported too.

- The owner row follows `communities.owner_address`. It is synthesized when upstream has no
  member row for the owner. Any other `owner` row becomes `moderator`, so the node's
  one-owner index holds. `none` rows and actively banned wallets are not members.
- Bans: a re-ban keeps upstream's old `unbanned_by/at` and older lifted rows have none. Both
  are normalized to the node's rule that the lift fields are set exactly when the ban is
  lifted.
- Requests: only the newest pending request per (community, wallet) is kept. A pending
  request the node already holds wins.
- Kept on the node: `suspended*` and `ranking_score`. Upstream's ranking job output
  (`ranking_score`, `last_score_calculated_at`, `community_ranking_metrics`) has no writer
  here and is not copied. A local edit newer than upstream's `updated_at` is kept.
- Imported communities have no `social.community_signed_actions` entries, so they appear
  in the federation feeds only from their next signed action.

## Not migrated

- `private_voice_chats`: live call state that expires within minutes. The LiveKit rooms do
  not move.
- Community thumbnails: S3 objects, not rows in this database. `social.community_thumbnails`
  is filled when the blobs are moved, which is out of scope here.
- Nothing in `chat.*` or `social.friend_messages`: this database holds no chat.

## Shared targets

- `social.user_settings.private_messages_privacy` is also fed by comms-gatekeeper. This
  source upserts all three settings columns, so the source that runs last wins.
- `social.referral_progress*` / `referral_emails` may also be fed by referral-server's older
  database. `referral_progress` keeps the first referrer bound to a wallet, whichever source
  wrote it.

## Referral state

Progress includes the recorded IP (needed for signup limits), signup and tier timestamps,
and the granted flag used by accepted-invite counts. Status and the granted flag stay
consistent; completed progress does not regress on repeat imports. The first referrer wins,
and existing IP and transition timestamps are retained. The older referral-server only fills
gaps for the same referrer. Reward images retain their IDs and historical snapshots.
No reward requests, retry state or deliveries are created; existing target reward execution
fields are untouched. Redis login-day counters are not in this PostgreSQL database and
require a separate cutover decision before referral logins are enabled.

After upgrading an earlier import, restart `referral_progress` to populate fields whose
source watermark has not changed. The reward-image step has its own checkpoint.
