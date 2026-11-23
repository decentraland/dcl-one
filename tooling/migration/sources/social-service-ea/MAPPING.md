# social-service-ea → `social`

Upstream: `github.com/decentraland/social-service-ea` at the `revision` in
`source.toml`; table shapes are the result of every file in `src/migrations`.
All wallet columns are lowercased. Rows whose wallet is not `0x` + 40 hex, self-pairs
and self-referrals are skipped. When case variants of one key exist, the lowercase spelling
wins. Place ids are lowercased too, as the node stores them; of case variants in one
community the lowercase spelling wins, then the earliest added. Upstream `timestamp`
columns hold UTC; referral timestamps are epoch milliseconds.

| Step | Upstream | Target | Cursor |
| --- | --- | --- | --- |
| user_settings | social_settings | social.user_settings | address (full re-scan) |
| friendships | friendships + latest friendship_actions row | social.friendships | updated_at |
| friendship_actions | friendship_actions | social.friendship_actions | timestamp |
| blocks | blocks (id dropped) | social.blocks | blocked_at |
| blocks_reconcile | blocks (pairs) | deletes from social.blocks | whole set |
| user_mutes | user_mutes | social.user_mutes | muted_at |
| user_mutes_reconcile | user_mutes (pairs) | deletes from social.user_mutes | whole set |
| communities | communities | social.communities (+ deletes social.community_mirror rows) | updated_at |
| community_ranking_metrics | community_ranking_metrics + communities.ranking_score | social.communities ranking columns | key (full re-scan) |
| community_bans | community_bans | social.community_bans (+ deletes banned member rows) | key (full re-scan) |
| community_members | community_members (+ owner from communities) | social.community_members (+ deletes removed members) | whole set |
| community_requests | community_requests | social.community_requests | updated_at |
| community_places | community_places (id → place_id, lowercased) | social.community_places | added_at |
| community_posts | community_posts | social.community_posts | created_at |
| community_posts_reconcile | community_posts (ids) | deletes from social.community_posts | whole set |
| community_post_likes | community_post_likes | social.community_post_likes | liked_at |
| referral_progress | referral_progress | social.referral_progress | updated_at |
| referral_progress_viewed | referral_progress_viewed | social.referral_progress_viewed | updated_at |
| referral_reward_images | referral_reward_images | social.referral_reward_images | created_at |
| referral_emails | referral_emails | social.referral_emails | updated_at |

Member roles and ban lifts move no upstream timestamp, so those two steps re-read the whole
set on every run.

## Changes made on the node

Every step may run again after the node starts taking writes, so a re-run never undoes them.
For user settings, blocks, mutes, community members, bans and posts, what this source last
wrote per key is recorded in `dcl_migration.social_*_loaded`. A row whose node state no
longer matches the record was changed on the node (a settings change or reset, an unban or
re-ban, a role change, an edited post) and is left alone. A recorded row the node removed (a
kick, an unblock, a deleted post) is not re-added, unless upstream's row is newer (a later
join, block or mute). Blocks and mutes keep the newer timestamp. Rows imported before these
records existed are recorded on the next run. A ban record includes `banned_at`, which a node
re-ban refreshes; a ban with no record is the node's unless it already equals upstream's. A
wallet the node actively bans is never added as a member. `community_bans` runs first, so a ban
upstream lifted in the same run no longer blocks the rejoined member. An imported owner the load
demotes after an upstream transfer is removed when upstream no longer lists it.

Upstream deletes rows on unblock, unmute, kick, leave and post deletion. `community_members`
and the `*_reconcile` steps read the whole upstream set and delete the imported rows it no
longer holds, unless the node changed them since. A run whose upstream set is empty while
imported rows are recorded stops instead of deleting them all, unless `--allow-empty` names the step.

## Friendships

The node stores the latest action as a projection on the friendship (`last_action`,
`last_actor`, `last_action_at`, and `is_active = last_action = 'accept'`) and checks every
new action against it. The extract derives it from upstream's log (latest by timestamp,
then id), not from upstream's `is_active`, so the imported state and the imported history
agree. A friendship without actions becomes `accept` by the requested wallet when active,
`delete` otherwise. Upstream's pair key is ordered and case-sensitive: of `A→B`, `B→A` and their case variants,
the most recently updated row and its history are kept. A pair the node already holds under another id stays
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

