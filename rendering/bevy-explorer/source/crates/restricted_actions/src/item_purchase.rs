//! `openExplorerUi` with `EU_ITEM_PURCHASE`: a scene asks the player to confirm buying one item.
//!
//! The engine applies the gates and parses the item out of the scene's urn; the HUD owns the
//! confirmation and the purchase. An accepted request goes to the HUD on
//! `SystemApi::GetItemPurchaseRequestStream`, and the HUD reports back through
//! `SystemApi::SetItemPurchaseState`: first whether it can show the confirmation at all (that
//! answers the scene's call), then how it ended. The scene learns the verdict only, as a
//! `PBExplorerItemPurchaseResult` on its root entity: never a price, a balance or a reason.
//!
//! One request is in flight at a time, across all scenes. It stays in flight until the HUD
//! reports its end even if the requesting scene unloads, because the confirmation is still on
//! screen; nothing is written to a scene that is gone.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    rpc::{
        OpenExplorerUiResult, RpcCall, RpcCallEvent, RpcOrigin, RpcResultSender, RpcStreamSender,
    },
    sets::SceneSets,
    structs::{server_mode, PermissionType, PrimaryUser},
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_component::{
    proto_components::sdk::components::{
        common::ExplorerUi,
        pb_explorer_item_purchase_result::{Dismissed, Failed, Purchased, Status},
        pb_explorer_ui_events_result::{Event as UiEvent, UiClosed, UiOpened},
        PbExplorerItemPurchaseResult,
    },
    CrdtType, SceneComponentId, SceneEntityId,
};
use scene_runner::{
    permissions::Permission, renderer_context::RendererSceneContext, ContainingScene,
};
use serde_json::{json, Value};
use system_bridge::{
    ItemPurchaseItem, ItemPurchaseRequest, ItemPurchaseState, SetItemPurchaseState, SystemApi,
};

use crate::explorer_ui::{recent_user_gesture, write_event};

pub struct ItemPurchasePlugin;

impl Plugin for ItemPurchasePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemPurchases>()
            .add_systems(
                Update,
                (
                    open_item_purchase,
                    track_item_purchase.after(open_item_purchase),
                    write_owed_closed.after(track_item_purchase),
                )
                    .in_set(SceneSets::RestrictedActions),
            )
            .add_console_command::<ItemPurchaseStateCommand, _>(item_purchase_state)
            .add_console_command::<ItemPurchaseCancelCommand, _>(item_purchase_cancel)
            .add_debug_section(DebugSection {
                title: "Scene item purchase",
                query: "/item_purchase_state",
                fields: &[
                    ("HUD listeners", "hudListeners"),
                    ("Request", "active.state"),
                    ("Requesting scene", "active.scene"),
                    ("Item urn", "active.urn"),
                    ("Request id", "active.requestId"),
                    ("Last ended", "last.outcome"),
                    ("Last scene", "last.scene"),
                    ("Last item urn", "last.urn"),
                ],
                actions: &[("Cancel request", "/item_purchase_cancel")],
                inputs: &[],
            });
    }
}

/// 2^256 - 1, the largest item id a collection can hold.
const UINT256_MAX: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

/// The collection item a scene's urn names: `urn:decentraland:{matic|amoy}:collections-v2:
/// {contract}:{item id}`, and nothing else. A token id, a third-party collection or trailing
/// text is refused, so what reaches the HUD is only ever a contract address and a number.
fn parse_item_urn(urn: &str) -> Option<ItemPurchaseItem> {
    let mut parts = urn.strip_prefix("urn:decentraland:")?.split(':');
    let (network, kind, contract, item_id) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || !matches!(network, "matic" | "amoy") || kind != "collections-v2" {
        return None;
    }
    let hex = contract.strip_prefix("0x")?;
    if hex.len() != 40 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // a uint256 has at most 78 decimal digits; one spelling of each id, the one the Shop takes
    if item_id.is_empty()
        || item_id.len() > 78
        || !item_id.bytes().all(|b| b.is_ascii_digit())
        || (item_id.len() > 1 && item_id.starts_with('0'))
        // same length, so the decimal strings order as the numbers do
        || (item_id.len() == UINT256_MAX.len() && item_id > UINT256_MAX)
    {
        return None;
    }
    Some(ItemPurchaseItem {
        contract: contract.to_ascii_lowercase(),
        item_id: item_id.to_owned(),
    })
}

/// What the scene asked for, kept to address its events.
#[derive(Clone, Debug, PartialEq)]
struct SceneRequest {
    scene: Entity,
    scene_hash: String,
    /// the urn as the scene gave it, echoed in the result
    urn: String,
    /// the collection item the urn names, if it names one
    item: Option<ItemPurchaseItem>,
    request_id: u32,
}

