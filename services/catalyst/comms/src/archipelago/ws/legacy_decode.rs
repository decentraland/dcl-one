//! Decode only the legacy socket vocabulary; unknown fields cannot select a control route.
use crate::protocol::archipelago::{
    Position,
    archipelago::{
        ChallengeRequestMessage, ClientPacket, Heartbeat, SignedChallengeMessage, client_packet,
    },
};

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ()> {
        let end = self.pos.checked_add(n).ok_or(())?;
        let bytes = self.bytes.get(self.pos..end).ok_or(())?;
        self.pos = end;
        Ok(bytes)
    }
    fn uint32(&mut self) -> Result<u32, ()> {
        let mut value = 0;
        for i in 0..5 {
            let byte = self.take(1)?[0];
            value |= u32::from(byte & if i == 4 { 15 } else { 127 }) << (7 * i);
            if byte < 128 {
                return Ok(value);
            }
        }
        self.take(5)?;
        Ok(value)
    }
    fn end(&mut self) -> Result<usize, ()> {
        let length = self.uint32()? as usize;
        self.pos.checked_add(length).ok_or(())
    }
    fn string(&mut self) -> Result<String, ()> {
        let end = self.end()?.min(self.bytes.len());
        let text = String::from_utf8_lossy(self.bytes.get(self.pos..end).ok_or(())?).into_owned();
        self.pos = end;
        Ok(text)
    }
    fn skip(&mut self, wire: u32) -> Result<(), ()> {
        // Group nesting consumes input iteratively, including unmatched field numbers.
        let mut groups = usize::from(wire == 3);
        if groups == 0 {
            return self.skip_scalar(wire);
        }
        while groups > 0 {
            let tag = self.uint32()?;
            match tag & 7 {
                3 => groups += 1,
                4 => groups -= 1,
                kind => self.skip_scalar(kind)?,
            }
        }
        Ok(())
    }
    fn skip_scalar(&mut self, wire: u32) -> Result<(), ()> {
        match wire {
            0 => while self.take(1)?[0] & 128 != 0 {},
            1 => {
                self.take(8)?;
            }
            2 => {
                let length = self.uint32()? as usize;
                self.take(length)?;
            }
            5 => {
                self.take(4)?;
            }
            _ => return Err(()),
        }
        Ok(())
    }
    fn auth(&mut self, end: usize) -> Result<String, ()> {
        let mut text = String::new();
        while self.pos < end {
            let tag = self.uint32()?;
            if tag == 10 {
                text = self.string()?;
            } else if tag == 0 || tag & 7 == 4 {
                break;
            } else {
                self.skip(tag & 7)?;
            }
        }
        Ok(text)
    }
    fn position(&mut self, end: usize) -> Result<Position, ()> {
        let mut position = Position::default();
        while self.pos < end {
            let tag = self.uint32()?;
            if matches!(tag, 13 | 21 | 29) {
                let value = f32::from_le_bytes(self.take(4)?.try_into().map_err(|_| ())?);
                match tag {
                    13 => position.x = value,
                    21 => position.y = value,
                    _ => position.z = value,
                }
            } else if tag == 0 || tag & 7 == 4 {
                break;
            } else {
                self.skip(tag & 7)?;
            }
        }
        Ok(position)
    }
    fn heartbeat(&mut self, end: usize) -> Result<Heartbeat, ()> {
        let mut heartbeat = Heartbeat::default();
        while self.pos < end {
            let tag = self.uint32()?;
            match tag {
                10 => {
                    let end = self.end()?;
                    heartbeat.position = Some(self.position(end)?);
                }
                18 => heartbeat.desired_room = Some(self.string()?),
                _ if tag == 0 || tag & 7 == 4 => break,
                _ => self.skip(tag & 7)?,
            }
        }
        Ok(heartbeat)
    }
}
pub(super) fn decode(bytes: &[u8]) -> Result<ClientPacket, ()> {
    let mut reader = Reader { bytes, pos: 0 };
    let mut packet = ClientPacket::default();
    while reader.pos < bytes.len() {
        let tag = reader.uint32()?;
        packet.message = match tag {
            10 => {
                let end = reader.end()?;
                Some(client_packet::Message::ChallengeRequest(
                    ChallengeRequestMessage {
                        address: reader.auth(end)?,
                    },
                ))
            }
            18 => {
                let end = reader.end()?;
                Some(client_packet::Message::SignedChallenge(
                    SignedChallengeMessage {
                        auth_chain_json: reader.auth(end)?,
                    },
                ))
            }
            26 => {
                let end = reader.end()?;
                Some(client_packet::Message::Heartbeat(reader.heartbeat(end)?))
            }
            _ if tag == 0 || tag & 7 == 4 => break,
            _ => {
                reader.skip(tag & 7)?;
                continue;
            }
        };
    }
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::archipelago::archipelago::client_packet;
    #[test]
    fn locked_buffer_decoder_differential_corpus() {
        use serde_json::{Value, json};
        let corpus: Value = serde_json::from_str(include_str!("legacy-fixtures.json")).unwrap();
        for row in corpus["cases"].as_array().unwrap() {
            let bytes: Vec<u8> = serde_json::from_value(row["bytes"].clone()).unwrap();
            let actual = match decode(&bytes) {
                Err(()) => json!({"error": true}),
                Ok(ClientPacket { message: None, .. }) => Value::Null,
                Ok(ClientPacket {
                    message: Some(message),
                    ..
                }) => match message {
                    client_packet::Message::ChallengeRequest(m) => {
                        json!({"arm": "challengeRequest", "text": m.address})
                    }
                    client_packet::Message::SignedChallenge(m) => {
                        json!({"arm": "signedChallenge", "text": m.auth_chain_json})
                    }
                    client_packet::Message::Heartbeat(m) => {
                        json!({"arm": "heartbeat", "position": m.position.map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]), "desiredRoom": m.desired_room})
                    }
                    _ => panic!("extended arm selected"),
                },
            };
            assert_eq!(actual, row["expected"], "bytes={bytes:?}");
        }
    }

    #[test]
    fn repeated_legacy_messages_replace_and_unknown_arms_do_not_select_routes() {
        let heartbeat = [0x1a, 7, 0x0a, 5, 0x0d, 0, 0, 0x80, 0x3f];
        for suffix in [&[0x1a, 0][..], &[0x22, 0], &[0x2a, 0], &[0], &[0x1c]] {
            let bytes = [heartbeat.as_slice(), suffix].concat();
            let packet = decode(&bytes).unwrap();
            let Some(client_packet::Message::Heartbeat(hb)) = packet.message else {
                panic!("legacy heartbeat lost")
            };
            if suffix == [0x1a, 0] {
                assert!(hb.position.is_none());
            } else {
                assert_eq!(hb.position.unwrap().x, 1.);
            }
        }
        let packet = decode(&[0x1a, 9, 0x0a, 5, 0x0d, 0, 0, 0x80, 0x3f, 0x0a, 0]).unwrap();
        let Some(client_packet::Message::Heartbeat(hb)) = packet.message else {
            panic!()
        };
        assert_eq!(hb.position.unwrap().x, 0.);
        assert!(decode(&[0x18, 0]).unwrap().message.is_none());
        assert!(decode(&[0]).unwrap().message.is_none());
    }
}
