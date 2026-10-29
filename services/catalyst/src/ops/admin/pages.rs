//! The operator console's HTML (`GET /admin`, `GET /admin/{family}`): the
//! old node's server-rendered page with the progressive-enhancement script
//! that signs in over `/admin/auth/*` and posts each control's form to
//! `/admin/api/*` (`console.html`). The old page probed each bundle over
//! HTTP for its health; v2 is one process, so the page reads the module
//! states in memory instead: the sync state and failed-deployment backlog,
//! the job registry, the social RPC's connected wallets, the scene host's
//! sockets and loaded scenes, and every write switch.
//!
//! Every control posts to a route of this module (`api`, `console`), with the
//! field names those routes take. The families are the old console's groups
//! (`content`, `telemetry`, `data`, `social`, `social-rpc`, `explore` for the
//! worlds operator routes, `scene-state`, `create`), each at
//! `/admin/{family}`; the keys avoid the static `/admin/*` routes other
//! modules answer at the root (`/admin/worlds`, `/admin/blocked`,
//! `/admin/access-log`).
//!
//! Differences from the old page: the status sections render for a signed-in
//! operator only (the `/admin/api` reports they mirror take the session too;
//! a visitor sees the sign-in card), the page makes no third-party request
//! (the old template imported its fonts from Google), and the answer carries
//! `Cache-Control: no-store` and refuses framing. At the edge `/admin` stays
//! the web tier's (`deploy/nix/routes.nix`); this page answers on the node's
//! own listener, as the old one did.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;

use super::{SharedState, auth, session};
use crate::content::SyncState;
use crate::jobs::JobState;

const TEMPLATE: &str = include_str!("console.html");

/// The console's families: key, title, what they cover.
pub const FAMILIES: &[(&str, &str, &str)] = &[
    (
        "content",
        "Content core",
        "Synchronization, the deployments cache and the failed-deployments backlog",
    ),
    (
        "telemetry",
        "Telemetry",
        "Issue triage, read-only SQL, quotas, ingest, export, purge",
    ),
    (
        "data",
        "Data plane",
        "Credits ledger, MANA price overrides, the EVM relay's allowlist",
    ),
    (
        "social",
        "Social",
        "Platform moderation, communities, notification broadcasts, badges",
    ),
    (
        "social-rpc",
        "Social RPC",
        "Connected wallets, private calls, friendships, socket controls",
    ),
    (
        "explore",
        "Explore and worlds",
        "Worlds, the wallet block list, the access log, place and event moderation, POIs, explorer feature flags and denylist",
    ),
    (
        "scene-state",
        "Scene host",
        "Authoritative SDK7 scenes: reload, CRDT dump, kick, reset",
    ),
    (
        "create",
        "Creator plane",
        "Camera-reel moderation, builder curation, the asset-bundle registry",
    ),
];

pub(super) fn routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/admin", get(index))
        .route("/admin/{family}", get(family))
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