enum Phase {
    /// sent to the HUD; the scene's call is unanswered until the HUD says whether it can show
    /// the confirmation. No deadline: the HUD opens the confirmation as it accepts, so a request
    /// given up on here could still be bought there. It ends only with the HUD's answer or the
    /// HUD itself.
    AwaitingHud {
        response: RpcResultSender<OpenExplorerUiResult>,
    },
    /// the HUD is showing the confirmation
    Open,
}

struct Active {
    id: u32,
    request: SceneRequest,
    phase: Phase,
}

/// How the last request ended, for the debug panel.
struct Ended {
    request: SceneRequest,
    outcome: &'static str,
}

/// What a transition owes the requesting scene.
#[derive(Debug, PartialEq)]
enum Change {
    Opened(SceneRequest),
    Closed(SceneRequest, ItemPurchaseState),
}

#[derive(Resource, Default)]
struct ItemPurchases {
    streams: Vec<RpcStreamSender<ItemPurchaseRequest>>,
    last_id: u32,
    active: Option<Active>,
    last: Option<Ended>,
}

/// On a scene whose purchase verdict was written at `result_tick` and whose `closed` event is
/// still owed. The two are different grow-only components, and a scene receives the components
/// of one tick in no set order, so `closed` waits for a later tick: a scene that stops waiting
/// on `closed` has the verdict by then. Until it is written the scene's next request is
/// `WAS_ALREADY_OPEN`, so a new `opened` never lands ahead of the previous `closed`.
#[derive(Component)]
struct ItemPurchaseClosedOwed {
    request_id: u32,
    result_tick: u32,
}

/// Whether a `closed` event may follow a verdict written at `result_tick`. The scene's tick
/// number advances when a tick's reply arrives, and a tick already running when the verdict was
/// written does not carry it, so one tick later is not enough: by two, the verdict went out with
/// an earlier tick than `closed` will.
fn closed_is_due(result_tick: u32, tick: u32) -> bool {
    tick.wrapping_sub(result_tick) >= 2
}

impl ItemPurchases {
    fn has_hud(&mut self) -> bool {
        self.streams.retain(|stream| !stream.is_closed());
        !self.streams.is_empty()
    }

    /// Sends a permitted request to the HUD, or answers it when it cannot start.
    fn begin(&mut self, request: SceneRequest, response: RpcResultSender<OpenExplorerUiResult>) {
        if self.active.is_some() {
            response.send(OpenExplorerUiResult::WasAlreadyOpen);
            return;
        }
        // the HUD may have gone while the permission prompt was up
        if !self.has_hud() {
            response.send(OpenExplorerUiResult::RejectedFeatureDisabled);
            return;
        }
        self.last_id = self.last_id.wrapping_add(1);
        let message = ItemPurchaseRequest {
            id: self.last_id,
            item: request.item.clone(),
        };
        for stream in &self.streams {
            let _ = stream.send(message.clone());
        }
        self.active = Some(Active {
            id: self.last_id,
            request,
            phase: Phase::AwaitingHud { response },
        });
    }

    /// Applies a HUD report. A report for a request that is no longer in flight, or one out of
    /// order, is ignored.
    fn report(&mut self, id: u32, state: ItemPurchaseState) -> Option<Change> {
        let active = self.active.take_if(|active| active.id == id)?;
        match (active.phase, state) {
            (Phase::AwaitingHud { response, .. }, ItemPurchaseState::Accepted) => {
                response.send(OpenExplorerUiResult::Opened);
                let request = active.request.clone();
                self.active = Some(Active {
                    phase: Phase::Open,
                    ..active
                });
                Some(Change::Opened(request))
            }
            (Phase::AwaitingHud { response, .. }, ItemPurchaseState::Unavailable) => {
                response.send(OpenExplorerUiResult::RejectedFeatureDisabled);
                self.end(active.request, "unavailable in this client");
                None
            }
            (
                Phase::Open,
                ItemPurchaseState::Purchased
                | ItemPurchaseState::Dismissed
                | ItemPurchaseState::Failed,
            ) => Some(self.close(active.request, state)),
            (phase, _) => {
                self.active = Some(Active { phase, ..active });
                None
            }
        }
    }

    /// Ends the request in flight once no HUD is listening: nothing is left to answer or finish
    /// it. With the confirmation up it reports `Dismissed`, which claims nothing about the
    /// purchase.
    fn hud_gone(&mut self) -> Option<Change> {
        if self.active.is_none() || self.has_hud() {
            return None;
        }
        let active = self.active.take()?;
        match active.phase {
            Phase::AwaitingHud { response } => {
                response.send(OpenExplorerUiResult::RejectedFeatureDisabled);
                self.end(active.request, "the HUD went away");
                None
            }
            Phase::Open => Some(self.close(active.request, ItemPurchaseState::Dismissed)),
        }
    }

    /// Drops the request in flight, whatever its phase. The HUD's confirmation is not closed
    /// by this; its later reports are ignored.
    fn cancel(&mut self) -> Option<Change> {
        let active = self.active.take()?;
        match active.phase {
            Phase::AwaitingHud { response, .. } => {
                response.send(OpenExplorerUiResult::RejectedFeatureDisabled);
                self.end(active.request, "cancelled");
                None
            }
            Phase::Open => Some(self.close(active.request, ItemPurchaseState::Dismissed)),
        }
    }

