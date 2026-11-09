//! Upstream's badge observers (`badges/processor/src/logic/badges/*` at
//! ad8aa13) as pure edits of one wallet's [`Progress`], each the body of an
//! [`super::apply_progress`] closure. They keep upstream's document keys and
//! dedup rules, so a document the TS database import carried resumes here,
//! and like upstream they leave a completed badge untouched. Tiers,
//! completion and notifications are the write path's: an evaluator only
//! moves `steps` and its state. Thresholds come from the catalog rows, except
//! the two upstream hardcodes in its observers (3 wearables of a rarity, a
//! height of 500).

use serde_json::{Map, Value, json};

use super::Progress;

pub const DCL_CITIZEN: &str = "dclcitizen";
pub const TRAVELER: &str = "traveler";
pub const WALKABOUT_WANDERER: &str = "walkabout_wanderer";
pub const MOVES_MASTER: &str = "moves_master";
pub const SOCIAL_BUTTERFLY: &str = "social_butterfly";
pub const VERTICAL_VOYAGER: &str = "vertical_voyager";
pub const PROFILE_PRO: &str = "profile_pro";
pub const LAND_ARCHITECT: &str = "land_architect";
pub const OPEN_FOR_BUSINESS: &str = "open_for_business";
pub const EMOTE_CREATOR: &str = "emote_creator";
pub const WEARABLE_DESIGNER: &str = "wearable_designer";
pub const EMOTIONISTA: &str = "emotionista";
pub const FASHIONISTA: &str = "fashionista";
pub const REFERRAL: &str = "referral";

/// The rarity badges: (badge id, the wearable rarity it counts).
pub const RARITY_BADGES: [(&str, &str); 6] = [
    ("regally_rare", "rare"),
    ("epic_ensemble", "epic"),
    ("legendary_look", "legendary"),
    ("mythic_model", "mythic"),
    ("unique_unicorn", "unique"),
    ("exotic_elegance", "exotic"),
];

/// Every badge an evaluator here awards (presence badges are configured
/// rows, not evaluators; event and one-time badges have no observer).
pub const EVALUATED: [&str; 20] = [
    DCL_CITIZEN,
    TRAVELER,
    WALKABOUT_WANDERER,
    MOVES_MASTER,
    SOCIAL_BUTTERFLY,
    VERTICAL_VOYAGER,
    "regally_rare",
    "epic_ensemble",
    "legendary_look",
    "mythic_model",
    "unique_unicorn",
    "exotic_elegance",
    PROFILE_PRO,
    LAND_ARCHITECT,
    OPEN_FOR_BUSINESS,
    EMOTE_CREATOR,
    WEARABLE_DESIGNER,
    EMOTIONISTA,
    FASHIONISTA,
    REFERRAL,
];

/// `vertical-voyager.ts` `ELEVATION_GAIN_NEEDED`.
pub const ELEVATION_NEEDED: f64 = 500.0;
/// `legendary-look.ts` and its siblings: equipped at once.
pub const WEARABLES_OF_A_RARITY: usize = 3;
/// `moves-master.ts` `MINUTES_IN_DAY`: the kept window of used minutes.
pub const MINUTES_IN_DAY: usize = 1440;
/// `traveler.ts`: a scene counts once the session spent this long on it.
pub const TRAVELER_MIN_STAY_MS: i64 = 60_000;

fn array<'a>(p: &'a mut Progress, key: &str) -> &'a mut Vec<Value> {
    let slot = p.state.entry(key).or_insert_with(|| json!([]));
    if !slot.is_array() {
        *slot = json!([]);
    }
    slot.as_array_mut().expect("just made an array")
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn replace_state(p: &mut Progress, steps: i32, state: Map<String, Value>) {
    p.steps = steps;
    p.state = state;
}

/// `decentraland-citizen.ts`: the first `move-to-parcel` completes it, the
/// document becomes `{steps: 1, visited: <newParcel>}`.
pub fn dcl_citizen(p: &mut Progress, new_parcel: &Value) {
    if p.completed {
        return;
    }
    let mut state = Map::new();
    if !new_parcel.is_null() {
        state.insert("visited".into(), new_parcel.clone());
    }
    replace_state(p, 1, state);
}

