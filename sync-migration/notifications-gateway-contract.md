# Notifications: gateway and sync boundaries

Reviewed against refreshed upstream [7d01cdb8](https://github.com/decentraland/notifications-workers/tree/7d01cdb8ef8c71874a79ae9c9f5f9a8751b80507). No Notifications replication adapter or production route is enabled. Follow the [decision summary](README.md): native replay first, selective outcomes only where necessary.

## Gateway behavior

The [source routes](https://github.com/decentraland/notifications-workers/blob/7d01cdb8ef8c71874a79ae9c9f5f9a8751b80507/inbox/src/controllers/routes.ts) distinguish actual reads from effects:

| Endpoint | Treatment |
| --- | --- |
| `GET /notifications` | Reads addressed/broadcast inbox data. Eligible for explicit shadow comparison only after matching target IDs, read receipts, authentication and freshness are proven. |
| `GET /subscription` | Reads subscription/default preferences and pending email. Same readiness gate; no response bodies or credentials in comparator logs. |
| `GET /unsubscribe/{address}[/{type}]` | **Mutation:** changes subscription preferences and emits analytics. Keep production-only; never shadow because it uses GET. |
| `PUT /notifications/read` | Native command candidate. Requires the same notification/broadcast IDs and atomic source acceptance; not implemented. |
| Subscription/opt-out writes | Native command candidates with null/default/delete semantics and source acceptance still to establish. |
| Email set/confirm/send | Production effect owner. Preserve confirmation identity and one-time/provider semantics; no speculative duplication. |

Gateway [MR !446](https://dcl.tools/ops/cloudflare-workers/-/merge_requests/446) now restricts shadow paths to a reviewed source-read set in addition to the operator's exact allowlist. Events and Notifications have production-only configuration templates. The reviewed reads do not authorize target activation: the current isolated follower has no public Notifications API. Real origins/Worker bindings, DNS routes and signed paths remain deployment checks.

The unsubscribe finding comes from the [handlers](https://github.com/decentraland/notifications-workers/blob/7d01cdb8ef8c71874a79ae9c9f5f9a8751b80507/inbox/src/controllers/handlers/unsubscription-handlers.ts), not a method-name assumption. The previous comparator also required a JSON primary response; these handlers normally return HTML. The new restriction prevents relying on response format as the only barrier to a mutating GET being copied.

## Native commands and residual candidates

[Read marking](https://github.com/decentraland/notifications-workers/blob/7d01cdb8ef8c71874a79ae9c9f5f9a8751b80507/common/src/adapters/db.ts) separately updates addressed rows and inserts per-wallet broadcast markers. Its response counts are retry-sensitive. Two uncoordinated calls do not establish one atomic receipt or matching response. Target [read-state operations](../services/catalyst/social/src/notifications/store.rs) must execute natively after accepted source identity and the baseline are established; copying these request-owned rows is not the default.

Queue-created notifications remain residual **candidates**, not an approved table mirror. The [parser](https://github.com/decentraland/notifications-workers/blob/7d01cdb8ef8c71874a79ae9c9f5f9a8751b80507/processor/src/logic/event-parser.ts) derives records from event keys, timestamps and configuration. The [service](https://github.com/decentraland/notifications-workers/blob/7d01cdb8ef8c71874a79ae9c9f5f9a8751b80507/processor/src/adapters/notifications-service.ts) filters against current opt-outs and starts email work after inserts. Assess replay of the accepted event plus resolved configuration/filter decisions before choosing exact output sync. Do not attach a second competing queue consumer or rerun provider delivery.

Any approved residual must preserve addressed versus global broadcast identity, ordering against read commands, deletes and per-wallet markers. Financial notification types cannot silently expand the excluded financial-service scope; define the allowed event types first.

## Unresolved source reliability issue

The refreshed [message processor](https://github.com/decentraland/notifications-workers/blob/7d01cdb8ef8c71874a79ae9c9f5f9a8751b80507/processor/src/adapters/message-processor.ts) still deletes the SQS message in `finally` after `saveNotifications` fails. Successful partial inserts can also precede a later failure because persistence loops over records. A safe source fix needs failed-write/redelivery tests and a defined poison-message policy; it must not claim email exactly-once delivery, since mail is deferred without a durable completion receipt in this path.

Neither gateway replay nor DB capture recovers a source event acknowledged before its facts commit. This source fix is pending. Notifications source files were not modified in this round; existing local unrelated files were preserved.