    fn close(&mut self, request: SceneRequest, state: ItemPurchaseState) -> Change {
        self.end(
            request.clone(),
            match state {
                ItemPurchaseState::Purchased => "purchased",
                ItemPurchaseState::Failed => "failed",
                _ => "dismissed",
            },
        );
        Change::Closed(request, state)
    }

    fn end(&mut self, request: SceneRequest, outcome: &'static str) {
        self.last = Some(Ended { request, outcome });
    }

    fn snapshot(&mut self) -> Value {
        let request = |request: &SceneRequest| {
            json!({
                "scene": request.scene_hash,
                "urn": request.urn,
                "requestId": request.request_id,
            })
        };
        let mut unavailable = serde_json::Map::new();
        let active = match &self.active {
            Some(active) => {
                let mut value = request(&active.request);
                value["state"] = json!(match active.phase {
                    Phase::AwaitingHud { .. } => "waiting for the HUD",
                    Phase::Open => "confirmation open",
                });
                value
            }
            None => {
                unavailable.insert("active".to_owned(), json!("No request is in flight"));
                Value::Null
            }
        };
        let last = match &self.last {
            Some(ended) => {
                let mut value = request(&ended.request);
                value["outcome"] = json!(ended.outcome);
                value
            }
            None => {
                unavailable.insert(
                    "last".to_owned(),
                    json!("No scene has requested a purchase this session"),
                );
                Value::Null
            }
        };
        json!({
            "hudListeners": if self.has_hud() { self.streams.len() } else { 0 },
            "active": active,
            "last": last,
            "unavailable": unavailable,
        })
    }
}

type OpenRequest = (SceneRequest, RpcResultSender<OpenExplorerUiResult>);

#[allow(clippy::too_many_arguments)]
fn open_item_purchase(
    mut events: EventReader<RpcCallEvent>,
    mut perms: Permission<OpenRequest>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    scenes: Query<&RendererSceneContext>,
    closed_owed: Query<(), With<ItemPurchaseClosedOwed>>,
    mut purchases: ResMut<ItemPurchases>,
    time: Res<Time>,
) {
    for (scene, request_id, urn, response) in
        events.read().filter_map(|ev| match (&ev.origin, &ev.call) {
            (
                RpcOrigin::Scene { entity: scene, .. },
                RpcCall::OpenExplorerUi {
                    ui,
                    request_id,
                    item_urn,
                    response,
                },
            ) if *ui == ExplorerUi::EuItemPurchase as i32 => {
                Some((*scene, *request_id, item_urn.clone(), response.clone()))
            }
            _ => None,
        })
    {
        let in_scene = player
            .single()
            .is_ok_and(|p| containing_scene.get(p).contains(&scene));
        let Some(context) = scenes.get(scene).ok().filter(|_| in_scene) else {
            response.send(OpenExplorerUiResult::RejectedNotCurrentScene);
            continue;
        };
        if !recent_user_gesture(context, &time) {
            response.send(OpenExplorerUiResult::RejectedNoUserGesture);
            continue;
        }
        // a portable follows the player everywhere; a headless server and a client without a
        // HUD listening have nobody to confirm
        if context.is_portable || server_mode() || !purchases.has_hud() {
            response.send(OpenExplorerUiResult::RejectedFeatureDisabled);
            continue;
        }
        // refused before any prompt: the player is not asked about a request that cannot open,
        // nor asked twice while an earlier request's prompt is still up
        if purchases.active.is_some()
            || perms.iter_pending().next().is_some()
            || closed_owed.contains(scene)
        {
            response.send(OpenExplorerUiResult::WasAlreadyOpen);
            continue;
        }
        let urn = urn.unwrap_or_default();
        let item = parse_item_urn(&urn);
        // a prompt names what the scene asked to sell, from the parsed item: never the scene's text
        let asked = item
            .as_ref()
            .map(|item| format!("Buy item {} of collection {}", item.item_id, item.contract));
        let request = SceneRequest {
            scene,
            scene_hash: context.hash.to_string(),
            urn,
            item,
            request_id,
        };
        perms.check(
            PermissionType::OpenExplorerUi,
            scene,
            (request, response),
            asked,
            // the current-scene gate above already applied
            true,
        );
    }

    for (request, response) in perms.drain_success(PermissionType::OpenExplorerUi) {
        // a prompt can be answered after the player walked out of the scene
        if !player
            .single()
            .is_ok_and(|p| containing_scene.get(p).contains(&request.scene))
        {
            response.send(OpenExplorerUiResult::RejectedNotCurrentScene);
            continue;
        }
        // its previous purchase ended while the prompt was up, and that `closed` is still owed
        if closed_owed.contains(request.scene) {
            response.send(OpenExplorerUiResult::WasAlreadyOpen);
            continue;
        }
        // begin() refuses a request another one overtook while its prompt was up
        purchases.begin(request, response);
    }

    for (_, response) in perms.drain_fail(PermissionType::OpenExplorerUi) {
        response.send(OpenExplorerUiResult::RejectedFeatureDisabled);
    }
}