/// `walkabout-wanderer.ts`: `steps += stepCount`. Upstream has no dedup; the
/// cursor reads each stored event once (the table is keyed by Segment's
/// `messageId`). A non-positive count is ignored (the counter is never
/// negative).
pub fn walkabout_wanderer(p: &mut Progress, step_count: i64) {
    if p.completed || step_count <= 0 {
        return;
    }
    let steps = i64::from(p.steps).saturating_add(step_count);
    p.steps = i32::try_from(steps).unwrap_or(i32::MAX);
}

/// `moves-master.ts`: one step per distinct minute after the last counted
/// one (`last_used_emote_timestamp`), with the last day of counted minutes
/// kept in `last_day_used_emotes_timestamps`.
pub fn moves_master(p: &mut Progress, timestamp_ms: i64) {
    if p.completed {
        return;
    }
    let minute = timestamp_ms.div_euclid(60_000) * 60_000;
    let last = p
        .state
        .get("last_used_emote_timestamp")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if minute as f64 <= last {
        return;
    }
    p.state
        .insert("last_used_emote_timestamp".into(), json!(minute));
    p.steps = p.steps.saturating_add(1);
    let minutes = array(p, "last_day_used_emotes_timestamps");
    minutes.push(json!(minute));
    if minutes.len() > MINUTES_IN_DAY {
        let excess = minutes.len() - MINUTES_IN_DAY;
        minutes.drain(..excess);
    }
}

/// `social-butterfly.ts`: one step per distinct passport opened
/// (`profiles_visited`), never one's own. Addresses compare case-insensitively
/// and are kept lowercased.
pub fn social_butterfly(p: &mut Progress, user: &str, receiver: &str) {
    let receiver = receiver.trim().to_ascii_lowercase();
    if receiver.is_empty() || receiver.eq_ignore_ascii_case(user) || p.completed {
        return;
    }
    let visited = array(p, "profiles_visited");
    if visited.iter().any(|v| {
        v.as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case(&receiver))
    }) {
        return;
    }
    visited.push(json!(receiver));
    p.steps = p.steps.saturating_add(1);
}

/// `vertical-voyager.ts`: a height of at least 500 completes it, the document
/// becomes `{steps: 1, height_reached}`.
pub fn vertical_voyager(p: &mut Progress, height: f64) {
    if p.completed || height.is_nan() || height < ELEVATION_NEEDED {
        return;
    }
    let mut state = Map::new();
    state.insert("height_reached".into(), json!(height));
    replace_state(p, 1, state);
}

/// One titled move of a session, as `traveler.ts` caches it.
#[derive(Debug, Clone, PartialEq)]
pub struct Move {
    pub scene_title: String,
    pub on_ms: i64,
}

/// `traveler.ts` on the session's titled moves up to and including the
/// current one (`moves.last()`): nothing on the session's first move or on a
/// move into a scene already counted; else every not-yet-counted title the
/// session stayed on for a minute or more (the time from a move to the next)
/// joins `scenes_titles_visited`, and `steps` is its size.
pub fn traveler(p: &mut Progress, moves: &[Move]) {
    let Some(current) = moves.last() else {
        return;
    };
    if p.completed {
        return;
    }
    let visited: Vec<String> = p
        .state
        .get("scenes_titles_visited")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if moves.len() == 1 || visited.contains(&current.scene_title) {
        return;
    }
    // Insertion order, as upstream's object keys.
    let mut spent: Vec<(&str, i64)> = Vec::new();
    for pair in moves.windows(2) {
        let title = pair[0].scene_title.as_str();
        if visited.iter().any(|v| v == title) {
            continue;
        }
        let time = pair[1].on_ms - pair[0].on_ms;
        match spent.iter_mut().find(|(t, _)| *t == title) {
            Some((_, total)) => *total += time,
            None => spent.push((title, time)),
        }
    }
    let mut titles: Vec<String> = Vec::new();
    for title in spent
        .into_iter()
        .filter(|(_, ms)| *ms >= TRAVELER_MIN_STAY_MS)
        .map(|(t, _)| t.to_string())
        .chain(visited)
    {
        if !titles.contains(&title) {
            titles.push(title);
        }
    }
    p.steps = i32::try_from(titles.len()).unwrap_or(i32::MAX);
    p.state
        .insert("scenes_titles_visited".into(), json!(titles));
}

