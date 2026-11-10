use std::borrow::Cow;

use dcl_component::SceneComponentId;

const CRDT_BATCH: u8 = 1;
const CRDT_STATE_RESPONSE: u8 = 3;

const PUT_COMPONENT: u32 = 1;
const DELETE_COMPONENT: u32 = 2;
const DELETE_ENTITY: u32 = 3;
const APPEND_VALUE: u32 = 4;
const PUT_COMPONENT_NETWORK: u32 = 5;
const DELETE_COMPONENT_NETWORK: u32 = 6;
const DELETE_ENTITY_NETWORK: u32 = 7;
const AUTHORITATIVE_PUT_COMPONENT: u32 = 8;

const VIDEO_CONTROL_STATE: SceneComponentId = SceneComponentId(2_092_194_694);

/// Filters SDK CRDT traffic inside a binary scene message. Other binary tags are opaque.
/// A malformed CRDT envelope or record drops the whole packet.
pub fn filter_scene_crdt(payload: &[u8], trusted: bool) -> Option<Cow<'_, [u8]>> {
    let body_offset = match payload.first().copied() {
        Some(CRDT_BATCH) => 1,
        Some(CRDT_STATE_RESPONSE) => 2 + usize::from(*payload.get(1)?),
        _ => return Some(Cow::Borrowed(payload)),
    };
    let mut remaining = payload.get(body_offset..)?;
    let mut filtered = None;
    let mut copy_start = 0;
    while !remaining.is_empty() {
        let declared = read_word(remaining, 0)? as usize;
        // A legacy network delete includes two body words despite its declared length.
        let legacy_delete =
            declared == 12 && read_word(remaining, 4) == Some(DELETE_ENTITY_NETWORK);
        let length = if legacy_delete { 16 } else { declared };
        let record = remaining.get(..length)?;
        let allowed = record_is_allowed(record, trusted)?;
        if !allowed || legacy_delete {
            let record_start = payload.len() - remaining.len();
            let filtered = filtered.get_or_insert_with(|| Vec::with_capacity(payload.len()));
            filtered.extend_from_slice(&payload[copy_start..record_start]);
            if allowed {
                filtered.extend_from_slice(&16_u32.to_le_bytes());
                filtered.extend_from_slice(&record[4..]);
            }
            copy_start = record_start + length;
        }
        remaining = &remaining[length..];
    }

    if let Some(mut filtered) = filtered {
        filtered.extend_from_slice(&payload[copy_start..]);
        Some(Cow::Owned(filtered))
    } else {
        Some(Cow::Borrowed(payload))
    }
}

fn record_is_allowed(record: &[u8], trusted: bool) -> Option<bool> {
    let (header_len, has_data) = match read_word(record, 4)? {
        PUT_COMPONENT | APPEND_VALUE | AUTHORITATIVE_PUT_COMPONENT => (24, true),
        PUT_COMPONENT_NETWORK => (28, true),
        DELETE_COMPONENT => (20, false),
        DELETE_COMPONENT_NETWORK => (24, false),
        DELETE_ENTITY => return (record.len() == 12).then_some(true),
        DELETE_ENTITY_NETWORK => return (record.len() == 16).then_some(true),
        // Unknown kinds advance by the validated record length.
        _ => return Some(true),
    };

    let data_len = if has_data {
        read_word(record, header_len - 4)? as usize
    } else {
        0
    };
    if record.len().checked_sub(header_len)? != data_len {
        return None;
    }
    let component = SceneComponentId(read_word(record, 12)?);
    let local_only = matches!(
        component,
        VIDEO_CONTROL_STATE
            | SceneComponentId::PHYSICS_COMBINED_IMPULSE
            | SceneComponentId::PHYSICS_COMBINED_FORCE
    );
    Some(!local_only && (trusted || component != SceneComponentId::VIDEO_PLAYER))
}