fn track_item_purchase(
    mut commands: Commands,
    mut events: EventReader<SystemApi>,
    mut purchases: ResMut<ItemPurchases>,
    mut scenes: Query<&mut RendererSceneContext>,
) {
    for ev in events.read() {
        match ev {
            SystemApi::GetItemPurchaseRequestStream(stream) => {
                purchases.streams.push(stream.clone())
            }
            SystemApi::SetItemPurchaseState(SetItemPurchaseState { id, state }) => {
                if let Some(change) = purchases.report(*id, *state) {
                    apply(&mut commands, &mut scenes, change);
                }
            }
            _ => (),
        }
    }

    if let Some(change) = purchases.hud_gone() {
        apply(&mut commands, &mut scenes, change);
    }
}

/// Writes each owed `closed` event once its scene has ticked past the verdict.
fn write_owed_closed(
    mut commands: Commands,
    mut scenes: Query<(Entity, &ItemPurchaseClosedOwed, &mut RendererSceneContext)>,
) {
    for (scene, owed, mut context) in &mut scenes {
        if !closed_is_due(owed.result_tick, context.tick_number) {
            continue;
        }
        write_event(
            &mut context,
            ExplorerUi::EuItemPurchase as i32,
            owed.request_id,
            UiEvent::Closed(UiClosed {}),
        );
        commands.entity(scene).remove::<ItemPurchaseClosedOwed>();
    }
}

/// Writes a transition's events to the requesting scene. A verdict's `closed` event is owed, not
/// written: see [`ItemPurchaseClosedOwed`].
fn apply(commands: &mut Commands, scenes: &mut Query<&mut RendererSceneContext>, change: Change) {
    let ui = ExplorerUi::EuItemPurchase as i32;
    match change {
        Change::Opened(request) => {
            if let Ok(mut context) = scenes.get_mut(request.scene) {
                // the player action that opened the confirmation is spent: the scene cannot
                // open another on it once this one closes
                context.last_action_event = None;
                write_event(
                    &mut context,
                    ui,
                    request.request_id,
                    UiEvent::Opened(UiOpened {}),
                );
            }
        }
        Change::Closed(request, state) => {
            // the scene may have unloaded while the confirmation was showing
            let Ok(mut context) = scenes.get_mut(request.scene) else {
                return;
            };
            let timestamp = context.tick_number;
            context.update_crdt(
                SceneComponentId::EXPLORER_ITEM_PURCHASE_RESULT,
                CrdtType::GO_ENT,
                SceneEntityId::ROOT,
                &PbExplorerItemPurchaseResult {
                    urn: request.urn,
                    timestamp,
                    request_id: request.request_id,
                    status: Some(match state {
                        ItemPurchaseState::Purchased => Status::Purchased(Purchased {}),
                        ItemPurchaseState::Failed => Status::Failed(Failed {}),
                        _ => Status::Dismissed(Dismissed {}),
                    }),
                },
            );
            commands
                .entity(request.scene)
                .insert(ItemPurchaseClosedOwed {
                    request_id: request.request_id,
                    result_tick: timestamp,
                });
        }
    }
}

/// The scene item purchase request in flight and how the last one ended, as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/item_purchase_state")]
struct ItemPurchaseStateCommand;

fn item_purchase_state(
    mut input: ConsoleCommand<ItemPurchaseStateCommand>,
    mut purchases: ResMut<ItemPurchases>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(purchases.snapshot().to_string());
    }
}

/// Drops the scene item purchase request in flight; an open one reports dismissed to its scene.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/item_purchase_cancel")]
struct ItemPurchaseCancelCommand;