fn human(n: u64) -> String {
    let s = n.to_string();
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in b.iter().enumerate() {
        if i > 0 && (b.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

fn stat(label: &str, value: &str, small: bool, hl: bool) -> String {
    let cls = if hl { "stat hl" } else { "stat" };
    let big = if small { "big sm" } else { "big" };
    format!(
        "<div class=\"{cls}\"><div class=\"lab\">{}</div><div class=\"{big}\">{value}</div></div>",
        esc(label)
    )
}

fn kv(k: &str, v: &str) -> String {
    format!(
        "<div class=\"kv\"><div class=\"k\">{}</div><div class=\"v\">{}</div></div>",
        esc(k),
        esc(if v.is_empty() { "\u{2014}" } else { v })
    )
}

fn switch(on: bool) -> &'static str {
    if on { "on (authority)" } else { "off (503)" }
}

/// The signed-in wallet, if the request carries a valid console cookie.
fn viewer(state: &SharedState, headers: &HeaderMap) -> Option<String> {
    let cookie = auth::cookie_value(headers, session::COOKIE_NAME)?;
    session::verify(&state.settings, &cookie)
}

fn page(state: &SharedState, title: &str, active: &str, body: &str) -> Response {
    let mut nav = format!(
        "<a href=\"/admin\" class=\"nav-l{}\">Admin</a>",
        if active.is_empty() { " on" } else { "" }
    );
    for (key, title, _) in FAMILIES {
        let on = if *key == active { " on" } else { "" };
        nav.push_str(&format!(
            "<a href=\"/admin/{key}\" class=\"nav-l{on}\">{}</a>",
            esc(title)
        ));
    }
    let html = TEMPLATE
        .replace("<!--SSR:title-->", &esc(title))
        .replace("<!--SSR:realmname-->", "operator console")
        .replace("<!--SSR:nav-->", &nav)
        .replace("<!--SSR:version-->", env!("CARGO_PKG_VERSION"))
        .replace(
            "<!--SSR:commit-->",
            option_env!("CATALYST_GIT_REV").unwrap_or("\u{2014}"),
        )
        .replace("<!--SSR:network-->", &esc(&state.settings.eth_network))
        .replace(
            "<!--SSR:mode-->",
            if state.settings.writes_enabled {
                "write"
            } else {
                "read-only"
            },
        )
        .replace("<!--SSR:now-->", &chrono::Utc::now().to_rfc3339())
        .replace("<!--SSR:main-->", body);
    let mut resp = Html(html).into_response();
    let h = resp.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("frame-ancestors 'none'; base-uri 'none'; form-action 'self'"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    resp
}

fn disabled_note() -> &'static str {
    "<section><div class=\"shead\"><h3>Operator controls</h3></div><div class=\"note\">The console is disabled \u{2014} set ADMIN_ADDRESSES and SESSION_SECRET (or SESSION_SECRET_FILE) to enable sign-in.</div></section>"
}

fn sign_in_card() -> String {
    "<section><div class=\"shead\"><h3>Operator controls</h3><span class=\"c\">privileged actions \u{2014} confirm before each</span></div>\
     <div class=\"ctlcard\"><div class=\"ctlh\">Sign in</div><div class=\"ctlbody\"><p class=\"ctldesc\">Authenticate with an allowlisted wallet to see the node's status and use its controls.</p>\
     <button class=\"btn\" id=\"admin-signin\">Sign in with wallet</button><span id=\"admin-who\" class=\"who\"></span>\
     <div class=\"ctl-result\" id=\"signin-result\"></div></div></div></section>"
        .to_string()
}

fn session_card(addr: &str) -> String {
    format!(
        "<div class=\"ctlcard\"><div class=\"ctlh\">Session</div><div class=\"ctlbody\"><p class=\"ctldesc\">Signed in as <span class=\"mono who\" id=\"admin-who\">{}</span></p><button class=\"btn ghost\" id=\"admin-signout\">Sign out</button></div></div>",
        esc(addr)
    )
}

enum Input {
    Text(&'static str),
    Area(&'static str),
    Select(&'static [&'static str]),
}

struct Field(&'static str, &'static str, Input);

/// A standalone button inside a form: posts only `fields` to `action`.
struct Extra(&'static str, &'static str, &'static str, &'static str);

struct Form {
    action: &'static str,
    submit: &'static str,
    confirm: &'static str,
    fields: &'static [Field],
    extras: &'static [Extra],
    num: &'static str,
    list: &'static str,
    danger: bool,
    pre: bool,
}

const fn form(action: &'static str, submit: &'static str, fields: &'static [Field]) -> Form {
    Form {
        action,
        submit,
        confirm: "",
        fields,
        extras: &[],
        num: "",
        list: "",
        danger: false,
        pre: false,
    }
}

fn render_form(f: &Form, idx: usize) -> String {
    let mut b = format!(
        "<form class=\"ctlform{}\" data-admin-action=\"{}\"",
        if f.danger { " danger-form" } else { "" },
        esc(f.action)
    );
    if !f.confirm.is_empty() {
        b.push_str(&format!(" data-confirm=\"{}\"", esc(f.confirm)));
    }
    if !f.num.is_empty() {
        b.push_str(&format!(" data-num=\"{}\"", esc(f.num)));
    }
    if !f.list.is_empty() {
        b.push_str(&format!(" data-list=\"{}\"", esc(f.list)));
    }
    let out_id = format!("out-{idx}");
    if f.pre {
        b.push_str(&format!(" data-result=\"#{out_id}\""));
    }
    b.push('>');
    for Field(name, label, input) in f.fields {
        b.push_str(&format!("<label>{}", esc(label)));
        match input {
            Input::Text(ph) => b.push_str(&format!(
                "<input name=\"{}\" placeholder=\"{}\">",
                esc(name),
                esc(ph)
            )),
            Input::Area(ph) => b.push_str(&format!(
                "<textarea name=\"{}\" rows=\"3\" placeholder=\"{}\"></textarea>",
                esc(name),
                esc(ph)
            )),
            Input::Select(options) => {
                b.push_str(&format!("<select name=\"{}\">", esc(name)));
                for o in *options {
                    b.push_str(&format!("<option value=\"{0}\">{0}</option>", esc(o)));
                }
                b.push_str("</select>");
            }
        }
        b.push_str("</label>");
    }
    b.push_str("<div class=\"btnrow\">");
    b.push_str(&format!(
        "<button class=\"btn{}\" type=\"submit\">{}</button>",
        if f.danger { " danger" } else { "" },
        esc(f.submit)
    ));
    for Extra(label, action, fields, confirm) in f.extras {
        b.push_str(&format!(
            "<button class=\"btn ghost\" type=\"button\" data-admin-action=\"{}\" data-fields=\"{}\"{}{}>{}</button>",
            esc(action),
            esc(fields),
            if confirm.is_empty() { String::new() } else { format!(" data-confirm=\"{}\"", esc(confirm)) },
            if f.pre { format!(" data-result=\"#{out_id}\"") } else { String::new() },
            esc(label)
        ));
    }
    b.push_str("</div>");
    if f.pre {
        b.push_str(&format!(
            "<pre class=\"ctl-result sqlout\" id=\"{out_id}\"></pre>"
        ));
    } else {
        b.push_str("<div class=\"ctl-result\"></div>");
    }
    b.push_str("</form>");
    b
}

fn card(title: &str, desc: &str, forms: &[Form], counter: &mut usize) -> String {
    let mut inner = String::new();
    for f in forms {
        *counter += 1;
        inner.push_str(&render_form(f, *counter));
    }
    format!(
        "<div class=\"ctlcard\"><div class=\"ctlh\">{}</div><div class=\"ctlbody\"><p class=\"ctldesc\">{}</p>{inner}</div></div>",
        esc(title),
        esc(desc)
    )
}

const ADDRESS: Field = Field("address", "Address", Input::Text("0x\u{2026}"));
const REASON: Field = Field("reason", "Reason", Input::Text("reason (audited)"));

fn content_cards(n: &mut usize) -> String {
    card(
        "Content",
        "Sync pause/resume/force act on the running orchestrator (open while OPS_WRITES_ENABLED is off); the cache flush and the failed-deployments backlog are writes.",
        &[
            Form {
                extras: &[
                    Extra(
                        "Resume sync",
                        "/admin/api/content/sync/resume",
                        "",
                        "Resume content synchronization?",
                    ),
                    Extra(
                        "Force sync",
                        "/admin/api/content/sync/force",
                        "",
                        "Force a sync pass now?",
                    ),
                    Extra(
                        "Flush deployments cache",
                        "/admin/api/content/flush-cache",
                        "",
                        "Flush the deployments cache?",
                    ),
                ],
                confirm: "Pause content synchronization?",
                ..form("/admin/api/content/sync/pause", "Pause sync", &[])
            },
            Form {
                confirm: "Re-arm this failed deployment for the retry worker?",
                extras: &[Extra(
                    "Clear (blank: all)",
                    "/admin/api/content/failed-deployments/clear",
                    "id",
                    "Purge this failed deployment (every one when blank)?",
                )],
                ..form(
                    "/admin/api/content/failed-deployments/retry",
                    "Re-arm",
                    &[Field("id", "Failed entity id", Input::Text("bafy\u{2026}"))],
                )
            },
        ],
        n,
    )
}

fn telemetry_cards(n: &mut usize) -> String {
    card(
        "Telemetry",
        "Issue triage and the read-only SQL console (the telemetry reader role, READ ONLY, 1000 rows).",
        &[
            Form {
                confirm: "Update this issue's state?",
                ..form(
                    "/admin/api/telemetry/issue-state",
                    "Set issue state",
                    &[
                        Field(
                            "fingerprint",
                            "Fingerprint",
                            Input::Text("issue fingerprint"),
                        ),
                        Field(
                            "status",
                            "Status",
                            Input::Select(&["resolved", "ignored", "unresolved"]),
                        ),
                        Field(
                            "assignee",
                            "Assignee",
                            Input::Text("address / handle (optional)"),
                        ),
                        Field("note", "Note", Input::Text("optional note")),
                    ],
                )
            },
            Form {
                pre: true,
                ..form(
                    "/admin/api/telemetry/sql",
                    "Run query",
                    &[Field("sql", "Ad-hoc SQL", Input::Area("select ..."))],
                )
            },
        ],
        n,
    ) + &card(
        "Telemetry operations",
        "Quota, the ingest toggle, regroup/release, export/audit, and the destructive purge/bulk-delete.",
        &[
            Form {
                num: "daily_limit",
                confirm: "Set this project's daily quota (blank: remove it)?",
                ..form(
                    "/admin/api/telemetry/quota",
                    "Set quota",
                    &[
                        Field("project", "Project", Input::Text("project")),
                        Field(
                            "daily_limit",
                            "Daily limit",
                            Input::Text("events/day (blank removes)"),
                        ),
                    ],
                )
            },
            Form {
                confirm: "Change the ingest toggle?",
                extras: &[Extra(
                    "Ingest report",
                    "/admin/api/telemetry/ingest",
                    "",
                    "",
                )],
                pre: true,
                ..form(
                    "/admin/api/telemetry/ingest",
                    "Set ingest",
                    &[Field(
                        "enabled",
                        "Ingest",
                        Input::Select(&["true", "false"]),
                    )],
                )
            },
            Form {
                list: "sources",
                confirm: "Merge these fingerprints into the canonical one?",
                ..form(
                    "/admin/api/telemetry/regroup",
                    "Regroup",
                    &[
                        Field("sources", "Source fingerprints", Input::Text("fp1, fp2")),
                        Field("canonical", "Canonical", Input::Text("fingerprint")),
                    ],
                )
            },
            Form {
                confirm: "Record this release state?",
                ..form(
                    "/admin/api/telemetry/release",
                    "Mark release",
                    &[
                        Field("release", "Release", Input::Text("release id")),
                        Field("state", "State", Input::Text("state")),
                        Field("note", "Note", Input::Text("optional")),
                    ],
                )
            },
            Form {
                num: "limit",
                pre: true,
                extras: &[Extra(
                    "Audit log",
                    "/admin/api/telemetry/audit",
                    "fingerprint,action,limit",
                    "",
                )],
                ..form(
                    "/admin/api/telemetry/export",
                    "Export",
                    &[
                        Field("project", "Project", Input::Text("project (export)")),
                        Field("fingerprint", "Fingerprint", Input::Text("fingerprint")),
                        Field("action", "Action", Input::Text("action (audit)")),
                        Field("limit", "Limit", Input::Text("limit")),
                    ],
                )
            },
            Form {
                num: "older_than_days",
                danger: true,
                confirm: "PURGE telemetry data? This permanently deletes stored events.",
                extras: &[Extra(
                    "Bulk delete",
                    "/admin/api/telemetry/bulk-delete",
                    "project,fingerprint,before",
                    "BULK-DELETE the matching events? This is irreversible.",
                )],
                ..form(
                    "/admin/api/telemetry/purge",
                    "Purge",
                    &[
                        Field(
                            "older_than_days",
                            "\u{26A0} Older than (days)",
                            Input::Text("purge: days"),
                        ),
                        Field("project", "Project", Input::Text("project")),
                        Field(
                            "fingerprint",
                            "Fingerprint",
                            Input::Text("bulk-delete: fingerprint"),
                        ),
                        Field(
                            "before",
                            "Before",
                            Input::Text("bulk-delete: ISO timestamp"),
                        ),
                    ],
                )
            },
        ],
        n,
    )
}

fn data_cards(n: &mut usize) -> String {
    card(
        "Credits",
        "Grant, revoke and the claim block: the ledger records the signed-in wallet (OPS_WRITES_ENABLED).",
        &[
            Form {
                danger: true,
                confirm: "Grant these credits?",
                extras: &[Extra(
                    "Revoke",
                    "/admin/api/credits/revoke",
                    "address,amount,reason",
                    "Revoke these credits?",
                )],
                ..form(
                    "/admin/api/credits/grant",
                    "Grant",
                    &[
                        ADDRESS,
                        Field("amount", "Amount", Input::Text("credits")),
                        REASON,
                        Field("idempotencyKey", "Idempotency key", Input::Text("optional")),
                    ],
                )
            },
            Form {
                confirm: "Change this wallet's claim block?",
                ..form(
                    "/admin/api/credits/user-block",
                    "Set claim block",
                    &[
                        ADDRESS,
                        Field("blocked", "Blocked", Input::Select(&["true", "false"])),
                        REASON,
                    ],
                )
            },
        ],
        n,
    ) + &card(
        "MANA price",
        "Operator overrides of the spot feed (DATA_WRITES_ENABLED).",
        &[Form {
            confirm: "Set this price override?",
            extras: &[Extra(
                "Delete override",
                "/admin/api/price/override-delete",
                "token,vs",
                "Delete this override?",
            )],
            ..form(
                "/admin/api/price/override-set",
                "Set override",
                &[
                    Field("token", "Token", Input::Text("e.g. decentraland")),
                    Field("vs", "Currency", Input::Text("e.g. usd")),
                    Field("value", "Value", Input::Text("decimal")),
                    Field("note", "Note", Input::Text("optional")),
                ],
            )
        }],
        n,
    ) + &card(
        "EVM relay",
        "The read-only method allowlist and the per-network upstreams (process state, audited).",
        &[
            Form {
                pre: true,
                extras: &[
                    Extra("Methods", "/admin/api/rpc/methods-list", "", ""),
                    Extra("Networks", "/admin/api/rpc/networks-list", "", ""),
                    Extra(
                        "Reset methods",
                        "/admin/api/rpc/methods-reset",
                        "",
                        "Reset the allowlist to the defaults?",
                    ),
                ],
                ..form("/admin/api/rpc/config", "Config", &[])
            },
            Form {
                confirm: "Allow this method?",
                extras: &[Extra(
                    "Remove",
                    "/admin/api/rpc/methods-remove",
                    "method",
                    "Remove this method?",
                )],
                ..form(
                    "/admin/api/rpc/methods-add",
                    "Add method",
                    &[Field("method", "Method", Input::Text("eth_\u{2026}"))],
                )
            },
            Form {
                confirm: "Set this upstream?",
                extras: &[Extra(
                    "Remove network",
                    "/admin/api/rpc/networks-delete",
                    "network",
                    "Remove this network?",
                )],
                ..form(
                    "/admin/api/rpc/networks-set",
                    "Set upstream",
                    &[
                        Field("network", "Network", Input::Text("mainnet")),
                        Field("url", "URL", Input::Text("https://\u{2026}")),
                    ],
                )
            },
        ],
        n,
    )
}

fn social_cards(n: &mut usize) -> String {
    card(
        "Platform moderation",
        "Bans and warnings through comms; the signed-in wallet is the moderator of record (SOCIAL_WRITES_ENABLED).",
        &[Form {
            num: "duration",
            confirm: "Ban this user?",
            extras: &[
                Extra(
                    "Unban",
                    "/admin/api/social/user-unban",
                    "address",
                    "Unban this user?",
                ),
                Extra(
                    "Warn",
                    "/admin/api/social/user-warning",
                    "address,reason",
                    "Warn this user?",
                ),
            ],
            ..form(
                "/admin/api/social/user-ban",
                "Ban",
                &[
                    ADDRESS,
                    REASON,
                    Field("duration", "Duration (ms)", Input::Text("blank: permanent")),
                    Field("customMessage", "Message", Input::Text("optional")),
                ],
            )
        }],
        n,
    ) + &card(
        "Communities",
        "The admin listing and suspension.",
        &[
            Form {
                num: "limit",
                pre: true,
                ..form(
                    "/admin/api/communities/list",
                    "List",
                    &[
                        Field("status", "Status", Input::Text("all / active / suspended")),
                        Field("owner", "Owner", Input::Text("0x\u{2026}")),
                        Field("search", "Search", Input::Text("name")),
                        Field("limit", "Limit", Input::Text("limit")),
                    ],
                )
            },
            Form {
                confirm: "Suspend this community?",
                extras: &[Extra(
                    "Unsuspend",
                    "/admin/api/communities/unsuspend",
                    "id",
                    "Unsuspend this community?",
                )],
                ..form(
                    "/admin/api/communities/suspend",
                    "Suspend",
                    &[Field("id", "Community id", Input::Text("uuid")), REASON],
                )
            },
        ],
        n,
    ) + &card(
        "Notifications and badges",
        "A broadcast to every wallet or a list; a badge grant or revoke.",
        &[
            Form {
                confirm: "Send this broadcast?",
                ..form(
                    "/admin/api/notifications/broadcast",
                    "Broadcast",
                    &[
                        Field("type", "Type", Input::Text("a notification type")),
                        Field(
                            "metadata",
                            "Metadata",
                            Input::Area("{\"title\": \"\u{2026}\"}"),
                        ),
                        Field(
                            "addresses",
                            "Addresses",
                            Input::Text("0x\u{2026}, 0x\u{2026} (blank: everyone)"),
                        ),
                        Field(
                            "broadcastId",
                            "Broadcast id",
                            Input::Text("optional idempotency key"),
                        ),
                    ],
                )
            },
            Form {
                confirm: "Grant this badge?",
                extras: &[Extra(
                    "Revoke",
                    "/admin/api/badges/revoke",
                    "address,badge_id",
                    "Revoke this badge?",
                )],
                ..form(
                    "/admin/api/badges/grant",
                    "Grant badge",
                    &[
                        ADDRESS,
                        Field("badge_id", "Badge", Input::Text("badge id")),
                        Field("tierId", "Tier", Input::Text("optional")),
                    ],
                )
            },
        ],
        n,
    )
}

fn social_rpc_cards(n: &mut usize) -> String {
    card(
        "Social RPC",
        "Connected wallets and calls; disconnect and force-presence stay open while social writes are off.",
        &[
            Form {
                num: "limit",
                pre: true,
                extras: &[
                    Extra("Presence", "/admin/api/social-rpc/presence", "", ""),
                    Extra(
                        "Voice calls",
                        "/admin/api/social-rpc/voice-calls",
                        "limit",
                        "",
                    ),
                ],
                ..form(
                    "/admin/api/social-rpc/friendships",
                    "Friendships",
                    &[ADDRESS, Field("limit", "Limit", Input::Text("limit"))],
                )
            },
            Form {
                confirm: "Disconnect this wallet's sockets?",
                extras: &[
                    Extra(
                        "Force presence",
                        "/admin/api/social-rpc/force-presence",
                        "address,status",
                        "Broadcast this presence?",
                    ),
                    Extra(
                        "Reset settings",
                        "/admin/api/social-rpc/reset-settings",
                        "address",
                        "Reset this wallet's social settings?",
                    ),
                ],
                ..form(
                    "/admin/api/social-rpc/disconnect",
                    "Disconnect",
                    &[
                        ADDRESS,
                        Field("status", "Presence", Input::Select(&["online", "offline"])),
                    ],
                )
            },
        ],
        n,
    )
}

fn worlds_cards(n: &mut usize) -> String {
    card(
        "Worlds",
        "The worlds operator routes (WORLDS_WRITES_ENABLED for enable/disable and the block list).",
        &[
            Form {
                num: "limit,offset",
                pre: true,
                extras: &[Extra(
                    "Blocked wallets",
                    "/admin/api/worlds/blocked-list",
                    "",
                    "",
                )],
                ..form(
                    "/admin/api/worlds/list",
                    "List worlds",
                    &[
                        Field("limit", "Limit", Input::Text("limit")),
                        Field("offset", "Offset", Input::Text("offset")),
                    ],
                )
            },
            Form {
                pre: true,
                extras: &[
                    Extra(
                        "Enable",
                        "/admin/api/worlds/enable",
                        "world_name",
                        "Enable this world?",
                    ),
                    Extra(
                        "Disable",
                        "/admin/api/worlds/disable",
                        "world_name",
                        "Disable this world?",
                    ),
                    Extra(
                        "Ban status",
                        "/admin/api/worlds/ban-status",
                        "world_name,address,parcel",
                        "",
                    ),
                ],
                ..form(
                    "/admin/api/worlds/detail",
                    "Detail",
                    &[
                        Field("world_name", "World", Input::Text("name.dcl.eth")),
                        ADDRESS,
                        Field("parcel", "Parcel", Input::Text("x,y (ban status)")),
                    ],
                )
            },
            Form {
                confirm: "Block this wallet from every world?",
                extras: &[Extra(
                    "Unblock",
                    "/admin/api/worlds/blocked-remove",
                    "wallet",
                    "Unblock this wallet?",
                )],
                ..form(
                    "/admin/api/worlds/blocked-add",
                    "Block wallet",
                    &[Field("wallet", "Wallet", Input::Text("0x\u{2026}"))],
                )
            },
            Form {
                num: "limit",
                pre: true,
                ..form(
                    "/admin/api/worlds/access-log",
                    "Access log",
                    &[
                        Field("world", "World", Input::Text("optional")),
                        ADDRESS,
                        Field("limit", "Limit", Input::Text("limit")),
                    ],
                )
            },
        ],
        n,
    ) + &card(
        "Explorer overlays",
        "Local overrides over the mirrored dcl-lists POIs, feature flags and denylist (OPS_WRITES_ENABLED); reloads rebuild what is served.",
        &[
            Form {
                pre: true,
                confirm: "Add this point of interest?",
                extras: &[
                    Extra(
                        "Hide",
                        "/admin/api/places/poi-delete",
                        "position",
                        "Hide this point of interest?",
                    ),
                    Extra("List POIs", "/admin/api/places/pois-list", "", ""),
                ],
                ..form(
                    "/admin/api/places/poi-create",
                    "Add POI",
                    &[Field("position", "Parcel", Input::Text("x,y"))],
                )
            },
            Form {
                confirm: "Override this flag?",
                extras: &[
                    Extra(
                        "Clear override",
                        "/admin/api/explorer-api/flags-clear",
                        "name",
                        "Drop this override?",
                    ),
                    Extra(
                        "Reload flags",
                        "/admin/api/explorer-api/flags-reload",
                        "",
                        "",
                    ),
                ],
                ..form(
                    "/admin/api/explorer-api/flags-toggle",
                    "Set flag",
                    &[
                        Field("name", "Flag", Input::Text("explorer-\u{2026}")),
                        Field("value", "Value", Input::Select(&["true", "false"])),
                    ],
                )
            },
            Form {
                confirm: "Add this wallet to the denylist?",
                extras: &[
                    Extra(
                        "Remove",
                        "/admin/api/explorer-api/blocklist-remove",
                        "wallet",
                        "Remove this wallet from the denylist?",
                    ),
                    Extra("Reload", "/admin/api/explorer-api/blocklist-reload", "", ""),
                ],
                ..form(
                    "/admin/api/explorer-api/blocklist-add",
                    "Denylist wallet",
                    &[Field("wallet", "Wallet", Input::Text("0x\u{2026}"))],
                )
            },
            Form {
                confirm: "Revoke this sign-in request?",
                extras: &[
                    Extra("Get", "/admin/api/explorer-api/challenge-get", "id", ""),
                    Extra(
                        "Pending requests",
                        "/admin/api/explorer-api/challenges-list",
                        "",
                        "",
                    ),
                    Extra(
                        "Revoke identity",
                        "/admin/api/explorer-api/identity-revoke",
                        "id",
                        "Revoke this parked identity?",
                    ),
                    Extra(
                        "Parked identities",
                        "/admin/api/explorer-api/identities-list",
                        "",
                        "",
                    ),
                ],
                ..form(
                    "/admin/api/explorer-api/challenge-revoke",
                    "Revoke request",
                    &[Field("id", "Request or identity id", Input::Text("uuid"))],
                )
            },
            Form {
                confirm: "Set this runtime config key?",
                extras: &[
                    Extra("Get", "/admin/api/explorer-api/config-get", "key", ""),
                    Extra(
                        "Delete",
                        "/admin/api/explorer-api/config-delete",
                        "key",
                        "Delete this key?",
                    ),
                    Extra("List", "/admin/api/explorer-api/config-list", "", ""),
                ],
                ..form(
                    "/admin/api/explorer-api/config-set",
                    "Set config",
                    &[
                        Field("key", "Key", Input::Text("key")),
                        Field("value", "Value", Input::Text("value")),
                    ],
                )
            },
            Form {
                confirm: "Settle this report?",
                extras: &[Extra(
                    "List reports",
                    "/admin/api/places/reports",
                    "status,entity_id",
                    "",
                )],
                ..form(
                    "/admin/api/places/report-resolve",
                    "Resolve report",
                    &[
                        Field("id", "Report", Input::Text("report id")),
                        Field(
                            "status",
                            "Status",
                            Input::Select(&["resolved", "dismissed", "actioned", "open"]),
                        ),
                        Field("resolution", "Resolution", Input::Text("resolution")),
                        Field("notes", "Notes", Input::Text("moderator notes")),
                    ],
                )
            },
            Form {
                confirm: "Record this place decision?",
                extras: &[
                    Extra(
                        "Highlight",
                        "/admin/api/places/place-highlight",
                        "place_id,highlighted",
                        "Set this place's highlight?",
                    ),
                    Extra(
                        "Rate",
                        "/admin/api/places/place-rating",
                        "place_id,content_rating",
                        "Set this place's content rating?",
                    ),
                ],
                ..form(
                    "/admin/api/places/place-disable",
                    "Set disabled",
                    &[
                        Field("place_id", "Place", Input::Text("place id")),
                        Field("disabled", "Disabled", Input::Select(&["true", "false"])),
                        Field(
                            "highlighted",
                            "Highlighted",
                            Input::Select(&["true", "false"]),
                        ),
                        Field(
                            "content_rating",
                            "Rating",
                            Input::Select(&["RP", "E", "T", "A", "R"]),
                        ),
                        Field("reason", "Reason", Input::Text("reason (disable)")),
                    ],
                )
            },
            Form {
                confirm: "Set this world's highlight?",
                extras: &[Extra(
                    "Rate",
                    "/admin/api/places/world-rating",
                    "world_id,content_rating",
                    "Set this world's content rating?",
                )],
                ..form(
                    "/admin/api/places/world-highlight",
                    "Set highlight",
                    &[
                        Field("world_id", "World", Input::Text("id or name.dcl.eth")),
                        Field(
                            "highlighted",
                            "Highlighted",
                            Input::Select(&["true", "false"]),
                        ),
                        Field(
                            "content_rating",
                            "Rating",
                            Input::Select(&["RP", "E", "T", "A", "R"]),
                        ),
                    ],
                )
            },
            Form {
                num: "x,y,duration",
                confirm: "Create this event (pending approval)?",
                ..form(
                    "/admin/api/events/create",
                    "Create event",
                    &[
                        Field("name", "Name", Input::Text("event name")),
                        Field("start_at", "Start", Input::Text("2026-10-01T18:00:00Z")),
                        Field("duration", "Duration (ms)", Input::Text("3600000")),
                        Field("x", "X", Input::Text("parcel x")),
                        Field("y", "Y", Input::Text("parcel y")),
                        Field("world", "World event", Input::Select(&["false", "true"])),
                        Field(
                            "server",
                            "World",
                            Input::Text("name.dcl.eth (world events)"),
                        ),
                        Field("description", "Description", Input::Area("optional")),
                        Field("url", "Link", Input::Text("optional https://")),
                    ],
                )
            },
            Form {
                confirm: "Apply this moderation to the event?",
                ..form(
                    "/admin/api/events/moderate",
                    "Moderate event",
                    &[
                        Field("event_id", "Event", Input::Text("event id")),
                        Field(
                            "action",
                            "Action",
                            Input::Select(&[
                                "approve",
                                "reject",
                                "feature",
                                "unfeature",
                                "archive",
                            ]),
                        ),
                        Field("reason", "Reason", Input::Text("reason (reject)")),
                    ],
                )
            },
            Form {
                confirm: "Enable or disable this point of interest?",
                ..form(
                    "/admin/api/places/poi-update",
                    "Set POI",
                    &[
                        Field("position", "Parcel", Input::Text("x,y")),
                        Field("enabled", "Enabled", Input::Select(&["true", "false"])),
                    ],
                )
            },
        ],
        n,
    )
}

fn scene_cards(n: &mut usize) -> String {
    card(
        "Scene host",
        "The authoritative SDK7 scenes this node runs.",
        &[
            Form {
                confirm: "Reload this scene?",
                ..form(
                    "/admin/api/scene/reload",
                    "Reload scene",
                    &[Field("name", "Scene", Input::Text("scene key"))],
                )
            },
            Form {
                pre: true,
                extras: &[
                    Extra(
                        "Kick all",
                        "/admin/api/scene-state/kick-all",
                        "scene",
                        "Disconnect every socket of this scene?",
                    ),
                    Extra(
                        "Reset",
                        "/admin/api/scene-state/reset",
                        "scene",
                        "Reset this scene's state?",
                    ),
                ],
                ..form(
                    "/admin/api/scene-state/crdt",
                    "CRDT dump",
                    &[Field("scene", "Scene", Input::Text("scene key"))],
                )
            },
        ],
        n,
    )
}

fn create_cards(n: &mut usize) -> String {
    card(
        "Camera reel and builder",
        "Photo moderation and item curation (CREATE_WRITES_ENABLED).",
        &[
            Form {
                confirm: "Review this photo?",
                extras: &[Extra(
                    "Delete photo",
                    "/admin/api/camera-reel/image-delete",
                    "image_id",
                    "Delete this photo?",
                )],
                ..form(
                    "/admin/api/camera-reel/image-review",
                    "Set review",
                    &[
                        Field("image_id", "Image id", Input::Text("uuid")),
                        Field(
                            "reviewStatus",
                            "Review",
                            Input::Select(&["ok", "flagged", "rejected"]),
                        ),
                    ],
                )
            },
            Form {
                list: "item_ids",
                confirm: "Set this curation status?",
                extras: &[Extra(
                    "Set for listed items",
                    "/admin/api/builder/items-status",
                    "collection_id,item_ids,status",
                    "Set the status of every listed item?",
                )],
                ..form(
                    "/admin/api/builder/item-status",
                    "Set item status",
                    &[
                        Field("collection_id", "Collection", Input::Text("uuid")),
                        Field("item_id", "Item", Input::Text("uuid")),
                        Field("item_ids", "Items (bulk)", Input::Text("uuid, uuid")),
                        Field(
                            "status",
                            "Status",
                            Input::Select(&["approved", "rejected", "pending"]),
                        ),
                    ],
                )
            },
        ],
        n,
    ) + &card(
        "Asset-bundle registry (abgen)",
        "The external abgen runtime on this host: re-ingest, cache flush, build queues and its denylist (OPS_WRITES_ENABLED).",
        &[
            Form {
                pre: true,
                extras: &[
                    Extra(
                        "Pause queues",
                        "/admin/api/create/queues-pause",
                        "",
                        "Pause the build queues?",
                    ),
                    Extra(
                        "Resume queues",
                        "/admin/api/create/queues-resume",
                        "",
                        "Resume the build queues?",
                    ),
                    Extra(
                        "Retry failed",
                        "/admin/api/create/queues-retry",
                        "",
                        "Retry the failed builds?",
                    ),
                    Extra(
                        "Re-ingest registry",
                        "/admin/api/create/registry-reingest",
                        "",
                        "Re-ingest the registry?",
                    ),
                    Extra(
                        "Flush AB cache",
                        "/admin/api/create/flush-ab-cache",
                        "",
                        "Flush the asset-bundle cache?",
                    ),
                ],
                ..form("/admin/api/create/queues-status", "Queue status", &[])
            },
            Form {
                confirm: "Denylist this entity?",
                extras: &[Extra(
                    "Remove",
                    "/admin/api/create/denylist-remove",
                    "entity_id",
                    "Remove this entity from the denylist?",
                )],
                ..form(
                    "/admin/api/create/denylist-add",
                    "Denylist add",
                    &[Field("entity_id", "Entity id", Input::Text("bafy\u{2026}"))],
                )
            },
        ],
        n,
    )
}

fn controls(family: Option<&str>) -> String {
    let mut n = 0usize;
    let want = |key: &str| family.is_none_or(|f| f == key);
    let mut b = String::new();
    for (key, render) in [
        ("content", content_cards as fn(&mut usize) -> String),
        ("telemetry", telemetry_cards),
        ("data", data_cards),
        ("social", social_cards),
        ("social-rpc", social_rpc_cards),
        ("explore", worlds_cards),
        ("scene-state", scene_cards),
        ("create", create_cards),
    ] {
        if want(key) {
            b.push_str(&render(&mut n));
        }
    }
    b
}

async fn status(state: &SharedState) -> String {
    let sync = &state.content.sync;
    let sync_state = match sync.state() {
        SyncState::Bootstrapping => "Bootstrapping".to_string(),
        SyncState::PartiallySynced { .. } => "PartiallySynced".to_string(),
        SyncState::Syncing => "Syncing".to_string(),
        SyncState::Failed => "Failed".to_string(),
    };
    let control = match &state.sync_control {
        None => "no sync worker",
        Some(c) if c.is_paused() => "paused",
        Some(_) => "running",
    };
    let failed: Option<i64> =
        sqlx::query_scalar("SELECT count(*)::bigint FROM content.failed_deployments")
            .fetch_one(&state.pool)
            .await
            .ok();
    let jobs = state.jobs.report();
    let running = jobs
        .iter()
        .filter(|j| matches!(j.state, JobState::Running))
        .count() as u64;
    let failed_jobs = jobs
        .iter()
        .filter(|j| matches!(j.state, JobState::Failed))
        .count() as u64;
    let d = &state.domains;
    let online = d
        .social_rpc
        .as_ref()
        .map(|r| r.ctx.presence_snapshot().len() as u64);
    let (scene_conns, scenes) = match &d.scene_host {
        Some(h) => (
            Some(u64::from(h.scenes.connections())),
            Some(h.scenes.loaded().len() as u64),
        ),
        None => (None, None),
    };
    let opt = |v: Option<u64>| v.map(human).unwrap_or_else(|| "\u{2014}".into());

    let mut b = String::new();
    b.push_str("<section><div class=\"shead\"><h3>Live activity</h3><span class=\"c\">this node, in memory</span></div><div class=\"statusbar\">");
    b.push_str(&stat("content sync", &esc(&sync_state), true, true));
    b.push_str(&stat("sync control", control, true, false));
    b.push_str(&stat(
        "failed deployments",
        &failed
            .map(|f| human(f.max(0) as u64))
            .unwrap_or_else(|| "unavailable".into()),
        false,
        false,
    ));
    b.push_str(&stat("jobs running", &human(running), false, false));
    b.push_str(&stat("jobs failed", &human(failed_jobs), false, false));
    b.push_str(&stat("wallets online", &opt(online), false, false));
    b.push_str(&stat("scene sockets", &opt(scene_conns), false, false));
    b.push_str(&stat("loaded scenes", &opt(scenes), false, false));
    b.push_str("</div></section>");

    b.push_str("<section><div class=\"shead\"><h3>Write switches</h3><span class=\"c\">off answers 503 with Retry-After</span></div><div class=\"grid\">");
    b.push_str(&kv(
        "OPS_WRITES_ENABLED",
        switch(state.settings.writes_enabled),
    ));
    b.push_str(&kv(
        "SOCIAL_WRITES_ENABLED",
        switch(d.social_writes_enabled),
    ));
    b.push_str(&kv(
        "WORLDS_WRITES_ENABLED",
        switch(d.worlds_writes_enabled),
    ));
    if let Some(p) = &d.price {
        b.push_str(&kv(
            "DATA_WRITES_ENABLED",
            switch(p.settings.writes_enabled),
        ));
    }
    if let Some(c) = &d.camera_reel {
        b.push_str(&kv(
            "CREATE_WRITES_ENABLED",
            switch(c.settings.writes_enabled),
        ));
    }
    b.push_str(&kv("network", &state.settings.eth_network));
    b.push_str(&kv(
        "sync frontier (ms)",
        &sync
            .sync_frontier_ms()
            .map(|f| f.to_string())
            .unwrap_or_default(),
    ));
    b.push_str("</div></section>");

    b.push_str("<section><div class=\"shead\"><h3>Jobs</h3><span class=\"c\">the job registry</span></div>");
    b.push_str("<table><thead><tr><th>Job</th><th>State</th><th>Progress</th><th>Last error</th></tr></thead><tbody>");
    for j in &jobs {
        let st = match j.state {
            JobState::Running => "running",
            JobState::Standby => "standby",
            JobState::StoodDown => "stood down",
            JobState::Finished => "finished",
            JobState::Failed => "failed",
        };
        b.push_str(&format!(
            "<tr><td class=\"mono\">{}</td><td>{st}</td><td class=\"mono\">{}</td><td class=\"mono\">{}</td></tr>",
            esc(&j.name),
            esc(j.progress.as_deref().unwrap_or("")),
            esc(j.last_error.as_deref().unwrap_or(""))
        ));
    }
    b.push_str("</tbody></table></section>");
    b
}

fn families_table() -> String {
    let mut b = String::from(
        "<section><div class=\"shead\"><h3>Families</h3><span class=\"c\">one page per module group</span></div><div class=\"links\">",
    );
    for (key, title, desc) in FAMILIES {
        b.push_str(&format!(
            "<a class=\"link\" href=\"/admin/{key}\"><div class=\"lt\">{}</div><div class=\"ld\">{}</div><div class=\"lu\">/admin/{key}</div></a>",
            esc(title),
            esc(desc)
        ));
    }
    b.push_str("</div></section>");
    b
}

/// `GET /admin`: the status and every control (the old `controls(.., "all")`).
pub async fn index(State(state): State<SharedState>, headers: HeaderMap) -> Response {
    let mut b = String::from("<main><div class=\"wrap\">");
    if !state.settings.admin_enabled() {
        b.push_str(disabled_note());
    } else if let Some(addr) = viewer(&state, &headers) {
        b.push_str("<section><div class=\"shead\"><h3>Operator controls</h3><span class=\"c\">privileged actions \u{2014} confirm before each</span></div>");
        b.push_str(&session_card(&addr));
        b.push_str("</section>");
        b.push_str(&status(&state).await);
        b.push_str(&families_table());
        b.push_str("<section><div class=\"shead\"><h3>Every control</h3></div>");
        b.push_str(&controls(None));
        b.push_str("</section>");
    } else {
        b.push_str(&sign_in_card());
    }
    b.push_str("<div class=\"note\">/admin is not routed at the public edge (the web tier answers there): reach it on the node's own listener, over the loopback or the private network.</div>");
    b.push_str("</div></main>");
    page(&state, "catalyst \u{2014} admin", "", &b)
}

/// `GET /admin/{family}`: one family's controls, 404 for an unknown key.
pub async fn family(
    State(state): State<SharedState>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some((key, title, desc)) = FAMILIES.iter().find(|(k, _, _)| *k == key) else {
        let body = "<main><div class=\"wrap\"><a class=\"back\" href=\"/admin\">\u{2190} back to admin</a><div class=\"note\">no such family</div></div></main>";
        let mut resp = page(&state, "catalyst \u{2014} not found", "", body);
        *resp.status_mut() = StatusCode::NOT_FOUND;
        return resp;
    };
    let mut b = format!(
        "<main><div class=\"wrap\"><a class=\"back\" href=\"/admin\">\u{2190} back to admin</a><div class=\"shead\"><h3>{}</h3><span class=\"c\">{}</span></div>",
        esc(title),
        esc(desc)
    );
    if !state.settings.admin_enabled() {
        b.push_str(disabled_note());
    } else if let Some(addr) = viewer(&state, &headers) {
        b.push_str(&session_card(&addr));
        if *key == "content" {
            b.push_str(&status(&state).await);
        }
        b.push_str(&controls(Some(key)));
    } else {
        b.push_str(&sign_in_card());
    }
    b.push_str("</div></main>");
    page(&state, &format!("catalyst \u{2014} {title}"), key, &b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_placeholders_are_all_filled() {
        const FILLED: &[&str] = &[
            "title",
            "realmname",
            "nav",
            "version",
            "commit",
            "network",
            "mode",
            "now",
            "main",
        ];
        let mut rest = TEMPLATE;
        let mut seen = Vec::new();
        while let Some(i) = rest.find("<!--SSR:") {
            let after = &rest[i + "<!--SSR:".len()..];
            let end = after.find("-->").expect("unterminated SSR placeholder");
            let key = &after[..end];
            assert!(
                FILLED.contains(&key),
                "template has unfilled placeholder: {key}"
            );
            seen.push(key.to_string());
            rest = &after[end + 3..];
        }
        for key in FILLED {
            assert!(
                seen.iter().any(|s| s == key),
                "the template lost <!--SSR:{key}-->"
            );
        }
        assert!(
            !TEMPLATE.contains("fonts.googleapis.com"),
            "no third-party request from the operator page"
        );
    }

    #[test]
    fn every_control_posts_to_a_mounted_console_route() {
        let html = controls(None);
        let mut actions: Vec<&str> = html
            .split("data-admin-action=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap())
            .collect();
        actions.sort_unstable();
        actions.dedup();
        let mounted: Vec<&str> = super::super::console::CONSOLE_ROUTES
            .iter()
            .copied()
            .chain(super::super::OPEN_WHILE_WRITES_DISABLED.iter().copied())
            .chain(
                super::super::ADMIN_API_ROUTES
                    .iter()
                    .filter(|(m, _)| *m == "POST")
                    .map(|(_, p)| *p),
            )
            .collect();
        for a in &actions {
            assert!(
                mounted.contains(a),
                "the form posts to {a}, which no /admin/api route answers"
            );
        }
        for route in super::super::console::CONSOLE_ROUTES {
            assert!(
                actions.contains(route),
                "{route} has no control on the page"
            );
        }
    }

    #[test]
    fn escaping_neutralizes_markup_and_numbers_group() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(human(1234567), "1,234,567");
        assert_eq!(human(0), "0");
    }

    #[tokio::test]
    async fn a_family_key_is_not_shadowed_by_the_social_rpc_admin_nest() {
        use axum::body::Body;
        use tower::ServiceExt;
        let nested = axum::Router::new().nest(
            "/admin/social",
            axum::Router::new().route("/presence", get(|| async { "presence" })),
        );
        let pages = axum::Router::new().route(
            "/admin/{family}",
            get(|Path(k): Path<String>| async move { k }),
        );
        let app = nested.merge(pages);
        for (path, expect) in [
            ("/admin/social", "social"),
            ("/admin/social-rpc", "social-rpc"),
        ] {
            let resp = app
                .clone()
                .oneshot(axum::http::Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
            assert_eq!(&body[..], expect.as_bytes(), "{path}");
        }
    }
}