- The owner row follows the node's `communities.owner_address` (a newer node transfer
  wins, see above). It is synthesized when upstream has no member row for the owner. Any other `owner` row becomes `moderator`, so the node's
  one-owner index holds. `none` rows and actively banned wallets are not members.
- Bans: a re-ban keeps upstream's old `unbanned_by/at` and older lifted rows have none. Both
  are normalized to the node's rule that the lift fields are set exactly when the ban is
  lifted. Members are removed for a ban only while it is in force on the node.
- Requests: only the newest pending request per (community, wallet) is kept. Status changes
  are applied before new rows, so a batch that resolves one request and carries its pending
  replacement keeps the replacement. A pending request the node already holds wins, a status
  the node made terminal is never reopened, and no pending request is added for a wallet the
  node bans there.
- Kept on the node: `suspended*`. A local edit newer than upstream's `updated_at` is kept.
  Ranking is the next section.
- Imported communities have no `social.community_signed_actions` entries, so they appear
  in the federation feeds only from their next signed action.

## Community ranking

The node's daily ranking pass scores a local community from five lifetime counters on
`social.communities` (`ranking_events_count`, `ranking_events_total_attendees`,
`ranking_photos_count`, `ranking_streams_count`, `ranking_streams_total_participants`) plus
facts it reads live: places, posts, members of the last 7 days, description, age and
`has_thumbnail`. The counters cannot be rebuilt from the node's data (no stream history; an
event rebuild would reach before upstream started counting), so `community_ranking_metrics`
copies upstream's `community_ranking_metrics` (zeros when a community has no row) and its
`ranking_score`, which stands until the node's next pass. `has_thumbnail` is not copied: the
node reads it from `social.community_thumbnails`, which `blobs social-service-ea` fills.

What the step last loaded is recorded in `dcl_migration.social_community_ranking_loaded`. A
re-run whose upstream counters are unchanged leaves the node's counters and score alone, so
after the cutover, once upstream's event, photo and streaming handlers are stopped, the node
keeps counting; upstream's daily rescore alone changes nothing. The score is copied with the
counters. When upstream's counters changed, events and attendees take upstream's
values and `ranking_tracking_since` moves to the moment upstream was read, so the node's
ended-event scan does not count again an imported event upstream counted; events that ended
on the node only before that moment are dropped. Photos and streams are upstream's plus the
node's own count, what it holds above upstream's last loaded value (imported photos are published without a ranking
increment, so the node never counts one of them). A taken-over mirrored community starts its tracking
there too.

Order: `run` (communities before community_ranking_metrics), then `blobs social-service-ea`,
then start or restart the node, whose ranking job scores on acquiring its lease and daily.

## Community places

A place id resolves to a full row through the node's own catalogue (`explore.place`),
whatever `PLACES_MIRROR_UPSTREAM` says. A Genesis place is keyed there by upstream's places
UUID only after `run places`: with the mirror off and no places import, the catalogue holds
only the node's derived rows, keyed by the UUIDv5 of the base parcel, and an imported
community place comes back as its bare `{id, addedBy, addedAt}` row. The rows resolve as
soon as places is imported, in any order. The load reports the UUID ids the catalogue
lacks under their exact stored spelling (the node's lookup), except world-scene ids. Some
ids stay bare whatever is imported, as upstream serves them: a world scene's UUID (only its
world name is a destination) and a place upstream disabled (redeployed elsewhere,
undeployed), which the places import skips, so it is reported too.

## Community thumbnails

Upstream keeps a community's picture on its CDN bucket, not in this database, at
`social/communities/<id>/raw-thumbnail.png`; the import moves the rows, and the takeover
deletes the mirror row that pointed at upstream's copy. `migrate.py blobs social-service-ea`
copies them after `run`: for each community this source holds that the node has as `local` with
no `social.community_thumbnails` row, it reads that object (by default from
`https://assets-cdn.decentraland.org/`), checks it is a PNG, JPEG, GIF or WebP image, stores it
under the content hash the node's upload uses (`hash.rs::hash_bytes`) and inserts the row right
after, in batches, so the storage GC does not see the blob unreferenced. A community upstream has
no picture for (a 403 or 404) gets no row, as upstream serves it none; a row the node holds wins.

## Not migrated

- `private_voice_chats`: live call state that expires within minutes. The LiveKit rooms do
  not move.
- Nothing in `chat.*` or `social.friend_messages`: this database holds no chat.

## Shared targets

- `social.user_settings`: settings changed on the node after this source wrote them are kept.
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
