use bytes::Bytes;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PacketFlags(pub u8);

impl PacketFlags {
    pub const RELIABLE: u8 = 1 << 0;

    pub const UNSEQUENCED: u8 = 1 << 1;

    pub const UNRELIABLE_FRAGMENT: u8 = 1 << 3;

    pub const UNTHROTTLED: u8 = 1 << 5;

    pub fn is_reliable(self) -> bool {
        self.0 & Self::RELIABLE != 0
    }
    pub fn is_unthrottled(self) -> bool {
        self.0 & Self::UNTHROTTLED != 0
    }
    pub fn is_unsequenced(self) -> bool {
        self.0 & Self::UNSEQUENCED != 0
    }
}

#[derive(Debug, Clone)]
pub struct Packet {
    pub channel: u8,
    pub flags: PacketFlags,
    pub data: Bytes,
}

impl Packet {
    pub fn reliable(channel: u8, data: impl Into<Bytes>) -> Self {
        Self {
            channel,
            flags: PacketFlags(PacketFlags::RELIABLE),
            data: data.into(),
        }
    }
    pub fn unreliable(channel: u8, data: impl Into<Bytes>) -> Self {
        Self {
            channel,
            flags: PacketFlags::default(),
            data: data.into(),
        }
    }
    pub fn unthrottled(channel: u8, data: impl Into<Bytes>) -> Self {
        Self {
            channel,
            flags: PacketFlags(PacketFlags::UNTHROTTLED),
            data: data.into(),
        }
    }
    pub fn unsequenced(channel: u8, data: impl Into<Bytes>) -> Self {
        Self {
            channel,
            flags: PacketFlags(PacketFlags::UNSEQUENCED),
            data: data.into(),
        }
    }

    pub fn from_enet(channel: u8, raw: &rusty_enet::Packet) -> Self {
        let flags = match raw.kind() {
            rusty_enet::PacketKind::Reliable => PacketFlags(PacketFlags::RELIABLE),
            rusty_enet::PacketKind::Unreliable { sequenced: false }
            | rusty_enet::PacketKind::AlwaysUnreliable { sequenced: false } => {
                PacketFlags(PacketFlags::UNSEQUENCED)
            }
            _ => PacketFlags::default(),
        };
        Self {
            channel,
            flags,
            data: Bytes::copy_from_slice(raw.data()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_throttle_bypass_is_explicit_and_orthogonal_to_packet_kind() {
        let ordinary = Packet::unreliable(1, vec![1]);
        let bypass = Packet::unthrottled(1, vec![1]);
        assert!(!ordinary.flags.is_unthrottled());
        assert!(bypass.flags.is_unthrottled());
        assert!(!bypass.flags.is_reliable());
        assert!(!bypass.flags.is_unsequenced());
        assert!(!Packet::reliable(0, vec![1]).flags.is_unthrottled());
        assert!(!Packet::unsequenced(2, vec![1]).flags.is_unthrottled());
    }
}
