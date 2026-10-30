# Capture manifest: social family

Written by `tests/conformance/capture/social.mjs` (this file is rendered from
`manifest.json`, which lists every run). Each run is one sequential pass with at
most one request per 550 ms per host, reads only. A run's throwaway signing
key is generated in memory and discarded at exit; nothing was ever written under it.
Redactions are applied to fixture and raw bodies alike (JWTs, e-mail addresses).

## Runs

| started (UTC) | ended (UTC) | arguments | throwaway signer | requests | redactions |
| --- | --- | --- | --- | --- | --- |
| 2026-09-25T19:23:58.813Z | 2026-09-25T19:25:10.228Z | `(all)` | 0xe45c088898a7381f5ea1bfd8cdc8144a952c7d6f | 131 | 0 jwt, 4 email |
| 2026-09-25T19:29:32.836Z | 2026-09-25T19:29:43.044Z | `--steps member-invites-signed-other,get-scene-adapter-signed,scene-participants,scene-participants-default-realm,scene-participants-world,scene-admin-signed-scene,scene-bans-signed-scene,scene-bans-addresses-signed-scene,notifications-bad-signature,face-profile-0-object,body-profile-0-object,face-profile-1-object,body-profile-1-object,face-profile-2-object,body-profile-2-object,face-unknown-cid-object,image-redirect,image-file-redirect,image-metadata,wearable-images` | 0xf7e07d862ddbe561a63ebb5dd809cf0542269587 | 20 | 1 jwt, 0 email |
| 2026-09-25T19:30:59.620Z | 2026-09-25T19:31:01.316Z | `--steps scene-bans-signed-scene,wearable-images` | 0xd6690b5a1525b60a799fc41a63f59e01ed620cb1 | 3 | 0 jwt, 0 email |
| 2026-10-26T17:36:50.433Z | 2026-10-26T17:36:54.029Z | `--only social,social-rpc --steps communities-list-origin,status,health-live,privacy-settings-unknown,privacy-settings-bad-address,unknown-path-404` | (none) | 7 | 0 jwt, 0 email |
| 2026-10-26T18:01:46.783Z | 2026-10-26T18:01:47.560Z | `--only social --steps mutes-post-unauth,mutes-delete-unauth` | (none) | 2 | 0 jwt, 0 email |
| 2026-10-26T20:46:17.163Z | 2026-10-26T20:46:18.928Z | `--only social --steps health-live,health-ready,health-startup` | (none) | 3 | 0 jwt, 0 email |

## Requests per host (all runs)

| host | requests |
| --- | --- |
| social-api.decentraland.org | 54 |
| peer.decentraland.org | 4 |
| comms-gatekeeper.decentraland.org | 39 |
| notifications.decentraland.org | 14 |
| badges.decentraland.org | 10 |
| assets-cdn.decentraland.org | 3 |
| profile-images.decentraland.org | 13 |
| camera-reel-service.decentraland.org | 21 |
| profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com | 7 |
| rpc-social-service-ea.decentraland.org | 5 |

## Helper lookups (not fixtures)

| run | label | method | URL | status | UTC |
| --- | --- | --- | --- | --- | --- |
| 1 | active profile entities | POST | https://peer.decentraland.org/content/entities/active | 200 | 2026-09-25T19:24:23Z |
| 1 | Genesis Plaza scene entity | POST | https://peer.decentraland.org/content/entities/active | 200 | 2026-09-25T19:24:24Z |
| 2 | active profile entities | POST | https://peer.decentraland.org/content/entities/active | 200 | 2026-09-25T19:29:33Z |
| 2 | Genesis Plaza scene entity | POST | https://peer.decentraland.org/content/entities/active | 200 | 2026-09-25T19:29:33Z |

## Requests