/// The rarity badges (`legendary-look.ts` and siblings): at least three
/// equipped wearables of the badge's rarity complete it, the document
/// becomes `{steps: 1, completed_with: [urn, ...]}`. `equipped` is the
/// profile's distinct on-chain wearables with the rarity their entity (or
/// item) carries.
pub fn rarity_badge(p: &mut Progress, rarity: &str, equipped: &[(String, Option<String>)]) {
    if p.completed {
        return;
    }
    let matching: Vec<&String> = equipped
        .iter()
        .filter(|(_, r)| r.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(rarity)))
        .map(|(urn, _)| urn)
        .collect();
    if matching.len() < WEARABLES_OF_A_RARITY {
        return;
    }
    let mut state = Map::new();
    state.insert("completed_with".into(), json!(matching));
    replace_state(p, 1, state);
}

/// `profile-pro.ts`: a non-empty `avatars[0].description` completes it, the
/// document becomes `{steps: 1, description_added}`.
pub fn profile_pro(p: &mut Progress, description: Option<&str>) {
    let Some(description) = description.filter(|d| !d.is_empty()) else {
        return;
    };
    if p.completed {
        return;
    }
    let mut state = Map::new();
    state.insert("description_added".into(), json!(description));
    replace_state(p, 1, state);
}

/// `land-architect.ts`: any scene deployment completes it (`steps = 1`).
pub fn land_architect(p: &mut Progress) {
    if p.completed {
        return;
    }
    p.steps = 1;
}

/// `open-for-business.ts`, one sub-goal: the store deployment
/// (`store_completed`) or a collection (`collection_submitted`). Each counts
/// one step once; the badge's two steps complete it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusinessStep {
    Store,
    Collection,
}

pub fn open_for_business(p: &mut Progress, step: BusinessStep) {
    let key = match step {
        BusinessStep::Store => "store_completed",
        BusinessStep::Collection => "collection_submitted",
    };
    if p.completed || truthy(p.state.get(key)) {
        return;
    }
    p.steps = p.steps.saturating_add(1);
    p.state.insert(key.into(), Value::Bool(true));
}

/// The creators' published-item list key (`emote-creator.ts`,
/// `wearable-designer.ts`).
pub fn published_key(badge_id: &str) -> &'static str {
    if badge_id == EMOTE_CREATOR {
        "published_emotes"
    } else {
        "published_wearables"
    }
}

/// `emote-creator.ts` / `wearable-designer.ts`: one step per item not yet in
/// the list (`{itemId, createdAt}`; `itemId` is the collections subgraph's
/// `<contract>-<item>`), for every item of the creator the mirror holds.
pub fn creator(p: &mut Progress, badge_id: &str, items: &[(String, i64)]) {
    if p.completed {
        return;
    }
    let list = array(p, published_key(badge_id));
    let mut added = 0;
    for (item_id, created_at_ms) in items {
        let known = list.iter().any(|v| {
            v.get("itemId")
                .and_then(Value::as_str)
                .is_some_and(|i| i.eq_ignore_ascii_case(item_id))
        });
        if !known {
            list.push(json!({ "itemId": item_id, "createdAt": created_at_ms }));
            added += 1;
        }
    }
    p.steps = p.steps.saturating_add(added);
}

/// The buyers' purchase list key (`emotionista.ts`, `fashionista.ts`).
pub fn purchases_key(badge_id: &str) -> &'static str {
    if badge_id == EMOTIONISTA {
        "transactions_emotes_purchase"
    } else {
        "transactions_wearable_purchase"
    }
}

/// `emotionista.ts` / `fashionista.ts`: one step per transaction
/// (`{transactionHash, saleAt}`), so a transaction buying several items of
/// the category counts once.
pub fn buyer(p: &mut Progress, badge_id: &str, tx_hash: &str, sale_at_ms: i64) {
    if p.completed {
        return;
    }
    let list = array(p, purchases_key(badge_id));
    if list.iter().any(|v| {
        v.get("transactionHash")
            .and_then(Value::as_str)
            .is_some_and(|h| h.eq_ignore_ascii_case(tx_hash))
    }) {
        return;
    }
    list.push(json!({ "transactionHash": tx_hash, "saleAt": sale_at_ms }));
    p.steps = p.steps.saturating_add(1);
}

/// `referral.ts` counted `steps++` per delivered event, so a redelivery
/// counted twice; here `steps` is the count of the referrer's accepted
/// invitations.
pub fn referral(p: &mut Progress, accepted: i64) {
    if p.completed {
        return;
    }
    p.steps = i32::try_from(accepted.max(0)).unwrap_or(i32::MAX);
}

