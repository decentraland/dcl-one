use crate::{CommsMessageType, MAX_COMMS_MESSAGE_BYTES};

const MAX_TOPIC_BYTES: usize = 512;
const TOPIC_LENGTH_BYTES: usize = 2;

pub fn valid_topic(topic: &str) -> bool {
    topic.len() <= MAX_TOPIC_BYTES
}

pub fn encode_topic(topic: &str, data: &str) -> Option<Vec<u8>> {
    if !valid_topic(topic) || data.is_empty() {
        return None;
    }
    let header_len = TOPIC_LENGTH_BYTES + topic.len();
    if data.len() > MAX_COMMS_MESSAGE_BYTES - header_len {
        return None;
    }

    let mut frame = Vec::with_capacity(1 + header_len + data.len());
    frame.push(CommsMessageType::Topic as u8);
    frame.extend_from_slice(&(topic.len() as u16).to_le_bytes());
    frame.extend_from_slice(topic.as_bytes());
    frame.extend_from_slice(data.as_bytes());
    Some(frame)
}

pub fn decode_topic(payload: &[u8]) -> Option<(&str, &str)> {
    if payload.len() > MAX_COMMS_MESSAGE_BYTES {
        return None;
    }
    let (header, body) = payload.split_at_checked(TOPIC_LENGTH_BYTES)?;
    let topic_len = usize::from(u16::from_le_bytes(header.try_into().ok()?));
    if topic_len > MAX_TOPIC_BYTES {
        return None;
    }
    let (topic, data) = body.split_at_checked(topic_len)?;
    if data.is_empty() {
        return None;
    }
    Some((
        std::str::from_utf8(topic).ok()?,
        std::str::from_utf8(data).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_frame_uses_a_utf8_byte_count() {
        let packet = encode_topic("café", "ok").unwrap();
        assert_eq!(packet, b"\x03\x05\x00caf\xc3\xa9ok");
        assert_eq!(decode_topic(&packet[1..]), Some(("café", "ok")));
    }

    #[test]
    fn topic_and_data_share_the_bevy_payload_cap() {
        let topic = "é".repeat(MAX_TOPIC_BYTES / 2);
        let data = "x".repeat(MAX_COMMS_MESSAGE_BYTES - TOPIC_LENGTH_BYTES - topic.len());
        let packet = encode_topic(&topic, &data).unwrap();
        assert_eq!(packet.len(), MAX_COMMS_MESSAGE_BYTES + 1);
        assert_eq!(
            decode_topic(&packet[1..]),
            Some((topic.as_str(), data.as_str()))
        );
        assert!(encode_topic(&topic, &(data + "x")).is_none());
        assert!(encode_topic(&(topic + "x"), "ok").is_none());
    }

    #[test]
    fn incomplete_and_non_utf8_topics_are_rejected() {
        assert!(decode_topic(&[1, 0, b'a']).is_none());
        assert!(decode_topic(&[2, 0, b'a']).is_none());
        assert!(decode_topic(&[1, 0, 0xff, b'a']).is_none());
        assert!(decode_topic(&[1, 0, b'a', 0xff]).is_none());
    }
}
