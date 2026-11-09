use bevy::{platform::collections::HashMap, prelude::*};
use common::memory_reclaim::ReclaimTurn;
use std::{
    collections::BTreeMap,
    ops::Bound::{Excluded, Included},
};

struct Pin<A: Asset> {
    handle: Handle<A>,
    frame: u32,
    sequence: u64,
}

/// Cache-owned handles indexed by content identity, with exactly one recency
/// record per pin. Consumers that outlive a lookup keep their own strong handles.
pub(super) struct IdleCache<A: Asset> {
    entries: HashMap<String, Pin<A>>,
    order: BTreeMap<u64, String>,
    sequence: u64,
    cursor: u64,
    scan_end: Option<u64>,
    scan_released: u64,
}

impl<A: Asset> Default for IdleCache<A> {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            order: Default::default(),
            sequence: 0,
            cursor: 0,
            scan_end: None,
            scan_released: 0,
        }
    }
}

impl<A: Asset> IdleCache<A> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get_or_insert_with(
        &mut self,
        key: &str,
        frame: u32,
        load: impl FnOnce() -> Handle<A>,
    ) -> &Handle<A> {
        if !self.entries.contains_key(key) {
            self.entries.insert(
                key.to_owned(),
                Pin {
                    handle: load(),
                    frame,
                    sequence: 0,
                },
            );
        }
        let pin = self.entries.get_mut(key).unwrap();
        let order_key = self
            .order
            .remove(&pin.sequence)
            .unwrap_or_else(|| key.to_owned());
        self.sequence += 1;
        pin.sequence = self.sequence;
        pin.frame = frame;
        self.order.insert(pin.sequence, order_key);
        &pin.handle
    }

    pub fn reclaim(
        &mut self,
        frame: u32,
        turn: &mut ReclaimTurn,
        pending: impl Fn(AssetId<A>) -> bool,
    ) {
        // A finite pass prevents repeated hot touches from stranding older pins.
        let scan_end = *self.scan_end.get_or_insert(self.sequence);
        while turn.inspect() {
            let Some((&sequence, key)) = self
                .order
                .range((Excluded(self.cursor), Included(scan_end)))
                .next()
            else {
                self.cursor = 0;
                self.scan_end = None;
                turn.complete_scan(std::mem::take(&mut self.scan_released));
                break;
            };
            self.cursor = sequence;
            let pin = &self.entries[key];
            // Inclusive three-frame grace, also across FrameCount wraparound.
            if frame.wrapping_sub(pin.frame) <= super::RETAIN_TICKS || pending(pin.handle.id()) {
                continue;
            }
            let key = self.order.remove(&sequence).unwrap();
            self.entries.remove(&key);
            turn.released();
            self.scan_released += 1;
        }
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
