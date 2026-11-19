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

/// The collection item a scene's urn names: `urn:decentraland:{matic|amoy}:collections-v2:
/// {contract}:{item id}`, and nothing else. A token id, a third-party collection or trailing
/// text is refused, so what reaches the HUD is only ever a contract address and a number.
/// 2^256 - 1, the largest item id a collection can hold.
const UINT256_MAX: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

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
        // refused before any prompt: the player is not asked about a request that cannot open
        if purchases.active.is_some() {
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
        // begin() refuses a request another one overtook while its prompt was up
        purchases.begin(request, response);
    }

    for (_, response) in perms.drain_fail(PermissionType::OpenExplorerUi) {
        response.send(OpenExplorerUiResult::RejectedFeatureDisabled);
    }
}

fn track_item_purchase(
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
                    apply(&mut scenes, change);
                }
            }
            _ => (),
        }
    }

    if let Some(change) = purchases.hud_gone() {
        apply(&mut scenes, change);
    }
}

/// Writes a transition's events to the requesting scene, the verdict before `closed`.
fn apply(scenes: &mut Query<&mut RendererSceneContext>, change: Change) {
    let ui = ExplorerUi::EuItemPurchase as i32;
    match change {
        Change::Opened(request) => {
            if let Ok(mut context) = scenes.get_mut(request.scene) {
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
            write_event(
                &mut context,
                ui,
                request.request_id,
                UiEvent::Closed(UiClosed {}),
            );
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
) {
    if let Some(Ok(_)) = input.take() {
        if purchases.active.is_none() {
            input.reply_failed("no item purchase request is in flight");
            return;
        }
        if let Some(change) = purchases.cancel() {
            apply(&mut scenes, change);
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
}