/// A profile's wearable as upstream fetched it (`badge-context.ts`
/// `getWearablesWithRarity`): on-chain only, an extended URN (with a token
/// id) cut to its item URN, lowercased.
pub fn wearable_item_urn(raw: &str) -> Option<String> {
    let urn = raw.trim().to_ascii_lowercase();
    if !urn.starts_with("urn:decentraland:") || urn.starts_with("urn:decentraland:off-chain:") {
        return None;
    }
    let parts: Vec<&str> = urn.split(':').collect();
    let item = if parts.len() > 6 && (parts[3] == "collections-v1" || parts[3] == "collections-v2")
    {
        parts[..6].join(":")
    } else {
        urn
    };
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(steps: i32, doc: Value) -> Progress {
        let mut state = match doc {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        state.remove("steps");
        Progress {
            steps,
            state,
            completed: false,
        }
    }

    fn completed(steps: i32, doc: Value) -> Progress {
        Progress {
            completed: true,
            ..progress(steps, doc)
        }
    }

    #[test]
    fn citizen_completes_on_the_first_move_and_keeps_the_parcel() {
        let mut p = progress(0, json!({}));
        dcl_citizen(&mut p, &json!("10,20"));
        assert_eq!(p.document(), json!({"steps": 1, "visited": "10,20"}));
        let mut done = completed(1, json!({"visited": "1,1"}));
        let before = done.clone();
        dcl_citizen(&mut done, &json!("2,2"));
        assert_eq!(done, before, "a completed badge is left untouched");
    }

    #[test]
    fn walkabout_adds_the_step_count_and_ignores_non_positive_counts() {
        // walkabout-wanderer.spec: 1000 then 9000 steps reach the starter tier
        let mut p = progress(0, json!({}));
        walkabout_wanderer(&mut p, 1000);
        walkabout_wanderer(&mut p, 9000);
        assert_eq!(p.steps, 10_000);
        walkabout_wanderer(&mut p, -5);
        walkabout_wanderer(&mut p, 0);
        assert_eq!(p.steps, 10_000);
        let mut p = progress(i32::MAX - 1, json!({}));
        walkabout_wanderer(&mut p, 10);
        assert_eq!(p.steps, i32::MAX);
    }

    #[test]
    fn moves_master_counts_distinct_minutes_after_the_last() {
        let mut p = progress(0, json!({}));
        let t = 1_700_000_000_000_i64;
        moves_master(&mut p, t);
        moves_master(&mut p, t + 10_000); // same minute
        moves_master(&mut p, t - 120_000); // older than the last counted
        let next = (t / 60_000 + 1) * 60_000;
        moves_master(&mut p, next + 5);
        assert_eq!(p.steps, 2);
        assert_eq!(
            p.state["last_used_emote_timestamp"],
            json!(next),
            "minute-normalized"
        );
        assert_eq!(
            p.state["last_day_used_emotes_timestamps"],
            json!([t / 60_000 * 60_000, next])
        );
    }

    #[test]
    fn moves_master_keeps_a_day_of_minutes() {
        let mut p = progress(0, json!({}));
        for m in 1..=(MINUTES_IN_DAY as i64 + 2) {
            moves_master(&mut p, m * 60_000);
        }
        let kept = p.state["last_day_used_emotes_timestamps"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(kept.len(), MINUTES_IN_DAY);
        assert_eq!(kept[0], json!(3 * 60_000), "the oldest two shifted out");
        assert_eq!(p.steps, MINUTES_IN_DAY as i32 + 2);
    }

    #[test]
    fn moves_master_resumes_from_an_imported_document() {
        let mut p = progress(
            7,
            json!({"last_used_emote_timestamp": 600_000, "last_day_used_emotes_timestamps": [600_000]}),
        );
        moves_master(&mut p, 600_000 + 59_000);
        assert_eq!(p.steps, 7, "the same minute as the imported last one");
        moves_master(&mut p, 660_000);
        assert_eq!(p.steps, 8);
    }

    #[test]
    fn social_butterfly_counts_distinct_receivers_never_oneself() {
        let user = "0x00000000000000000000000000000000000000aa";
        let mut p = progress(1, json!({"profiles_visited": ["0xBB"]}));
        social_butterfly(&mut p, user, "0xbb");
        social_butterfly(&mut p, user, user);
        social_butterfly(&mut p, user, "0x00000000000000000000000000000000000000AA");
        assert_eq!(p.steps, 1, "seen and self do not count");
        social_butterfly(&mut p, user, "0xCC");
        assert_eq!(
            p.document(),
            json!({"steps": 2, "profiles_visited": ["0xBB", "0xcc"]})
        );
    }

    #[test]
    fn vertical_voyager_needs_500() {
        let mut p = progress(0, json!({}));
        vertical_voyager(&mut p, 499.9);
        assert_eq!(p.steps, 0);
        vertical_voyager(&mut p, 500.0);
        assert_eq!(p.document(), json!({"steps": 1, "height_reached": 500.0}));
    }

    fn mv(title: &str, on_ms: i64) -> Move {
        Move {
            scene_title: title.into(),
            on_ms,
        }
    }

    #[test]
    fn traveler_counts_scenes_with_a_minute_spent_on_them() {
        let mut p = progress(0, json!({}));
        // traveler.spec: the first movement of a session counts nothing
        traveler(&mut p, &[mv("A", 0)]);
        assert_eq!(p, progress(0, json!({})));
        // 30 s on A then a move to B: A has not had its minute yet
        traveler(&mut p, &[mv("A", 0), mv("B", 30_000)]);
        assert_eq!(
            p.document(),
            json!({"steps": 0, "scenes_titles_visited": []})
        );
        // back on A after 90 s on B: B counts
        traveler(&mut p, &[mv("A", 0), mv("B", 30_000), mv("A", 120_000)]);
        assert_eq!(
            p.document(),
            json!({"steps": 1, "scenes_titles_visited": ["B"]})
        );
        // C after another 40 s on A: A sums 30 s + 40 s
        let moves = [
            mv("A", 0),
            mv("B", 30_000),
            mv("A", 120_000),
            mv("C", 160_000),
        ];
        traveler(&mut p, &moves);
        assert_eq!(
            p.document(),
            json!({"steps": 2, "scenes_titles_visited": ["A", "B"]}),
            "new titles first, then the visited ones, as upstream's Set"
        );
        // a move into a counted scene counts nothing, even with time owed
        let before = p.clone();
        traveler(&mut p, &[mv("C", 0), mv("B", 600_000)]);
        assert_eq!(p, before);
    }

    #[test]
    fn traveler_resumes_from_an_imported_document() {
        let mut p = progress(2, json!({"scenes_titles_visited": ["X", "Y"]}));
        traveler(&mut p, &[mv("X", 0), mv("Z", 300_000), mv("W", 400_000)]);
        assert_eq!(
            p.document(),
            json!({"steps": 3, "scenes_titles_visited": ["Z", "X", "Y"]}),
            "X is not counted again"
        );
    }

    #[test]
    fn rarity_badges_need_three_of_the_rarity() {
        let eq = |list: &[(&str, Option<&str>)]| -> Vec<(String, Option<String>)> {
            list.iter()
                .map(|(u, r)| (u.to_string(), r.map(str::to_string)))
                .collect()
        };
        let mut p = progress(0, json!({}));
        rarity_badge(
            &mut p,
            "legendary",
            &eq(&[
                ("a", Some("legendary")),
                ("b", Some("legendary")),
                ("c", Some("epic")),
            ]),
        );
        assert_eq!(p.steps, 0);
        rarity_badge(
            &mut p,
            "legendary",
            &eq(&[
                ("a", Some("legendary")),
                ("b", Some("Legendary")),
                ("c", Some("legendary")),
                ("d", None),
            ]),
        );
        assert_eq!(
            p.document(),
            json!({"steps": 1, "completed_with": ["a", "b", "c"]})
        );
    }

    #[test]
    fn profile_pro_needs_a_description() {
        let mut p = progress(0, json!({}));
        profile_pro(&mut p, Some(""));
        profile_pro(&mut p, None);
        assert_eq!(p.steps, 0);
        profile_pro(&mut p, Some("hi"));
        assert_eq!(p.document(), json!({"steps": 1, "description_added": "hi"}));
    }

    #[test]
    fn land_architect_keeps_the_document() {
        let mut p = progress(0, json!({"x": 1}));
        land_architect(&mut p);
        assert_eq!(p.document(), json!({"steps": 1, "x": 1}));
    }

    #[test]
    fn open_for_business_counts_each_sub_goal_once() {
        // open-for-business.spec: the store then a collection completes it
        let mut p = progress(0, json!({}));
        open_for_business(&mut p, BusinessStep::Store);
        open_for_business(&mut p, BusinessStep::Store);
        assert_eq!(p.document(), json!({"steps": 1, "store_completed": true}));
        open_for_business(&mut p, BusinessStep::Collection);
        assert_eq!(
            p.document(),
            json!({"steps": 2, "store_completed": true, "collection_submitted": true})
        );
        // an imported half: the collection was submitted under TS
        let mut p = progress(1, json!({"collection_submitted": true}));
        open_for_business(&mut p, BusinessStep::Collection);
        assert_eq!(p.steps, 1);
        open_for_business(&mut p, BusinessStep::Store);
        assert_eq!(p.steps, 2);
    }

    #[test]
    fn creators_count_items_not_listed_yet() {
        let mut p = progress(
            1,
            json!({"published_emotes": [{"itemId": "0xabc-0", "createdAt": 5}]}),
        );
        creator(
            &mut p,
            EMOTE_CREATOR,
            &[("0xabc-0".into(), 5_000), ("0xabc-1".into(), 6_000)],
        );
        assert_eq!(
            p.document(),
            json!({"steps": 2, "published_emotes": [
                {"itemId": "0xabc-0", "createdAt": 5},
                {"itemId": "0xabc-1", "createdAt": 6_000}
            ]})
        );
        let before = p.clone();
        creator(&mut p, EMOTE_CREATOR, &[("0xABC-1".into(), 6_000)]);
        assert_eq!(p, before);
        let mut w = progress(0, json!({}));
        creator(&mut w, WEARABLE_DESIGNER, &[("0xdef-3".into(), 1)]);
        assert_eq!(
            w.state["published_wearables"][0]["itemId"],
            json!("0xdef-3")
        );
    }

    #[test]
    fn buyers_count_a_transaction_once() {
        let mut p = progress(0, json!({}));
        buyer(&mut p, EMOTIONISTA, "0xaa", 1);
        buyer(&mut p, EMOTIONISTA, "0xAA", 2);
        buyer(&mut p, EMOTIONISTA, "0xbb", 3);
        assert_eq!(
            p.document(),
            json!({"steps": 2, "transactions_emotes_purchase": [
                {"transactionHash": "0xaa", "saleAt": 1},
                {"transactionHash": "0xbb", "saleAt": 3}
            ]})
        );
        let mut f = progress(
            4,
            json!({"transactions_wearable_purchase": [{"transactionHash": "0xcc", "saleAt": 0}]}),
        );
        buyer(&mut f, FASHIONISTA, "0xcc", 9);
        assert_eq!(f.steps, 4, "an imported transaction is not counted again");
    }

    #[test]
    fn referral_is_the_accepted_count() {
        let mut p = progress(9, json!({}));
        referral(&mut p, 6);
        assert_eq!(p.steps, 6, "upstream's double count is not kept");
        let mut done = completed(100, json!({}));
        referral(&mut done, 120);
        assert_eq!(done.steps, 100);
    }

    #[test]
    fn wearable_urns_are_on_chain_items() {
        assert_eq!(
            wearable_item_urn(
                "urn:decentraland:matic:collections-v2:0xAB:3:105312291668557186697918027683670432318895095400549111254310977539"
            ),
            Some("urn:decentraland:matic:collections-v2:0xab:3".into())
        );
        assert_eq!(
            wearable_item_urn(
                "urn:decentraland:ethereum:collections-v1:halloween_2019:bride_of_frankie_eyebrows:12"
            ),
            Some(
                "urn:decentraland:ethereum:collections-v1:halloween_2019:bride_of_frankie_eyebrows"
                    .into()
            )
        );
        assert_eq!(
            wearable_item_urn("urn:decentraland:matic:collections-v2:0xab:3"),
            Some("urn:decentraland:matic:collections-v2:0xab:3".into())
        );
        assert_eq!(
            wearable_item_urn("urn:decentraland:off-chain:base-avatars:eyes_00"),
            None
        );
        assert_eq!(wearable_item_urn("eyes_00"), None);
    }

    #[test]
    fn every_evaluated_badge_is_in_the_seeded_catalog() {
        let schema = include_str!(concat!(env!("DCL_ONE_ROOT"), "/schemas/40_social.sql"));
        for id in EVALUATED {
            assert!(
                schema.contains(&format!("'{id}', ")),
                "{id} is a catalog row"
            );
        }
        for (id, _) in RARITY_BADGES {
            assert!(EVALUATED.contains(&id));
        }
    }
}
