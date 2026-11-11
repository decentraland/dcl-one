//! Saved mutes for nearby voice: the wallets the local user hushed. The list is kept per account
//! by the social service (`/v1/mutes`), mirrored here, and applied only on this client, where the
//! nearby voice of a muted wallet plays at volume 0. Calls and community streams ignore it.

use std::collections::{BTreeSet, HashSet};

use alloy_core::primitives::Address;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    base_domain::{self, Service},
    rpc::RpcStreamSender,
    util::{AsH160, TaskCompat, TaskExt},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use ipfs::IpfsAssetServer;
use serde::Deserialize;
use serde_json::json;
use system_bridge::SystemApi;
use wallet::{sign_service_request, Wallet};

const PAGE_SIZE: usize = 100;
/// Stops a list whose reported total never ends: 10 000 mutes.
const MAX_PAGES: usize = 100;

pub struct NearbyMutesPlugin;

impl Plugin for NearbyMutesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NearbyMutes>()
            .init_resource::<NearbyMuteStreams>()
            .add_systems(
                Update,
                (follow_account, respond_to_system_api, publish).chain(),
            )
            .add_console_command::<NearbyMutesCommand, _>(nearby_mutes_command)
            .add_console_command::<NearbyMuteCommand, _>(nearby_mute_command)
            .add_debug_section(DebugSection {
                title: "Nearby voice mutes",
                query: "/nearby_mutes",
                fields: &[
                    ("Account", "account"),
                    ("Saved list", "load"),
                    ("Muted wallets", "count"),
                    ("Muted (up to 32)", "muted"),
                ],
                actions: &[],
                inputs: &[
                    DebugInput {
                        label: "Hush (wallet address)",
                        field: "",
                        command: "/nearby_mute on {value}",
                        kind: DebugInputKind::Text,
                    },
                    DebugInput {
                        label: "Hear (wallet address)",
                        field: "",
                        command: "/nearby_mute off {value}",
                        kind: DebugInputKind::Text,
                    },
                ],
            });
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Load {
    /// No account, so nothing to read.
    #[default]
    Idle,
    Loading,
    Loaded,
    Failed,
}

impl Load {
    fn as_str(self) -> &'static str {
        match self {
            Load::Idle => "idle",
            Load::Loading => "loading",
            Load::Loaded => "loaded",
            Load::Failed => "failed",
        }
    }
}

#[derive(Resource, Default)]
pub struct NearbyMutes {
    muted: HashSet<Address>,
    /// Wallets unmuted since the account's list was requested: a snapshot that arrives later
    /// must not mute them again.
    session_unmuted: HashSet<Address>,
    account: Option<Address>,
    load: Load,
    /// Counts the changes to `muted`, for the streams.
    revision: u64,
}

impl NearbyMutes {
    pub fn is_muted(&self, address: Address) -> bool {
        self.muted.contains(&address)
    }

    fn set_muted(&mut self, address: Address, muted: bool) -> bool {
        let changed = if muted {
            self.session_unmuted.remove(&address);
            self.muted.insert(address)
        } else {
            self.session_unmuted.insert(address);
            self.muted.remove(&address)
        };
        self.revision += u64::from(changed);
        changed
    }

    /// The server's list only adds: what the user muted before it arrived stays.
    fn merge(&mut self, saved: impl IntoIterator<Item = Address>) {
        for address in saved {
            if !self.session_unmuted.contains(&address) && self.muted.insert(address) {
                self.revision += 1;
            }
        }
    }

    fn reset(&mut self, account: Option<Address>) {
        if !self.muted.is_empty() {
            self.revision += 1;
        }
        self.muted.clear();
        self.session_unmuted.clear();
        self.account = account;
        self.load = if account.is_some() {
            Load::Loading
        } else {
            Load::Idle
        };
    }