fn item_purchase_cancel(
    mut input: ConsoleCommand<ItemPurchaseCancelCommand>,
    mut purchases: ResMut<ItemPurchases>,
    mut scenes: Query<&mut RendererSceneContext>,
    mut commands: Commands,
) {
    if let Some(Ok(_)) = input.take() {
        if purchases.active.is_none() {
            input.reply_failed("no item purchase request is in flight");
            return;
        }
        if let Some(change) = purchases.cancel() {
            apply(&mut commands, &mut scenes, change);
        }
        input.reply_ok("cancelled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::rpc::{RpcResultReceiver, RpcStreamReceiver};

    const URN: &str =
        "urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000AB:12";

    fn request(urn: &str) -> SceneRequest {
        SceneRequest {
            scene: Entity::from_raw(1),
            scene_hash: "hash".to_owned(),
            urn: urn.to_owned(),
            item: parse_item_urn(urn),
            request_id: 9,
        }
    }

    fn with_hud() -> (ItemPurchases, RpcStreamReceiver<ItemPurchaseRequest>) {
        let (sender, receiver) = RpcStreamSender::channel();
        let mut purchases = ItemPurchases::default();
        purchases.streams.push(sender);
        (purchases, receiver)
    }

    fn begin(purchases: &mut ItemPurchases, urn: &str) -> RpcResultReceiver<OpenExplorerUiResult> {
        let (sender, receiver) = RpcResultSender::channel();
        purchases.begin(request(urn), sender);
        receiver
    }

    #[test]
    fn closed_follows_the_verdict_by_two_ticks() {
        // the tick running when the verdict was written does not carry it
        assert!(!closed_is_due(7, 7));
        assert!(!closed_is_due(7, 8));
        assert!(closed_is_due(7, 9));
        assert!(closed_is_due(7, 20));
        // the tick counter wraps
        assert!(!closed_is_due(u32::MAX, 0));
        assert!(closed_is_due(u32::MAX, 1));
    }

    #[test]
    fn urn_names_a_collection_item_and_nothing_else() {
        assert_eq!(
            parse_item_urn(URN),
            Some(ItemPurchaseItem {
                contract: "0x00000000000000000000000000000000000000ab".to_owned(),
                item_id: "12".to_owned(),
            })
        );
        assert!(parse_item_urn(&URN.replace("matic", "amoy")).is_some());
        assert!(parse_item_urn(&URN.replace(":12", ":0")).is_some());
        assert!(parse_item_urn(&URN.replace(":12", &format!(":{UINT256_MAX}"))).is_some());
        for bad in [
            "",
            "urn:item",
            // a minted token, not an item
            &format!("{URN}:1"),
            &URN.replace("matic", "ethereum"),
            &URN.replace("collections-v2", "collections-v1"),
            &URN.replace("collections-v2", "collections-thirdparty"),
            &URN.replace("0x", ""),
            &URN.replace("AB:", "A:"),
            &URN.replace("AB:", "ZZ:"),
            &URN.replace(":12", ":"),
            &URN.replace(":12", ":1&first=2"),
            &URN.replace(":12", ":-1"),
            // past the largest id a collection holds, at the same length
            &URN.replace(":12", &format!(":{}", "9".repeat(78))),
            &URN.replace(
                ":12",
                ":115792089237316195423570985008687907853269984665640564039457584007913129639936",
            ),
            // one spelling of each id
            &URN.replace(":12", ":012"),
            &URN.replace(":12", ":00"),
            &format!("{URN}{}", "0".repeat(77)),
            &format!(" {URN}"),
        ] {
            assert_eq!(parse_item_urn(bad), None, "{bad}");
        }
    }

    #[test]
    fn accepted_request_opens_then_closes_with_the_hud_verdict() {
        let (mut purchases, mut hud) = with_hud();
        let mut response = begin(&mut purchases, URN);

        let sent = hud.try_recv().unwrap();
        assert_eq!(sent.item, parse_item_urn(URN));
        // unanswered until the HUD says it is showing the confirmation
        assert!(response.try_recv().is_err());

        assert_eq!(
            purchases.report(sent.id, ItemPurchaseState::Accepted),
            Some(Change::Opened(request(URN)))
        );
        assert_eq!(response.try_recv(), Ok(OpenExplorerUiResult::Opened));

        assert_eq!(
            purchases.report(sent.id, ItemPurchaseState::Purchased),
            Some(Change::Closed(request(URN), ItemPurchaseState::Purchased))
        );
        assert!(purchases.active.is_none());
        // a repeated report has nothing to close
        assert_eq!(purchases.report(sent.id, ItemPurchaseState::Failed), None);
    }

    #[test]
    fn unparsable_urn_still_reaches_the_hud_without_an_item() {
        let (mut purchases, mut hud) = with_hud();
        let _response = begin(
            &mut purchases,
            "urn:decentraland:off-chain:base-avatars:eyes_00",
        );
        assert_eq!(hud.try_recv().unwrap().item, None);
    }

    #[test]
    fn second_request_is_already_open() {
        let (mut purchases, mut hud) = with_hud();
        let _first = begin(&mut purchases, URN);
        let mut second = begin(&mut purchases, URN);
        assert_eq!(second.try_recv(), Ok(OpenExplorerUiResult::WasAlreadyOpen));
        // and only the first reached the HUD
        assert!(hud.try_recv().is_ok());
        assert!(hud.try_recv().is_err());
    }

    #[test]
    fn unavailable_hud_disables_the_feature() {
        let (mut purchases, mut hud) = with_hud();
        let mut response = begin(&mut purchases, URN);
        let id = hud.try_recv().unwrap().id;
        assert_eq!(purchases.report(id, ItemPurchaseState::Unavailable), None);
        assert_eq!(
            response.try_recv(),
            Ok(OpenExplorerUiResult::RejectedFeatureDisabled)
        );
        assert!(purchases.active.is_none());
    }

    #[test]
    fn no_hud_disables_the_feature() {
        let mut purchases = ItemPurchases::default();
        let mut response = begin(&mut purchases, URN);
        assert_eq!(
            response.try_recv(),
            Ok(OpenExplorerUiResult::RejectedFeatureDisabled)
        );
    }

    #[test]
    fn slow_hud_keeps_the_request_until_it_answers() {
        let (mut purchases, mut hud) = with_hud();
        let mut response = begin(&mut purchases, URN);
        let id = hud.try_recv().unwrap().id;

        // however long the answer takes, the request is neither answered nor freed: the HUD
        // opens the confirmation as it accepts
        assert_eq!(purchases.hud_gone(), None);
        assert!(response.try_recv().is_err());
        assert!(purchases.active.is_some());

        assert!(purchases.report(id, ItemPurchaseState::Accepted).is_some());
        assert_eq!(response.try_recv(), Ok(OpenExplorerUiResult::Opened));
    }

    #[test]
    fn unanswered_request_ends_when_the_hud_goes_away() {
        let (mut purchases, hud) = with_hud();
        let mut response = begin(&mut purchases, URN);

        drop(hud);
        assert_eq!(purchases.hud_gone(), None);
        assert_eq!(
            response.try_recv(),
            Ok(OpenExplorerUiResult::RejectedFeatureDisabled)
        );
        assert!(purchases.active.is_none());
    }

    #[test]
    fn reports_out_of_order_or_for_another_request_are_ignored() {
        let (mut purchases, mut hud) = with_hud();
        let mut response = begin(&mut purchases, URN);
        let id = hud.try_recv().unwrap().id;

        // an outcome before the confirmation is showing
        assert_eq!(purchases.report(id, ItemPurchaseState::Purchased), None);
        assert_eq!(purchases.report(id + 1, ItemPurchaseState::Accepted), None);
        assert!(response.try_recv().is_err());

        assert!(purchases.report(id, ItemPurchaseState::Accepted).is_some());
        // a second ack while open
        assert_eq!(purchases.report(id, ItemPurchaseState::Accepted), None);
        assert_eq!(purchases.report(id, ItemPurchaseState::Unavailable), None);
        assert!(purchases.active.is_some());
    }

    #[test]
    fn open_request_is_dismissed_when_the_hud_goes_away() {
        let (mut purchases, mut hud) = with_hud();
        let _response = begin(&mut purchases, URN);
        let id = hud.try_recv().unwrap().id;
        purchases.report(id, ItemPurchaseState::Accepted);

        assert_eq!(purchases.hud_gone(), None);
        drop(hud);
        assert_eq!(
            purchases.hud_gone(),
            Some(Change::Closed(request(URN), ItemPurchaseState::Dismissed))
        );
        assert!(purchases.active.is_none());
    }

    #[test]
    fn cancel_frees_the_slot_in_either_phase() {
        let (mut purchases, mut hud) = with_hud();
        let mut response = begin(&mut purchases, URN);
        assert_eq!(purchases.cancel(), None);
        assert_eq!(
            response.try_recv(),
            Ok(OpenExplorerUiResult::RejectedFeatureDisabled)
        );

        let _response = begin(&mut purchases, URN);
        hud.try_recv().unwrap();
        let id = hud.try_recv().unwrap().id;
        purchases.report(id, ItemPurchaseState::Accepted);
        assert_eq!(
            purchases.cancel(),
            Some(Change::Closed(request(URN), ItemPurchaseState::Dismissed))
        );
        assert_eq!(purchases.cancel(), None);
    }

    /// The systems themselves, in an app: the gates, the HUD's reports and the owed `closed`,
    /// against the events that can land while a purchase is under way.
    mod systems {
        use super::*;
        use bevy::platform::collections::HashSet;
        use common::structs::{
            AppConfig, CurrentRealm, PermissionUsed, PermissionValue, PrimaryPlayerRes,
        };
        use dcl_types::SceneId;
        use scene_runner::{
            initialize_scene::{
                LiveScenes, PortableKind, PortableScenes, PortableSource, ScenePointers,
            },
            permissions::PermissionManager,
        };

        const HASH: &str = "hash";

        /// An app with a player standing in one scene that has just had a player action, and a
        /// HUD listening.
        fn app() -> (App, Entity, RpcStreamReceiver<ItemPurchaseRequest>) {
            let mut app = App::new();
            app.add_event::<RpcCallEvent>()
                .add_event::<SystemApi>()
                .add_event::<PermissionUsed>()
                .init_resource::<ItemPurchases>()
                .init_resource::<PermissionManager>()
                .init_resource::<AppConfig>()
                .init_resource::<CurrentRealm>()
                .init_resource::<ScenePointers>()
                .init_resource::<LiveScenes>()
                .init_resource::<PortableScenes>()
                .init_resource::<Time>()
                .add_systems(
                    Update,
                    (
                        open_item_purchase,
                        track_item_purchase.after(open_item_purchase),
                        write_owed_closed.after(track_item_purchase),
                    ),
                );
            let player = app
                .world_mut()
                .spawn((PrimaryUser::default(), GlobalTransform::default()))
                .id();
            app.insert_resource(PrimaryPlayerRes(player));

            let scene = app.world_mut().spawn_empty().id();
            let mut context = RendererSceneContext::new(
                SceneId::DUMMY,
                HASH.to_owned(),
                "storage_root".to_owned(),
                false,
                0,
                "title".to_owned(),
                IVec2::ZERO,
                HashSet::from_iter([IVec2::ZERO]),
                vec![],
                Default::default(),
                vec![],
                scene,
                0.0,
                false,
                "sdk_version",
                false,
                false,
            );
            context.last_action_event = Some(0.0);
            app.world_mut().entity_mut(scene).insert(context);
            enter(&mut app, scene);

            let (sender, hud) = RpcStreamSender::channel();
            app.world_mut()
                .send_event(SystemApi::GetItemPurchaseRequestStream(sender));
            app.update();
            (app, scene, hud)
        }

        /// Puts the player in the scene. The containment lookup answers a live scene that is
        /// either under the player's parcel or follows the player; the latter needs no parcel
        /// grid, and the scene's own context still says it is not a portable.
        fn enter(app: &mut App, scene: Entity) {
            app.world_mut()
                .resource_mut::<LiveScenes>()
                .scenes
                .insert(HASH.to_owned(), scene);
            app.world_mut().resource_mut::<PortableScenes>().insert(
                HASH.to_owned(),
                PortableSource {
                    pid: HASH.to_owned(),
                    parent_scene: None,
                    ens: None,
                    super_user: false,
                    kind: PortableKind::Startup,
                },
            );
        }

        fn leave(app: &mut App) {
            app.world_mut().resource_mut::<LiveScenes>().scenes.clear();
        }

        fn ask(
            app: &mut App,
            scene: Entity,
            request_id: u32,
        ) -> RpcResultReceiver<OpenExplorerUiResult> {
            let (response, receiver) = RpcResultSender::channel();
            app.world_mut().send_event(RpcCallEvent {
                origin: RpcOrigin::Scene {
                    entity: scene,
                    hash: HASH.into(),
                },
                call: RpcCall::OpenExplorerUi {
                    ui: ExplorerUi::EuItemPurchase as i32,
                    request_id,
                    item_urn: Some(URN.to_owned()),
                    response,
                },
            });
            app.update();
            receiver
        }

        fn report(app: &mut App, id: u32, state: ItemPurchaseState) {
            app.world_mut()
                .send_event(SystemApi::SetItemPurchaseState(SetItemPurchaseState {
                    id,
                    state,
                }));
            app.update();
        }

        fn context(app: &mut App, scene: Entity) -> Mut<'_, RendererSceneContext> {
            app.world_mut()
                .get_mut::<RendererSceneContext>(scene)
                .unwrap()
        }

        /// A new player action in the scene, and `ticks` scene ticks.
        fn act_and_tick(app: &mut App, scene: Entity, ticks: u32) {
            let mut context = context(app, scene);
            context.last_action_event = Some(0.0);
            context.tick_number += ticks;
            app.update();
        }

        fn written(app: &mut App, scene: Entity, component: SceneComponentId) -> usize {
            context(app, scene)
                .crdt_store
                .go
                .get(&component)
                .and_then(|state| state.0.get(&SceneEntityId::ROOT))
                .map_or(0, |entries| entries.len())
        }

        fn ui_events(app: &mut App, scene: Entity) -> usize {
            written(app, scene, SceneComponentId::EXPLORER_UI_EVENTS_RESULT)
        }

        fn verdicts(app: &mut App, scene: Entity) -> usize {
            written(app, scene, SceneComponentId::EXPLORER_ITEM_PURCHASE_RESULT)
        }

        /// A purchase the HUD accepted: the request id the HUD holds.
        fn open(
            app: &mut App,
            scene: Entity,
            hud: &mut RpcStreamReceiver<ItemPurchaseRequest>,
        ) -> u32 {
            let mut response = ask(app, scene, 9);
            let id = hud.try_recv().unwrap().id;
            report(app, id, ItemPurchaseState::Accepted);
            assert_eq!(response.try_recv(), Ok(OpenExplorerUiResult::Opened));
            id
        }

        fn ask_first(app: &mut App) {
            app.world_mut()
                .resource_mut::<AppConfig>()
                .default_permissions
                .insert(PermissionType::OpenExplorerUi, PermissionValue::Ask);
        }

        fn allow_prompt(app: &mut App) {
            let prompt = app
                .world_mut()
                .resource_mut::<PermissionManager>()
                .pending
                .pop_front()
                .unwrap();
            prompt.sender.send(true);
            app.update();
        }

        #[test]
        fn verdict_is_written_at_once_and_closed_two_ticks_later() {
            let (mut app, scene, mut hud) = app();
            let id = open(&mut app, scene, &mut hud);
            assert_eq!(
                (ui_events(&mut app, scene), verdicts(&mut app, scene)),
                (1, 0)
            );

            report(&mut app, id, ItemPurchaseState::Purchased);
            assert_eq!(
                (ui_events(&mut app, scene), verdicts(&mut app, scene)),
                (1, 1)
            );

            act_and_tick(&mut app, scene, 1);
            assert_eq!(ui_events(&mut app, scene), 1);
            act_and_tick(&mut app, scene, 1);
            assert_eq!(ui_events(&mut app, scene), 2);
            app.update();
            assert!(app.world().get::<ItemPurchaseClosedOwed>(scene).is_none());
            assert_eq!(
                (ui_events(&mut app, scene), verdicts(&mut app, scene)),
                (2, 1)
            );
        }

        #[test]
        fn opening_spends_the_player_action() {
            let (mut app, scene, mut hud) = app();
            let mut response = ask(&mut app, scene, 9);
            assert!(context(&mut app, scene).last_action_event.is_some());
            let id = hud.try_recv().unwrap().id;
            report(&mut app, id, ItemPurchaseState::Accepted);
            assert_eq!(response.try_recv(), Ok(OpenExplorerUiResult::Opened));
            assert!(context(&mut app, scene).last_action_event.is_none());

            // so once it is dismissed and closed, the same click cannot open another
            report(&mut app, id, ItemPurchaseState::Dismissed);
            let mut context = context(&mut app, scene);
            context.tick_number += 2;
            app.update();
            app.update();
            let mut again = ask(&mut app, scene, 10);
            assert_eq!(
                again.try_recv(),
                Ok(OpenExplorerUiResult::RejectedNoUserGesture)
            );
        }

        #[test]
        fn scene_with_closed_owed_is_busy_until_it_is_written() {
            let (mut app, scene, mut hud) = app();
            let id = open(&mut app, scene, &mut hud);
            report(&mut app, id, ItemPurchaseState::Dismissed);

            // a new click before `closed`: no new `opened` may land ahead of it
            act_and_tick(&mut app, scene, 0);
            let mut early = ask(&mut app, scene, 10);
            assert_eq!(early.try_recv(), Ok(OpenExplorerUiResult::WasAlreadyOpen));
            assert!(hud.try_recv().is_err());

            act_and_tick(&mut app, scene, 2);
            app.update();
            assert_eq!(ui_events(&mut app, scene), 2);
            let mut later = ask(&mut app, scene, 11);
            assert!(later.try_recv().is_err());
            assert!(hud.try_recv().is_ok());
        }

        #[test]
        fn scene_unloading_with_the_confirmation_open_frees_the_slot_on_the_verdict() {
            let (mut app, scene, mut hud) = app();
            let id = open(&mut app, scene, &mut hud);

            leave(&mut app);
            app.world_mut().despawn(scene);
            app.update();
            // still in flight: the confirmation is on screen
            assert!(app.world().resource::<ItemPurchases>().active.is_some());

            report(&mut app, id, ItemPurchaseState::Purchased);
            app.update();
            assert!(app.world().resource::<ItemPurchases>().active.is_none());
        }

        #[test]
        fn scene_unloading_with_closed_owed_owes_nothing() {
            let (mut app, scene, mut hud) = app();
            let id = open(&mut app, scene, &mut hud);
            report(&mut app, id, ItemPurchaseState::Purchased);
            assert!(app.world().get::<ItemPurchaseClosedOwed>(scene).is_some());

            leave(&mut app);
            app.world_mut().despawn(scene);
            app.update();
            app.update();
            let mut owed = app.world_mut().query::<&ItemPurchaseClosedOwed>();
            assert_eq!(owed.iter(app.world()).count(), 0);
        }

        #[test]
        fn prompt_answered_after_the_player_left_is_refused() {
            let (mut app, scene, mut hud) = app();
            ask_first(&mut app);
            let mut response = ask(&mut app, scene, 9);
            assert!(response.try_recv().is_err());

            leave(&mut app);
            allow_prompt(&mut app);
            assert_eq!(
                response.try_recv(),
                Ok(OpenExplorerUiResult::RejectedNotCurrentScene)
            );
            assert!(hud.try_recv().is_err());
        }

        #[test]
        fn second_request_while_a_prompt_is_up_gets_no_prompt() {
            let (mut app, scene, mut hud) = app();
            ask_first(&mut app);
            let mut first = ask(&mut app, scene, 9);
            let mut second = ask(&mut app, scene, 10);
            assert_eq!(second.try_recv(), Ok(OpenExplorerUiResult::WasAlreadyOpen));
            assert_eq!(app.world().resource::<PermissionManager>().pending.len(), 1);

            allow_prompt(&mut app);
            assert!(first.try_recv().is_err());
            assert_eq!(hud.try_recv().unwrap().item, parse_item_urn(URN));
        }
    }
}
