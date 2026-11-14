//! Bounded admission for immediate scene requests. Shared by the trusted web relay and host tests.
use common::rpc::{RpcCall, RpcResultSender, RpcStreamSender};
use dcl_types::{SceneResponse, SceneResponseSender, TrySendError, MAX_RPC_CALLS_PER_TICK};
use std::collections::{HashMap, HashSet, VecDeque};

pub const MAX_IMMEDIATE_IN_FLIGHT: usize = 32;
pub const MAX_RETAINED_IMMEDIATE_BYTES: usize = 80 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ImmediateDispatch {
    Idle,
    Forwarded,
    Blocked,
}

/// Forward one renderer response using the relay's single retained-response slot.
pub fn forward_renderer_response(
    sender: &SceneResponseSender,
    response: SceneResponse,
    credits: &mut u32,
    held: &mut Option<SceneResponse>,
) -> Result<bool, &'static str> {
    let tick = matches!(response, SceneResponse::Ok(..));
    if tick && *credits == 0 {
        *held = Some(response);
        return Ok(false);
    }
    match sender.try_send(response) {
        Ok(()) => {
            if tick {
                *credits -= 1;
            }
            Ok(true)
        }
        Err(TrySendError::Full((_, response))) => {
            *held = Some(response);
            Ok(false)
        }
        Err(_) => Err("renderer response channel closed"),
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Query,
    File,
    Subscription,
}

fn endpoint(call: &RpcCall) -> Result<(u64, Kind), &'static str> {
    match call {
        RpcCall::ReadFile {
            response: RpcResultSender::Remote { id, .. },
            ..
        } => Ok((*id, Kind::File)),
        RpcCall::GetPermissions {
            response: RpcResultSender::Remote { id, .. },
            ..
        }
        | RpcCall::GetUserAccount {
            response: RpcResultSender::Remote { id, .. },
            ..
        }
        | RpcCall::EntityDefinition {
            response: RpcResultSender::Remote { id, .. },
            ..
        }
        | RpcCall::SignLegacyMessage {
            response: RpcResultSender::Remote { id, .. },
            ..
        }
        | RpcCall::SendAsync {
            response: RpcResultSender::Remote { id, .. },
            ..
        }
        | RpcCall::PullLegacyAvatarEvents {
            response: RpcResultSender::Remote { id, .. },
            ..
        } => Ok((*id, Kind::Query)),
        RpcCall::SubscribeLegacyAvatarEvents {
            sender: RpcStreamSender::Remote { id, .. },
        } => Ok((*id, Kind::Subscription)),
        _ => Err("unexpected immediate call or missing reply channel"),
    }
}

struct Pending {
    call: RpcCall,
    id: u64,
    kind: Kind,
}

#[derive(Default)]
pub struct ImmediateRpcWindow {
    admitted: usize,
    admitted_bytes: usize,
    pending: VecDeque<Pending>,
    retained_bytes: usize,
    charges: HashMap<u64, usize>,
    outstanding: HashSet<u64>,
    ids: HashSet<u64>,
    file: Option<u64>,
    subscribed: bool,
}

impl ImmediateRpcWindow {
    pub fn admit(
        &mut self,
        call: RpcCall,
        bytes: usize,
        open: impl Fn(u64) -> bool,
        exists: impl Fn(u64) -> bool,
    ) -> Result<(), &'static str> {
        self.reap_closed(exists);
        let (id, kind) = endpoint(&call)?;
        if !open(id) {
            return Err("unknown immediate channel");
        }
        if self.ids.contains(&id) {
            return Err("duplicate immediate channel");
        }
        let counted = !matches!(kind, Kind::File);
        if counted && self.admitted >= MAX_RPC_CALLS_PER_TICK {
            return Err("too many rpc calls");
        }
        if bytes > MAX_RETAINED_IMMEDIATE_BYTES.saturating_sub(self.admitted_bytes) {
            return Err("too many immediate rpc bytes per tick");
        }
        if self.pending.len() >= MAX_RPC_CALLS_PER_TICK
            || bytes > MAX_RETAINED_IMMEDIATE_BYTES.saturating_sub(self.retained_bytes)
        {
            return Err("immediate request queue full");
        }
        if matches!(kind, Kind::Subscription) {
            if self.subscribed {
                return Err("duplicate avatar subscription");
            }
            self.subscribed = true;
        }
        self.admitted += usize::from(counted);
        self.admitted_bytes += bytes;
        self.retained_bytes += bytes;
        self.charges.insert(id, bytes);
        self.ids.insert(id);
        self.pending.push_back(Pending { call, id, kind });
        Ok(())
    }

    /// A real renderer flush resets admissions, never outstanding reply state.
    pub fn tick(&mut self, queued: usize) -> Result<(), &'static str> {
        if queued > MAX_RPC_CALLS_PER_TICK.saturating_sub(self.admitted) {
            return Err("too many rpc calls");
        }
        self.admitted = 0;
        self.admitted_bytes = 0;
        Ok(())
    }

    fn reap_closed(&mut self, exists: impl Fn(u64) -> bool) {
        self.charges.retain(|id, bytes| {
            if exists(*id) {
                true
            } else {
                self.retained_bytes -= *bytes;
                self.ids.remove(id);
                false
            }
        });
    }

    pub fn next(
        &mut self,
        open: impl Fn(u64) -> bool,
        exists: impl Fn(u64) -> bool,
    ) -> Option<RpcCall> {
        self.reap_closed(&exists);
        self.outstanding.retain(|id| exists(*id));
        if let Some(id) = self.file {
            if exists(id) {
                return None;
            }
            self.file = None;
        }
        loop {
            let next = self.pending.front()?;
            if !open(next.id) {
                let cancelled = self.pending.pop_front()?;
                drop(cancelled);
                continue;
            }
            if self.outstanding.len() >= MAX_IMMEDIATE_IN_FLIGHT {
                return None;
            }
            let next = self.pending.pop_front()?;
            match next.kind {
                Kind::Query => {
                    self.outstanding.insert(next.id);
                }
                Kind::File => self.file = Some(next.id),
                Kind::Subscription => {}
            }
            return Some(next.call);
        }
    }

    /// Keep accepted requests in the bounded FIFO until the renderer accepts them.
    /// A full shared renderer queue neither consumes an admission nor loses a lifetime sender.
    pub fn forward_next(
        &mut self,
        sender: &SceneResponseSender,
        open: impl Fn(u64) -> bool,
        exists: impl Fn(u64) -> bool,
    ) -> Result<ImmediateDispatch, &'static str> {
        let Some(call) = self.next(open, exists) else {
            return Ok(ImmediateDispatch::Idle);
        };
        match sender.try_send(SceneResponse::ImmediateRpcCall(call)) {
            Ok(()) => Ok(ImmediateDispatch::Forwarded),
            Err(TrySendError::Full((_, SceneResponse::ImmediateRpcCall(call)))) => {
                let (id, kind) = endpoint(&call)?;
                self.outstanding.remove(&id);
                if self.file == Some(id) {
                    self.file = None;
                }
                self.pending.push_front(Pending { call, id, kind });
                Ok(ImmediateDispatch::Blocked)
            }
            Err(_) => Err("renderer response channel closed"),
        }
    }

    pub fn file_pending(&self, exists: impl Fn(u64) -> bool) -> bool {
        self.file.is_some_and(exists)
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.retained_bytes = 0;
        self.charges.clear();
        self.outstanding.clear();
        self.ids.clear();
        self.file = None;
    }
}

#[cfg(test)]
#[path = "immediate_rpc_tests.rs"]
mod tests;