    fn sorted(&self) -> Vec<String> {
        self.muted
            .iter()
            .map(|address| format!("{address:#x}"))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

#[derive(Deserialize)]
struct MutesResponse {
    data: MutesPage,
}

#[derive(Deserialize)]
struct MutesPage {
    #[serde(default)]
    results: Vec<MutedUser>,
    #[serde(default)]
    total: usize,
}

#[derive(Deserialize)]
struct MutedUser {
    address: Option<String>,
}

fn mutes_url() -> String {
    base_domain::url(Service::SocialApi, "/v1/mutes")
}

/// Where the next page starts, or `None` when the list is complete: an empty page ends it
/// whatever total the server reports, and the offset moves by what the page really held.
fn next_offset(offset: usize, page_len: usize, total: usize) -> Option<usize> {
    let next = offset + page_len;
    (page_len > 0 && next < total).then_some(next)
}

async fn signed_request(
    method: &str,
    url: String,
    body: Option<serde_json::Value>,
    client: &reqwest::Client,
    wallet: &Wallet,
) -> Result<platform::TracedJsonResponse, anyhow::Error> {
    let headers = sign_service_request(method, &url, wallet, "{}".to_owned()).await?;
    let mut request = client
        .request(method.parse()?, url)
        .timeout(std::time::Duration::from_secs(10));
    for (key, value) in headers {
        request = request.header(key, value);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = platform::TracedJsonResponse::send(request).await?;
    if !response.status().is_success() {
        anyhow::bail!("status: {}", response.discard());
    }
    Ok(response)
}

async fn load_mutes(
    client: reqwest::Client,
    wallet: Wallet,
) -> Result<Vec<Address>, anyhow::Error> {
    let mut muted = Vec::new();
    let mut offset = 0;
    for _ in 0..MAX_PAGES {
        let url = format!("{}?limit={PAGE_SIZE}&offset={offset}", mutes_url());
        let page = signed_request("GET", url, None, &client, &wallet)
            .await?
            .json::<MutesResponse>()
            .await?
            .data;
        muted.extend(
            page.results
                .iter()
                .filter_map(|user| user.address.as_deref()?.as_h160()),
        );
        match next_offset(offset, page.results.len(), page.total) {
            Some(next) => offset = next,
            None => break,
        }
    }
    Ok(muted)
}

/// Saves one change. A failure is only logged: the mute already holds for this session, and the
/// next change to that wallet is sent on its own.
fn save(address: Address, muted: bool, client: reqwest::Client, wallet: Wallet) {
    IoTaskPool::get()
        .spawn_compat(async move {
            let method = if muted { "POST" } else { "DELETE" };
            let body = json!({ "muted_address": format!("{address:#x}") });
            match signed_request(method, mutes_url(), Some(body), &client, &wallet).await {
                Ok(response) => {
                    response.discard();
                }
                Err(e) => warn!("nearby mutes: {method} for {address:#x} failed: {e}"),
            }
        })
        .detach();
}

type LoadTask = Task<(Address, Result<Vec<Address>, anyhow::Error>)>;

fn follow_account(
    wallet: Res<Wallet>,
    ipfs: IpfsAssetServer,
    mut mutes: ResMut<NearbyMutes>,
    mut task: Local<Option<LoadTask>>,
) {
    let account = wallet.address();
    if account != mutes.account {
        mutes.reset(account);
        *task = account.map(|account| {
            let client = ipfs.ipfs().client();
            let wallet = wallet.snapshot();
            IoTaskPool::get()
                .spawn_compat(async move { (account, load_mutes(client, wallet).await) })
        });
    }

    let Some((account, result)) = task.as_mut().and_then(|task| task.complete()) else {
        return;
    };
    *task = None;
    // an answer for an account that has since left
    if mutes.account != Some(account) {
        return;
    }
    match result {
        Ok(saved) => {
            info!("nearby mutes: {} saved", saved.len());
            mutes.merge(saved);
            mutes.load = Load::Loaded;
        }
        Err(e) => {
            warn!("nearby mutes: the saved list could not be read: {e}");
            mutes.load = Load::Failed;
        }
    }
}

fn apply(
    mutes: &mut NearbyMutes,
    address: &str,
    muted: bool,
    wallet: &Wallet,
    ipfs: &IpfsAssetServer,
) -> Result<(), String> {
    let address = address
        .as_h160()
        .ok_or_else(|| format!("`{address}` is not a wallet address"))?;
    if mutes.account == Some(address) {
        return Err("Cannot mute yourself".to_owned());
    }
    if mutes.set_muted(address, muted) && mutes.account.is_some() {
        save(address, muted, ipfs.ipfs().client(), wallet.snapshot());
    }
    Ok(())
}

fn respond_to_system_api(
    mut events: EventReader<SystemApi>,
    mut mutes: ResMut<NearbyMutes>,
    mut streams: ResMut<NearbyMuteStreams>,
    wallet: Res<Wallet>,
    ipfs: IpfsAssetServer,
) {
    for event in events.read() {
        match event {
            SystemApi::SetNearbyMuted(address, muted, sender) => {
                sender.send(apply(&mut mutes, address, *muted, &wallet, &ipfs));
            }
            SystemApi::GetNearbyMuteStream(sender) => {
                let _ = sender.send(mutes.sorted());
                streams.0.push(sender.clone());
            }
            _ => (),
        }
    }
}

/// The open streams and the revision they last got. Each reports the whole list on every change,
/// so an account change needs no message of its own.
#[derive(Resource, Default)]
struct NearbyMuteStreams(Vec<RpcStreamSender<Vec<String>>>, u64);

fn publish(mutes: Res<NearbyMutes>, mut streams: ResMut<NearbyMuteStreams>) {
    streams.0.retain(|stream| !stream.is_closed());
    if streams.1 == mutes.revision {
        return;
    }
    streams.1 = mutes.revision;
    let muted = mutes.sorted();
    for stream in &streams.0 {
        let _ = stream.send(muted.clone());
    }
}

/// Report the wallets whose nearby voice is muted, and where the saved list stands.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/nearby_mutes")]
struct NearbyMutesCommand;

fn nearby_mutes_command(mut input: ConsoleCommand<NearbyMutesCommand>, mutes: Res<NearbyMutes>) {
    let Some(Ok(_)) = input.take() else { return };
    let muted = mutes.sorted();
    input.reply_ok(
        json!({
            "account": mutes.account.map(|account| format!("{account:#x}")),
            "load": mutes.load.as_str(),
            "count": muted.len(),
            "muted": muted.into_iter().take(32).collect::<Vec<_>>(),
            "unavailable": { "account": "Not signed in" },
        })
        .to_string(),
    );
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Switch {
    On,
    Off,
}

/// Mute (`on`) or unmute (`off`) a wallet's nearby voice, as the profile menu's Hush and Hear do.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/nearby_mute")]
struct NearbyMuteCommand {
    // not `switch`: the web page builds a JS function with the argument names as its parameters
    state: Switch,
    address: String,
}

fn nearby_mute_command(
    mut input: ConsoleCommand<NearbyMuteCommand>,
    mut mutes: ResMut<NearbyMutes>,
    wallet: Res<Wallet>,
    ipfs: IpfsAssetServer,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let muted = command.state == Switch::On;
    match apply(&mut mutes, &command.address, muted, &wallet, &ipfs) {
        Ok(()) => input.reply_ok(if muted { "Muted" } else { "Unmuted" }),
        Err(e) => input.reply_failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallet(last: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = last;
        Address::from(bytes)
    }

    #[test]
    fn nobody_is_muted_at_first() {
        assert!(!NearbyMutes::default().is_muted(wallet(1)));
    }

    #[test]
    fn a_mute_holds_until_the_unmute() {
        let mut mutes = NearbyMutes::default();
        assert!(mutes.set_muted(wallet(1), true));
        assert!(mutes.is_muted(wallet(1)));
        assert!(mutes.set_muted(wallet(1), false));
        assert!(!mutes.is_muted(wallet(1)));
    }

    #[test]
    fn a_repeated_write_changes_nothing() {
        let mut mutes = NearbyMutes::default();
        mutes.set_muted(wallet(1), true);
        let revision = mutes.revision;
        assert!(!mutes.set_muted(wallet(1), true));
        assert!(!mutes.set_muted(wallet(2), false));
        assert_eq!(mutes.revision, revision);
    }

    #[test]
    fn the_saved_list_only_adds() {
        let mut mutes = NearbyMutes::default();
        mutes.set_muted(wallet(1), true);
        let revision = mutes.revision;
        mutes.merge([wallet(1), wallet(2)]);
        assert!(mutes.is_muted(wallet(1)));
        assert!(mutes.is_muted(wallet(2)));
        assert_eq!(mutes.revision, revision + 1);
    }

    #[test]
    fn an_unmute_before_the_saved_list_arrives_wins() {
        let mut mutes = NearbyMutes::default();
        mutes.set_muted(wallet(1), false);
        mutes.merge([wallet(1)]);
        assert!(!mutes.is_muted(wallet(1)));
    }

    #[test]
    fn a_mute_after_an_unmute_survives_the_saved_list() {
        let mut mutes = NearbyMutes::default();
        mutes.set_muted(wallet(1), false);
        mutes.set_muted(wallet(1), true);
        mutes.merge(Vec::new());
        assert!(mutes.is_muted(wallet(1)));
    }

    #[test]
    fn an_account_change_drops_the_previous_accounts_mutes() {
        let mut mutes = NearbyMutes::default();
        mutes.reset(Some(wallet(9)));
        mutes.set_muted(wallet(1), true);
        mutes.set_muted(wallet(2), false);
        let revision = mutes.revision;

        mutes.reset(Some(wallet(8)));
        assert!(!mutes.is_muted(wallet(1)));
        assert_eq!(mutes.load, Load::Loading);
        assert_eq!(mutes.revision, revision + 1);
        // the previous account's unmute does not shield the wallet from this account's list
        mutes.merge([wallet(2)]);
        assert!(mutes.is_muted(wallet(2)));

        mutes.reset(None);
        assert_eq!(mutes.load, Load::Idle);
    }

    #[test]
    fn addresses_are_listed_lowercase_and_read_in_any_case() {
        let mut mutes = NearbyMutes::default();
        let address = "0xAbCdEf0000000000000000000000000000000001";
        mutes.set_muted(address.as_h160().unwrap(), true);
        assert_eq!(mutes.sorted(), vec![address.to_lowercase()]);
        assert!(mutes.is_muted(address.to_lowercase().as_str().as_h160().unwrap()));
    }

    #[test]
    fn paging_ends_at_the_total_or_an_empty_page() {
        assert_eq!(next_offset(0, 100, 250), Some(100));
        // a short page moves the offset by what it held
        assert_eq!(next_offset(100, 60, 250), Some(160));
        assert_eq!(next_offset(200, 50, 250), None);
        // a total the server overstates
        assert_eq!(next_offset(100, 0, 250), None);
    }

    #[test]
    fn a_page_without_an_address_is_skipped() {
        let page: MutesResponse = serde_json::from_str(
            r#"{"data":{"results":[{"address":"0x0000000000000000000000000000000000000001","muted_at":"x"},{"address":null},{}],"total":3,"limit":100,"offset":0}}"#,
        )
        .unwrap();
        let read: Vec<_> = page
            .data
            .results
            .iter()
            .filter_map(|user| user.address.as_deref()?.as_h160())
            .collect();
        assert_eq!(read, vec![wallet(1)]);
        assert_eq!(page.data.total, 3);
    }
}
