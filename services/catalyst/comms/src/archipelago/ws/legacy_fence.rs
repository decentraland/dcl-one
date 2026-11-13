use super::*;
use crate::archipelago::control_v4::{AssignmentPayload, LaneKey, V4Error, V4Owner};

pub(super) const POLL_INTERVAL: Duration = Duration::from_secs(2);

pub(super) struct LegacyFence {
    last_fence: Option<crate::gatekeeper::admission::cluster::fence::TokenFence>,
}

fn realm() -> LaneKey {
    LaneKey::parse("realm").expect("static realm lane")
}

impl LegacyFence {
    pub fn new() -> Self {
        Self { last_fence: None }
    }

    pub async fn poll(
        &mut self,
        state: &AppState,
        socket: &mut WebSocket,
        link: &PeerLink,
        address: &str,
        session: &str,
    ) -> bool {
        use sqlx::Row;
        let Some(audience) = state.gatekeeper.cluster.control_v4_audience.as_ref() else {
            return true;
        };
        let row = sqlx::query(
            "SELECT owner_epoch, assignment_revision, fencing_token, assignment_json
            FROM social.archipelago_v4_assignments
            WHERE deployment_audience=$1 AND owner_address=$2 AND lane_key='realm'
              AND owner_session=$3 AND updated_at>=clock_timestamp()-interval '90 seconds'",
        )
        .bind(audience)
        .bind(address)
        .bind(session)
        .fetch_optional(&state.gatekeeper.pool)
        .await;
        let assignment = row.ok().flatten().and_then(|row| {
            let owner_epoch: i64 = row.try_get("owner_epoch").ok()?;
            let revision: i64 = row.try_get("assignment_revision").ok()?;
            let token: i64 = row.try_get("fencing_token").ok()?;
            if owner_epoch <= 0 || revision <= 0 || token <= 0 {
                return None;
            }
            let value: Option<serde_json::Value> = row.try_get("assignment_json").ok()?;
            let assignment = serde_json::from_value::<AssignmentPayload>(value?).ok()?;
            let fence = crate::gatekeeper::admission::cluster::fence::TokenFence {
                audience: audience.clone(),
                lane: "realm".into(),
                owner_session: session.into(),
                owner_epoch: owner_epoch as u64,
                assignment_revision: revision as u64,
                fencing_token: token as u64,
            };
            Some((assignment, fence))
        });
        let Some((assignment, token_fence)) = assignment else {
            state.registry.detach_selected_realm(link);
            self.last_fence = None;
            return true;
        };
        let Some(stamp) = assignment.pulse_admission.as_ref() else {
            state.registry.detach_selected_realm(link);
            return true;
        };
        if !state.gatekeeper.verify_island_stamp(stamp).await
            || stamp.wallet != address
            || stamp.session != session
            || crate::rooms::island_room_name(&stamp.cluster_id) != assignment.island_id
        {
            state.registry.detach_selected_realm(link);
            return true;
        }
        let Some(permit) =
            state
                .registry
                .bind_selected_realm(address, link, token_fence.clone(), stamp.clone())
        else {
            state.registry.detach_selected_realm(link);
            return true;
        };
        if self.last_fence.as_ref() == Some(&token_fence) && !link.needs_assignment() {
            return true;
        }
        socket.bind_selected_permit(permit);
        socket.bind_pulse(Some(stamp.clone()));
        socket.bind_island_token_fence(token_fence.clone());
        if !send_packet(
            socket,
            server_packet::Message::IslandChanged(project(assignment)),
            Some(link),
            &state.feed,
        )
        .await
        {
            return false;
        }
        self.last_fence = Some(token_fence);
        true
    }
}

fn project(assignment: AssignmentPayload) -> IslandChangedMessage {
    IslandChangedMessage {
        island_id: assignment.island_id,
        conn_str: assignment.connection_string,
        from_island_id: assignment.from_island_id,
        peers: assignment
            .peers
            .into_iter()
            .filter_map(|(address, value)| position_from_value(value).map(|p| (address, p)))
            .collect(),
        assignment_context: None,
    }
}

pub(super) async fn owns_realm(state: &AppState, owner: &V4Owner) -> Result<bool, V4Error> {
    match tokio::time::timeout(
        AUTHORITY_OPERATION_TIMEOUT,
        state.control_v4.snapshot(owner, &realm()),
    )
    .await
    .unwrap_or(Err(V4Error::AuthorityUnavailable))
    {
        Ok(_) => Ok(true),
        Err(V4Error::AuthorityConflict) => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn position_from_value(
    value: serde_json::Value,
) -> Option<crate::protocol::archipelago::Position> {
    if let Some(array) = value.as_array()
        && array.len() == 3
    {
        return Some(crate::protocol::archipelago::Position {
            x: array.first()?.as_f64()? as f32,
            y: array.get(1)?.as_f64()? as f32,
            z: array.get(2)?.as_f64()? as f32,
        });
    }
    Some(crate::protocol::archipelago::Position {
        x: value.get("x")?.as_f64()? as f32,
        y: value.get("y")?.as_f64()? as f32,
        z: value.get("z")?.as_f64()? as f32,
    })
}