fn read_word(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_component::SceneComponentId;

    const ORDINARY_COMPONENT: u32 = 1000;
    const VIDEO_CONTROL_STATE_ID: u32 = 2092194694;

    fn record(kind: u32, fields: &[u32], data: &[u8]) -> Vec<u8> {
        let length = (8 + fields.len() * 4 + data.len()) as u32;
        let mut record = length.to_le_bytes().to_vec();
        record.extend(kind.to_le_bytes());
        for field in fields {
            record.extend(field.to_le_bytes());
        }
        record.extend(data);
        record
    }

    fn packet(prefix: &[u8], records: &[&[u8]]) -> Vec<u8> {
        let mut packet = prefix.to_vec();
        for record in records {
            packet.extend_from_slice(record);
        }
        packet
    }

    #[test]
    fn legacy_network_entity_delete_keeps_the_next_record_aligned() {
        let corrected = record(DELETE_ENTITY_NETWORK, &[1, 2], b"");
        let mut legacy = corrected.clone();
        legacy[..4].copy_from_slice(&12_u32.to_le_bytes());
        let next = record(PUT_COMPONENT, &[1, ORDINARY_COMPONENT, 2, 2], b"ok");

        let received = packet(&[CRDT_BATCH], &[&legacy, &next]);
        assert_eq!(
            filter_scene_crdt(&received, false).as_deref(),
            Some(packet(&[CRDT_BATCH], &[&corrected, &next]).as_slice()),
        );
    }

    #[test]
    fn batch_preserves_normal_records_and_limits_video_to_trusted_senders() {
        let first = record(5, &[1, ORDINARY_COMPONENT, 1, 2, 2], b"ok");
        let video = record(5, &[1, SceneComponentId::VIDEO_PLAYER.0, 1, 2, 0], b"");
        let force = record(
            6,
            &[1, SceneComponentId::PHYSICS_COMBINED_FORCE.0, 1, 2],
            b"",
        );
        let last = record(7, &[1, 2], b"");
        let payload = packet(&[1], &[&first, &video, &force, &last]);

        assert_eq!(
            filter_scene_crdt(&payload, false).unwrap().as_ref(),
            packet(&[1], &[&first, &last])
        );
        assert_eq!(
            filter_scene_crdt(&payload, true).unwrap().as_ref(),
            packet(&[1], &[&first, &video, &last])
        );
    }

    #[test]
    fn state_response_keeps_the_destination_and_filters_components() {
        let destination = [3, 9, 5, 0, 0, 0, b'0', b'x', b'1', b'2', b'3'];
        let video_state = record(5, &[1, VIDEO_CONTROL_STATE_ID, 1, 2, 0], b"");
        let normal = record(5, &[1, ORDINARY_COMPONENT, 1, 2, 0], b"");
        let payload = packet(&destination, &[&video_state, &normal]);
        assert_eq!(
            filter_scene_crdt(&payload, true).unwrap().as_ref(),
            packet(&destination, &[&normal])
        );

        let all_filtered = packet(&destination, &[&video_state]);
        assert_eq!(
            filter_scene_crdt(&all_filtered, true).unwrap().as_ref(),
            destination
        );
    }

    #[test]
    fn local_only_components_are_omitted_from_all_component_operations() {
        let component = SceneComponentId::PHYSICS_COMBINED_IMPULSE.0;
        let records = [
            record(1, &[1, component, 2, 0], b""),
            record(2, &[1, component, 2], b""),
            record(4, &[1, component, 2, 0], b""),
            record(5, &[1, component, 2, 3, 0], b""),
            record(6, &[1, component, 2, 3], b""),
            record(8, &[1, component, 2, 0], b""),
        ];
        let records: Vec<_> = records.iter().map(Vec::as_slice).collect();
        let payload = packet(&[1], &records);
        assert_eq!(filter_scene_crdt(&payload, true).unwrap().as_ref(), [1]);
    }

    #[test]
    fn preserves_requests_and_opaque_binary_without_copying() {
        for payload in [&[][..], &[2], &[4, 1, 3]] {
            let Some(Cow::Borrowed(result)) = filter_scene_crdt(payload, false) else {
                panic!("expected opaque data to be borrowed");
            };
            assert_eq!(result, payload);
        }

        let delete_entity = record(3, &[1], b"");
        let payload = packet(&[1], &[&delete_entity]);
        assert!(matches!(
            filter_scene_crdt(&payload, false),
            Some(Cow::Borrowed(_))
        ));
    }

    #[test]
    fn preserves_framed_future_operations_without_skipping_the_rest_of_the_batch() {
        let future = record(10, &[1], b"");
        let video = record(2, &[1, SceneComponentId::VIDEO_PLAYER.0, 2], b"");
        let payload = packet(&[1], &[&future, &video]);

        assert_eq!(
            filter_scene_crdt(&payload, false).unwrap().as_ref(),
            packet(&[1], &[&future])
        );
    }

    #[test]
    fn rejects_incomplete_crdt_envelopes_and_records() {
        assert!(filter_scene_crdt(&[3, 2, b'a'], false).is_none());

        let normal = record(1, &[1, ORDINARY_COMPONENT, 2, 2], b"ok");
        let truncated = packet(&[1], &[&normal[..normal.len() - 1]]);
        assert!(filter_scene_crdt(&truncated, false).is_none());

        let body_size_mismatch = record(1, &[1, ORDINARY_COMPONENT, 2, 3], b"ok");
        let payload = packet(&[1], &[&body_size_mismatch]);
        assert!(filter_scene_crdt(&payload, false).is_none());
    }
}