| run | service | fixture | method | URL | status | bytes | UTC | signed path |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | social | communities-list | GET | https://social-api.decentraland.org/v1/communities?limit=10&offset=0 | 200 | 6341 | 2026-09-25T19:23:58Z |  |
| 1 | social | communities-list-page2 | GET | https://social-api.decentraland.org/v1/communities?limit=5&offset=5 | 200 | 3276 | 2026-09-25T19:23:59Z |  |
| 1 | social | communities-list-search | GET | https://social-api.decentraland.org/v1/communities?search=dec&limit=5&offset=0 | 200 | 3197 | 2026-09-25T19:23:59Z |  |
| 1 | social | communities-list-only-voice-chat | GET | https://social-api.decentraland.org/v1/communities?onlyWithActiveVoiceChat=true&limit=5 | 200 | 62 | 2026-09-25T19:24:00Z |  |
| 1 | social | communities-list-limit-zero | GET | https://social-api.decentraland.org/v1/communities?limit=0 | 200 | 50838 | 2026-09-25T19:24:01Z |  |
| 1 | social | communities-list-limit-not-a-number | GET | https://social-api.decentraland.org/v1/communities?limit=abc | 200 | 50838 | 2026-09-25T19:24:01Z |  |
| 1 | social | communities-list-401-minimal-unauth | GET | https://social-api.decentraland.org/v1/communities?minimal=true&search=dec | 401 | 91 | 2026-09-25T19:24:02Z |  |
| 1 | social | communities-list-minimal-signed | GET | https://social-api.decentraland.org/v1/communities?minimal=true&search=dec&limit=5 | 200 | 632 | 2026-09-25T19:24:02Z | /v1/communities |
| 1 | social | community-by-id | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b | 200 | 447 | 2026-09-25T19:24:03Z |  |
| 1 | social | community-by-id-404-unknown | GET | https://social-api.decentraland.org/v1/communities/00000000-0000-4000-8000-000000000000 | 404 | 91 | 2026-09-25T19:24:03Z |  |
| 1 | social | community-by-id-bad-id | GET | https://social-api.decentraland.org/v1/communities/not-a-uuid | 400 | 54 | 2026-09-25T19:24:04Z |  |
| 1 | social | community-members | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/members?limit=10&offset=0 | 200 | 4140 | 2026-09-25T19:24:04Z |  |
| 1 | social | community-members-only-online | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/members?onlyOnline=true&limit=5 | 200 | 2164 | 2026-09-25T19:24:05Z |  |
| 1 | social | community-members-404-unknown | GET | https://social-api.decentraland.org/v1/communities/00000000-0000-4000-8000-000000000000/members | 404 | 91 | 2026-09-25T19:24:05Z |  |
| 1 | social | community-places | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/places?limit=10&offset=0 | 200 | 1381 | 2026-09-25T19:24:06Z |  |
| 1 | social | community-posts | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/posts?limit=5&offset=0 | 200 | 2696 | 2026-09-25T19:24:07Z |  |
| 1 | social | community-posts-v2 | GET | https://social-api.decentraland.org/v2/communities/e99471aa-31c4-4952-abf6-99905445f43b/posts?limit=5&offset=0 | 200 | 1721 | 2026-09-25T19:24:07Z |  |
| 1 | social | communities-v2-list-search | GET | https://social-api.decentraland.org/v2/communities?search=dec&limit=5&offset=0 | 200 | 3077 | 2026-09-25T19:24:08Z |  |
| 1 | social | communities-v2-list-no-search | GET | https://social-api.decentraland.org/v2/communities?limit=5&offset=0 | 200 | 3013 | 2026-09-25T19:24:08Z |  |
| 1 | social | community-v2-by-id | GET | https://social-api.decentraland.org/v2/communities/e99471aa-31c4-4952-abf6-99905445f43b | 200 | 421 | 2026-09-25T19:24:09Z |  |
| 1 | social | community-v2-by-id-404-unknown | GET | https://social-api.decentraland.org/v2/communities/00000000-0000-4000-8000-000000000000 | 404 | 91 | 2026-09-25T19:24:09Z |  |
| 1 | social | community-v2-members | GET | https://social-api.decentraland.org/v2/communities/e99471aa-31c4-4952-abf6-99905445f43b/members?limit=5&offset=0 | 200 | 1029 | 2026-09-25T19:24:10Z |  |
| 1 | social | member-communities | GET | https://social-api.decentraland.org/v1/members/0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda/communities?limit=5&offset=0 | 200 | 360 | 2026-09-25T19:24:10Z |  |
| 1 | social | member-communities-bad-address | GET | https://social-api.decentraland.org/v1/members/not-an-address/communities | 200 | 64 | 2026-09-25T19:24:11Z |  |
| 1 | social | mutes-400-unauth | GET | https://social-api.decentraland.org/v1/mutes?limit=100&offset=0 | 400 | 101 | 2026-09-25T19:24:12Z |  |
| 1 | social | mutes-signed | GET | https://social-api.decentraland.org/v1/mutes?limit=100&offset=0 | 200 | 64 | 2026-09-25T19:24:12Z | /v1/mutes |
| 1 | social | member-requests-unauth | GET | https://social-api.decentraland.org/v1/members/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/requests | 400 | 101 | 2026-09-25T19:24:13Z |  |
| 1 | social | member-requests-signed | GET | https://social-api.decentraland.org/v1/members/0xe45c088898a7381f5ea1bfd8cdc8144a952c7d6f/requests?limit=10&offset=0 | 200 | 63 | 2026-09-25T19:24:13Z | /v1/members/0xe45c088898a7381f5ea1bfd8cdc8144a952c7d6f/requests |
| 1 | social | member-requests-v2-unauth | GET | https://social-api.decentraland.org/v2/members/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/requests | 400 | 101 | 2026-09-25T19:24:14Z |  |
| 1 | social | member-invites-unauth | GET | https://social-api.decentraland.org/v1/members/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/invites | 400 | 101 | 2026-09-25T19:24:14Z |  |
| 1 | social | member-invites-signed-self | GET | https://social-api.decentraland.org/v1/members/0xe45c088898a7381f5ea1bfd8cdc8144a952c7d6f/invites | 400 | 66 | 2026-09-25T19:24:15Z | /v1/members/0xe45c088898a7381f5ea1bfd8cdc8144a952c7d6f/invites |
| 1 | social | community-bans-unauth | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/bans | 400 | 101 | 2026-09-25T19:24:15Z |  |
| 1 | social | community-bans-signed-not-moderator | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/bans | 401 | 155 | 2026-09-25T19:24:16Z | /v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/bans |
| 1 | social | community-bans-v2-unauth | GET | https://social-api.decentraland.org/v2/communities/e99471aa-31c4-4952-abf6-99905445f43b/bans | 400 | 101 | 2026-09-25T19:24:16Z |  |
| 1 | social | community-requests-unauth | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/requests | 400 | 101 | 2026-09-25T19:24:17Z |  |
| 1 | social | community-requests-signed-not-owner | GET | https://social-api.decentraland.org/v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/requests | 401 | 131 | 2026-09-25T19:24:18Z | /v1/communities/e99471aa-31c4-4952-abf6-99905445f43b/requests |
| 1 | social | community-requests-v2-unauth | GET | https://social-api.decentraland.org/v2/communities/e99471aa-31c4-4952-abf6-99905445f43b/requests | 400 | 101 | 2026-09-25T19:24:18Z |  |
| 1 | social | voice-chats-active-unauth | GET | https://social-api.decentraland.org/v1/community-voice-chats/active | 400 | 101 | 2026-09-25T19:24:19Z |  |
| 1 | social | voice-chats-active-signed | GET | https://social-api.decentraland.org/v1/community-voice-chats/active | 200 | 37 | 2026-09-25T19:24:19Z | /v1/community-voice-chats/active |
| 1 | social | referral-progress-unauth | GET | https://social-api.decentraland.org/v1/referral-progress | 400 | 101 | 2026-09-25T19:24:20Z |  |
| 1 | social | referral-progress-signed | GET | https://social-api.decentraland.org/v1/referral-progress | 200 | 75 | 2026-09-25T19:24:20Z | /v1/referral-progress |
| 1 | social | moderation-communities-unauth | GET | https://social-api.decentraland.org/v1/moderation/communities | 400 | 101 | 2026-09-25T19:24:21Z |  |
| 1 | social | moderation-communities-signed-not-moderator | GET | https://social-api.decentraland.org/v1/moderation/communities | 403 | 66 | 2026-09-25T19:24:21Z | /v1/moderation/communities |
| 1 | social | managed-communities-401-no-bearer | GET | https://social-api.decentraland.org/v1/communities/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/managed | 401 | 70 | 2026-09-25T19:24:22Z |  |
| 1 | social | preflight-communities | OPTIONS | https://social-api.decentraland.org/v1/communities | 204 | 0 | 2026-09-25T19:24:23Z |  |
| 1 | social | unknown-path-404 | GET | https://social-api.decentraland.org/v1/does-not-exist | 404 | 9 | 2026-09-25T19:24:23Z |  |
| 1 | gatekeeper | status | GET | https://comms-gatekeeper.decentraland.org/status | 200 | 104 | 2026-09-25T19:24:24Z |  |
| 1 | gatekeeper | status-head | HEAD | https://comms-gatekeeper.decentraland.org/status | 200 | 0 | 2026-09-25T19:24:25Z |  |
| 1 | gatekeeper | ping | GET | https://comms-gatekeeper.decentraland.org/ping | 200 | 5 | 2026-09-25T19:24:25Z |  |
| 1 | gatekeeper | user-bans | GET | https://comms-gatekeeper.decentraland.org/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/bans | 200 | 27 | 2026-09-25T19:24:26Z |  |
| 1 | gatekeeper | user-bans-unknown-address | GET | https://comms-gatekeeper.decentraland.org/users/0x0000000000000000000000000000000000000001/bans | 200 | 27 | 2026-09-25T19:24:26Z |  |
| 1 | gatekeeper | user-bans-bad-address | GET | https://comms-gatekeeper.decentraland.org/users/not-an-address/bans | 200 | 27 | 2026-09-25T19:24:27Z |  |
| 1 | gatekeeper | get-scene-adapter-unauth | POST | https://comms-gatekeeper.decentraland.org/get-scene-adapter | 401 | 55 | 2026-09-25T19:24:27Z |  |
| 1 | gatekeeper | get-scene-adapter-signed-no-realm-name | POST | https://comms-gatekeeper.decentraland.org/get-scene-adapter | 401 | 69 | 2026-09-25T19:24:28Z | /get-scene-adapter |
| 1 | gatekeeper | get-server-scene-adapter-unauth | POST | https://comms-gatekeeper.decentraland.org/get-server-scene-adapter | 401 | 55 | 2026-09-25T19:24:28Z |  |
| 1 | gatekeeper | scene-participants (wrong query names sceneId/realmName; 400 body kept as scene-participants-400-no-params, files removed) | GET | https://comms-gatekeeper.decentraland.org/scene-participants?sceneId=bafkreignao6tsikf77gkx5vgt2gpjmgoeu2rlydnourzlavtq474gikzoy&realmName=main | 400 | 57 | 2026-09-25T19:24:29Z |  |
| 1 | gatekeeper | scene-participants-400-no-params | GET | https://comms-gatekeeper.decentraland.org/scene-participants | 400 | 57 | 2026-09-25T19:24:30Z |  |
| 1 | gatekeeper | scene-admin-unauth | GET | https://comms-gatekeeper.decentraland.org/scene-admin | 400 | 43 | 2026-09-25T19:24:30Z |  |
| 1 | gatekeeper | scene-admin-signed-wrong-signer | GET | https://comms-gatekeeper.decentraland.org/scene-admin | 400 | 127 | 2026-09-25T19:24:31Z | /scene-admin |
| 1 | gatekeeper | scene-bans-unauth | GET | https://comms-gatekeeper.decentraland.org/scene-bans?limit=20&offset=0 | 400 | 43 | 2026-09-25T19:24:31Z |  |
| 1 | gatekeeper | scene-bans-signed-wrong-signer | GET | https://comms-gatekeeper.decentraland.org/scene-bans?limit=20&offset=0 | 400 | 127 | 2026-09-25T19:24:32Z | /scene-bans |
| 1 | gatekeeper | scene-bans-addresses-unauth | GET | https://comms-gatekeeper.decentraland.org/scene-bans/addresses | 400 | 43 | 2026-09-25T19:24:32Z |  |
| 1 | gatekeeper | scene-stream-access-unauth | GET | https://comms-gatekeeper.decentraland.org/scene-stream-access | 400 | 43 | 2026-09-25T19:24:33Z |  |
| 1 | gatekeeper | user-warnings-401-unauth | GET | https://comms-gatekeeper.decentraland.org/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/warnings | 401 | 58 | 2026-09-25T19:24:33Z |  |
| 1 | gatekeeper | user-warnings-signed-not-moderator | GET | https://comms-gatekeeper.decentraland.org/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/warnings | 401 | 58 | 2026-09-25T19:24:34Z | /users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/warnings |
| 1 | gatekeeper | bans-401-unauth | GET | https://comms-gatekeeper.decentraland.org/bans | 401 | 58 | 2026-09-25T19:24:34Z |  |
| 1 | gatekeeper | private-messages-token-unauth | GET | https://comms-gatekeeper.decentraland.org/private-messages/token | 400 | 43 | 2026-09-25T19:24:35Z |  |
| 1 | gatekeeper | voice-chat-status-401-no-token | GET | https://comms-gatekeeper.decentraland.org/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/voice-chat-status | 401 | 43 | 2026-09-25T19:24:36Z |  |
| 1 | gatekeeper | community-voice-chat-active-401-no-token | GET | https://comms-gatekeeper.decentraland.org/community-voice-chat/active | 401 | 43 | 2026-09-25T19:24:36Z |  |
| 1 | gatekeeper | community-voice-chat-status-401-no-token | GET | https://comms-gatekeeper.decentraland.org/community-voice-chat/e99471aa-31c4-4952-abf6-99905445f43b/status | 401 | 43 | 2026-09-25T19:24:37Z |  |
| 1 | gatekeeper | world-ban-status-no-token | GET | https://comms-gatekeeper.decentraland.org/worlds/genesis.dcl.eth/parcels/0,0/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/ban-status | 401 | 43 | 2026-09-25T19:24:37Z |  |
| 1 | gatekeeper | cast-stream-info-invalid-key | GET | https://comms-gatekeeper.decentraland.org/cast/stream-info/not-a-real-streaming-key | 400 | 33 | 2026-09-25T19:24:38Z |  |
| 1 | gatekeeper | cast-presenters-unauth | GET | https://comms-gatekeeper.decentraland.org/cast/presenters | 400 | 43 | 2026-09-25T19:24:38Z |  |
| 1 | gatekeeper | preflight-get-scene-adapter | OPTIONS | https://comms-gatekeeper.decentraland.org/get-scene-adapter | 204 | 0 | 2026-09-25T19:24:39Z |  |
| 1 | gatekeeper | unknown-path-404 | GET | https://comms-gatekeeper.decentraland.org/does-not-exist | 404 | 9 | 2026-09-25T19:24:39Z |  |
| 1 | notifications | status | GET | https://notifications.decentraland.org/status | 200 | 104 | 2026-09-25T19:24:40Z |  |
| 1 | notifications | notifications-unauth | GET | https://notifications.decentraland.org/notifications | 400 | 101 | 2026-09-25T19:24:40Z |  |
| 1 | notifications | notifications-signed | GET | https://notifications.decentraland.org/notifications | 200 | 20 | 2026-09-25T19:24:41Z | /notifications |
| 1 | notifications | notifications-signed-filters | GET | https://notifications.decentraland.org/notifications?limit=5&onlyUnread&from=0 | 200 | 20 | 2026-09-25T19:24:41Z | /notifications |
| 1 | notifications | notifications-expired-timestamp | GET | https://notifications.decentraland.org/notifications | 401 | 186 | 2026-09-25T19:24:42Z |  |
| 1 | notifications | subscription-unauth | GET | https://notifications.decentraland.org/subscription | 400 | 101 | 2026-09-25T19:24:42Z |  |
| 1 | notifications | subscription-signed | GET | https://notifications.decentraland.org/subscription | 200 | 4073 | 2026-09-25T19:24:43Z | /subscription |
| 1 | notifications | opt-outs-community-unauth | GET | https://notifications.decentraland.org/subscription/opt-outs/community/e99471aa-31c4-4952-abf6-99905445f43b | 400 | 101 | 2026-09-25T19:24:43Z |  |
| 1 | notifications | opt-outs-community-signed | GET | https://notifications.decentraland.org/subscription/opt-outs/community/e99471aa-31c4-4952-abf6-99905445f43b | 200 | 87 | 2026-09-25T19:24:44Z | /subscription/opt-outs/community/e99471aa-31c4-4952-abf6-99905445f43b |
| 1 | notifications | opt-outs-bad-scope-signed | GET | https://notifications.decentraland.org/subscription/opt-outs/planet/00000000-0000-4000-8000-000000000000 | 400 | 49 | 2026-09-25T19:24:45Z | /subscription/opt-outs/planet/00000000-0000-4000-8000-000000000000 |
| 1 | notifications | unsubscribe-address-no-signature | GET | https://notifications.decentraland.org/unsubscribe/0x8d6f63e382d73cf53858864f673f39e9ff915a1e | 401 | 51 | 2026-09-25T19:24:45Z |  |
| 1 | notifications | preflight-notifications | OPTIONS | https://notifications.decentraland.org/notifications | 204 | 0 | 2026-09-25T19:24:46Z |  |
| 1 | notifications | unknown-path-404 | GET | https://notifications.decentraland.org/does-not-exist | 404 | 9 | 2026-09-25T19:24:46Z |  |
| 1 | badges | categories | GET | https://badges.decentraland.org/categories | 200 | 81 | 2026-09-25T19:24:46Z |  |
| 1 | badges | user-badges | GET | https://badges.decentraland.org/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/badges | 200 | 2430 | 2026-09-25T19:24:47Z |  |
| 1 | badges | user-badges-second-address | GET | https://badges.decentraland.org/users/0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda/badges | 200 | 4556 | 2026-09-25T19:24:47Z |  |
| 1 | badges | user-badges-unknown-address | GET | https://badges.decentraland.org/users/0x0000000000000000000000000000000000000001/badges | 200 | 41 | 2026-09-25T19:24:48Z |  |
| 1 | badges | user-badges-bad-address | GET | https://badges.decentraland.org/users/not-an-address/badges | 200 | 41 | 2026-09-25T19:24:49Z |  |
| 1 | badges | user-preview | GET | https://badges.decentraland.org/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/preview | 200 | 404 | 2026-09-25T19:24:49Z |  |
| 1 | badges | user-preview-unknown-address | GET | https://badges.decentraland.org/users/0x0000000000000000000000000000000000000001/preview | 200 | 36 | 2026-09-25T19:24:50Z |  |
| 1 | badges | badge-tiers | GET | https://badges.decentraland.org/badges/dclcitizen/tiers | 200 | 21 | 2026-09-25T19:24:50Z |  |
| 1 | badges | asset-2d | GET | https://assets-cdn.decentraland.org/dclcitizen/2d/normal.png | 200 | 63767 | 2026-09-25T19:24:50Z |  |
| 1 | badges | asset-3d | GET | https://assets-cdn.decentraland.org/dclcitizen/3d/normal.png | 200 | 848778 | 2026-09-25T19:24:51Z |  |
| 1 | badges | asset-404 | GET | https://assets-cdn.decentraland.org/assets/does-not-exist/2d/normal.png | 404 | 554 | 2026-09-25T19:24:53Z |  |
| 1 | badges | preflight-categories | OPTIONS | https://badges.decentraland.org/categories | 204 | 0 | 2026-09-25T19:24:53Z |  |
| 1 | badges | unknown-path-404 | GET | https://badges.decentraland.org/does-not-exist | 404 | 9 | 2026-09-25T19:24:54Z |  |
| 1 | profile-images | face-profile-0 | GET | https://profile-images.decentraland.org/entities/bafkreickiq5m2aj4i4hi4emzcyaqvh2w2wdig325ienuqnkycz5xr5l66a/face.png | 301 | 167 | 2026-09-25T19:24:54Z |  |
| 1 | profile-images | body-profile-0 | GET | https://profile-images.decentraland.org/entities/bafkreickiq5m2aj4i4hi4emzcyaqvh2w2wdig325ienuqnkycz5xr5l66a/body.png | 301 | 167 | 2026-09-25T19:24:54Z |  |
| 1 | profile-images | face-profile-1 | GET | https://profile-images.decentraland.org/entities/bafkreifkus6iegxyerri7rbfzmxrj5c726prza7lqobufstyc54yuzxwpy/face.png | 301 | 167 | 2026-09-25T19:24:55Z |  |
| 1 | profile-images | body-profile-1 | GET | https://profile-images.decentraland.org/entities/bafkreifkus6iegxyerri7rbfzmxrj5c726prza7lqobufstyc54yuzxwpy/body.png | 301 | 167 | 2026-09-25T19:24:56Z |  |
| 1 | profile-images | face-profile-2 | GET | https://profile-images.decentraland.org/entities/bafkreig3xwydxjwphcz4yjyqlvicvstrrsfs3dix6q7jyv4qfnfge2d22i/face.png | 301 | 167 | 2026-09-25T19:24:56Z |  |
| 1 | profile-images | body-profile-2 | GET | https://profile-images.decentraland.org/entities/bafkreig3xwydxjwphcz4yjyqlvicvstrrsfs3dix6q7jyv4qfnfge2d22i/body.png | 301 | 167 | 2026-09-25T19:24:57Z |  |
| 1 | profile-images | face-head | HEAD | https://profile-images.decentraland.org/entities/bafkreickiq5m2aj4i4hi4emzcyaqvh2w2wdig325ienuqnkycz5xr5l66a/face.png | 301 | 0 | 2026-09-25T19:24:57Z |  |
| 1 | profile-images | face-unknown-cid | GET | https://profile-images.decentraland.org/entities/bafkreiaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/face.png | 301 | 167 | 2026-09-25T19:24:58Z |  |
| 1 | profile-images | body-unknown-cid | GET | https://profile-images.decentraland.org/entities/bafkreiaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/body.png | 301 | 167 | 2026-09-25T19:24:58Z |  |
| 1 | profile-images | face-bad-cid | GET | https://profile-images.decentraland.org/entities/not-a-cid/face.png | 301 | 167 | 2026-09-25T19:24:59Z |  |
| 1 | profile-images | unknown-file | GET | https://profile-images.decentraland.org/entities/bafkreickiq5m2aj4i4hi4emzcyaqvh2w2wdig325ienuqnkycz5xr5l66a/other.png | 301 | 167 | 2026-09-25T19:24:59Z |  |
| 1 | profile-images | preflight-face | OPTIONS | https://profile-images.decentraland.org/entities/bafkreickiq5m2aj4i4hi4emzcyaqvh2w2wdig325ienuqnkycz5xr5l66a/face.png | 301 | 167 | 2026-09-25T19:25:00Z |  |
| 1 | profile-images | root-404 | GET | https://profile-images.decentraland.org/ | 404 | 9 | 2026-09-25T19:25:01Z |  |
| 1 | camera-reel | health-live | GET | https://camera-reel-service.decentraland.org/health/live | 200 | 7 | 2026-09-25T19:25:01Z |  |
| 1 | camera-reel | user | GET | https://camera-reel-service.decentraland.org/api/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e | 200 | 35 | 2026-09-25T19:25:01Z |  |
| 1 | camera-reel | user-unknown-address | GET | https://camera-reel-service.decentraland.org/api/users/0x0000000000000000000000000000000000000001 | 200 | 35 | 2026-09-25T19:25:02Z |  |
| 1 | camera-reel | user-images | GET | https://camera-reel-service.decentraland.org/api/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/images?limit=3&offset=0 | 200 | 47 | 2026-09-25T19:25:02Z |  |
| 1 | camera-reel | user-images-compact-page2 | GET | https://camera-reel-service.decentraland.org/api/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/images?limit=2&offset=1&compact=true | 200 | 47 | 2026-09-25T19:25:03Z |  |
| 1 | camera-reel | user-images-limit-over-max | GET | https://camera-reel-service.decentraland.org/api/users/0x0000000000000000000000000000000000000001/images?limit=1000&offset=0 | 200 | 47 | 2026-09-25T19:25:03Z |  |
| 1 | camera-reel | user-images-bad-limit | GET | https://camera-reel-service.decentraland.org/api/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/images?limit=abc | 400 | 54 | 2026-09-25T19:25:04Z |  |
| 1 | camera-reel | image-metadata-404 | GET | https://camera-reel-service.decentraland.org/api/images/00000000-0000-4000-8000-000000000000/metadata | 404 | 29 | 2026-09-25T19:25:05Z |  |
| 1 | camera-reel | place-images | GET | https://camera-reel-service.decentraland.org/api/places/3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d/images?limit=5&offset=0 | 200 | 1713 | 2026-09-25T19:25:05Z |  |
| 1 | camera-reel | place-images-page2 | GET | https://camera-reel-service.decentraland.org/api/places/3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d/images?limit=2&offset=2 | 200 | 702 | 2026-09-25T19:25:06Z |  |
| 1 | camera-reel | place-images-unknown-place | GET | https://camera-reel-service.decentraland.org/api/places/00000000-0000-4000-8000-000000000000/images | 200 | 27 | 2026-09-25T19:25:06Z |  |
| 1 | camera-reel | places-images-batch | POST | https://camera-reel-service.decentraland.org/api/places/images?limit=5&offset=0 | 200 | 1958 | 2026-09-25T19:25:07Z |  |
| 1 | camera-reel | places-images-batch-400-empty | POST | https://camera-reel-service.decentraland.org/api/places/images | 400 | 35 | 2026-09-25T19:25:07Z |  |
| 1 | camera-reel | wearable-images-400-bad-item | GET | https://camera-reel-service.decentraland.org/api/wearables/not-an-item/images | 400 | 67 | 2026-09-25T19:25:08Z |  |
| 1 | camera-reel | docs-openapi | GET | https://camera-reel-service.decentraland.org/api/docs/openapi.json | 200 | 10374 | 2026-09-25T19:25:08Z |  |
| 1 | camera-reel | preflight-user | OPTIONS | https://camera-reel-service.decentraland.org/api/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e | 200 | 0 | 2026-09-25T19:25:09Z |  |
| 1 | camera-reel | unknown-path-404 | GET | https://camera-reel-service.decentraland.org/api/does-not-exist | 404 | 0 | 2026-09-25T19:25:10Z |  |
| 2 | social | member-invites-signed-other | GET | https://social-api.decentraland.org/v1/members/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/invites | 200 | 11 | 2026-09-25T19:29:32Z | /v1/members/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/invites |
| 2 | gatekeeper | get-scene-adapter-signed | POST | https://comms-gatekeeper.decentraland.org/get-scene-adapter | 200 | 628 | 2026-09-25T19:29:33Z | /get-scene-adapter |
| 2 | gatekeeper | scene-participants | GET | https://comms-gatekeeper.decentraland.org/scene-participants?pointer=0,0&realm_name=main | 200 | 304 | 2026-09-25T19:29:34Z |  |
| 2 | gatekeeper | scene-participants-default-realm | GET | https://comms-gatekeeper.decentraland.org/scene-participants?pointer=0,0 | 200 | 304 | 2026-09-25T19:29:35Z |  |
| 2 | gatekeeper | scene-participants-world | GET | https://comms-gatekeeper.decentraland.org/scene-participants?realm_name=genesis.dcl.eth | 200 | 35 | 2026-09-25T19:29:35Z |  |
| 2 | gatekeeper | scene-admin-signed-scene | GET | https://comms-gatekeeper.decentraland.org/scene-admin | 200 | 5408 | 2026-09-25T19:29:36Z | /scene-admin |
| 2 | gatekeeper | scene-bans-signed-scene | GET | https://comms-gatekeeper.decentraland.org/scene-bans?limit=20&offset=0 | 401 | 60 | 2026-09-25T19:29:36Z | /scene-bans |
| 2 | gatekeeper | scene-bans-signed-scene | GET | https://comms-gatekeeper.decentraland.org/scene-bans?limit=20&offset=0 | 401 | 669 | 2026-09-25T19:29:37Z | /scene-bans?limit=20&offset=0 |
| 2 | gatekeeper | scene-bans-addresses-signed-scene | GET | https://comms-gatekeeper.decentraland.org/scene-bans/addresses | 401 | 60 | 2026-09-25T19:29:37Z | /scene-bans/addresses |
| 2 | notifications | notifications-bad-signature | GET | https://notifications.decentraland.org/notifications | 401 | 188 | 2026-09-25T19:29:38Z |  |
| 2 | profile-images | face-profile-0-object | GET | https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafkreickiq5m2aj4i4hi4emzcyaqvh2w2wdig325ienuqnkycz5xr5l66a/face.png | 200 | 36077 | 2026-09-25T19:29:38Z |  |
| 2 | profile-images | body-profile-0-object | GET | https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafkreickiq5m2aj4i4hi4emzcyaqvh2w2wdig325ienuqnkycz5xr5l66a/body.png | 200 | 44260 | 2026-09-25T19:29:38Z |  |
| 2 | profile-images | face-profile-1-object | GET | https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafkreifkus6iegxyerri7rbfzmxrj5c726prza7lqobufstyc54yuzxwpy/face.png | 200 | 35355 | 2026-09-25T19:29:39Z |  |
| 2 | profile-images | body-profile-1-object | GET | https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafkreifkus6iegxyerri7rbfzmxrj5c726prza7lqobufstyc54yuzxwpy/body.png | 200 | 42753 | 2026-09-25T19:29:39Z |  |
| 2 | profile-images | face-profile-2-object | GET | https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafkreig3xwydxjwphcz4yjyqlvicvstrrsfs3dix6q7jyv4qfnfge2d22i/face.png | 200 | 35470 | 2026-09-25T19:29:40Z |  |
| 2 | profile-images | body-profile-2-object | GET | https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafkreig3xwydxjwphcz4yjyqlvicvstrrsfs3dix6q7jyv4qfnfge2d22i/body.png | 200 | 91439 | 2026-09-25T19:29:41Z |  |
| 2 | profile-images | face-unknown-cid-object | GET | https://profile-images-bucket-43d0c58.s3.us-east-1.amazonaws.com/v1/entities/bafkreiaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/face.png | 404 | 356 | 2026-09-25T19:29:41Z |  |
| 2 | camera-reel | image-redirect | GET | https://camera-reel-service.decentraland.org/api/images/e9659dd5-0f87-4169-a9a6-1ec0d5faf051 | 307 | 0 | 2026-09-25T19:29:41Z |  |
| 2 | camera-reel | image-file-redirect | GET | https://camera-reel-service.decentraland.org/api/images/e9659dd5-0f87-4169-a9a6-1ec0d5faf051-1790074770.jpg | 307 | 0 | 2026-09-25T19:29:42Z |  |
| 2 | camera-reel | image-metadata | GET | https://camera-reel-service.decentraland.org/api/images/e9659dd5-0f87-4169-a9a6-1ec0d5faf051/metadata | 200 | 5612 | 2026-09-25T19:29:42Z |  |
| 3 | gatekeeper | scene-bans-signed-scene | GET | https://comms-gatekeeper.decentraland.org/scene-bans?limit=20&offset=0 | 401 | 60 | 2026-09-25T19:30:59Z | /scene-bans |
| 3 | gatekeeper | scene-bans-signed-scene | GET | https://comms-gatekeeper.decentraland.org/scene-bans?limit=20&offset=0 | 401 | 669 | 2026-09-25T19:31:00Z | /scene-bans?limit=20&offset=0 |
| 3 | camera-reel | wearable-images | GET | https://camera-reel-service.decentraland.org/api/wearables/0x8bac6894f5e7c2a34a237d451f8cb0cf1a824ecc-0/images?limit=3&offset=0 | 200 | 12382 | 2026-09-25T19:31:01Z |  |
| 4 | social | unknown-path-404 | GET | https://social-api.decentraland.org/v1/does-not-exist | 404 | 9 | 2026-10-26T17:36:50Z |  |
| 4 | social | communities-list-origin | GET | https://social-api.decentraland.org/v1/communities?limit=1&offset=0 | 200 | 504 | 2026-10-26T17:36:50Z |  |
| 4 | social-rpc | status | GET | https://rpc-social-service-ea.decentraland.org/status | 200 | 105 | 2026-10-26T17:36:51Z |  |
| 4 | social-rpc | health-live | GET | https://rpc-social-service-ea.decentraland.org/health/live | 200 | 5 | 2026-10-26T17:36:52Z |  |
| 4 | social-rpc | privacy-settings-unknown | GET | https://rpc-social-service-ea.decentraland.org/v1/users/0x0000000000000000000000000000000000000001/privacy-settings | 200 | 34 | 2026-10-26T17:36:52Z |  |
| 4 | social-rpc | privacy-settings-bad-address | GET | https://rpc-social-service-ea.decentraland.org/v1/users/not-an-address/privacy-settings | 400 | 27 | 2026-10-26T17:36:53Z |  |
| 4 | social-rpc | unknown-path-404 | GET | https://rpc-social-service-ea.decentraland.org/v1/does-not-exist | 404 | 21 | 2026-10-26T17:36:53Z |  |
| 5 | social | mutes-post-unauth | POST | https://social-api.decentraland.org/v1/mutes | 400 | 101 | 2026-10-26T18:01:46Z |  |
| 5 | social | mutes-delete-unauth | DELETE | https://social-api.decentraland.org/v1/mutes | 400 | 101 | 2026-10-26T18:01:47Z |  |
| 6 | social | health-live | GET | https://social-api.decentraland.org/health/live | 200 | 5 | 2026-10-26T20:46:17Z |  |
| 6 | social | health-ready | GET | https://social-api.decentraland.org/health/ready | 200 | 60 | 2026-10-26T20:46:17Z |  |
| 6 | social | health-startup | GET | https://social-api.decentraland.org/health/startup | 200 | 60 | 2026-10-26T20:46:18Z |  |

## Skipped

- run 1: camera-reel/image-redirect: no image id from user-images
- run 1: camera-reel/image-metadata: no image id
- run 1: camera-reel/wearable-images: no item id in the captured photo metadata
- run 2: camera-reel/wearable-images: no item id in the captured photo metadata
