use bevy::platform::collections::HashMap;
use dcl_component::{DclReader, SceneEntityId};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const SET_SIZE: usize = 100;

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct CrdtGOEntry {
    // timestamp: SceneCrdtTimestamp,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CrdtGOState(pub HashMap<SceneEntityId, VecDeque<CrdtGOEntry>>);

impl CrdtGOState {
    /// Adds a payload once per entity while its exact bytes remain in this set
    /// ([ADR-117](https://adr.decentraland.org/adr/ADR-117)).
    ///
    /// Scene stores discard their grow-only history on `CrdtStore::take_updates`.
    /// The inspector's filtered store retains it across scene ticks. At capacity,
    /// a distinct append evicts the oldest value.
    pub fn append(
        &mut self,
        entity: SceneEntityId,
        // timestamp: SceneCrdtTimestamp,
        new_data: &mut DclReader,
    ) -> bool {
        let queue = self.0.entry(entity).or_default();
        if queue.iter().any(|entry| entry.data == new_data.as_slice()) {
            return false;
        }

        let new_slot = if queue.len() == SET_SIZE {
            let mut slot = queue.pop_front().unwrap();
            slot.data.clear();
            slot.data.extend_from_slice(new_data.as_slice());
            // slot.timestamp = timestamp;
            slot
        } else {
            CrdtGOEntry {
                // timestamp,
                data: new_data.as_slice().to_vec(),
            }
        };
        queue.push_back(new_slot);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTITY: SceneEntityId = SceneEntityId {
        id: 1,
        generation: 0,
    };

    #[test]
    fn repeated_values_are_retained_once() {
        let mut state = CrdtGOState::default();
        for value in [b"event".as_slice(), b"".as_slice()] {
            assert!(state.append(ENTITY, &mut DclReader::new(value)));
            let duplicate = value.to_vec();
            assert!(!state.append(ENTITY, &mut DclReader::new(&duplicate)));
        }
        let values: Vec<_> = state.0[&ENTITY]
            .iter()
            .map(|entry| entry.data.as_slice())
            .collect();
        assert_eq!(values, [b"event".as_slice(), b"".as_slice()]);
    }

    #[test]
    fn repeated_value_after_distinct_value_keeps_first_occurrence_order() {
        let mut state = CrdtGOState::default();
        assert!(state.append(ENTITY, &mut DclReader::new(b"A")));
        assert!(state.append(ENTITY, &mut DclReader::new(b"B")));
        assert!(!state.append(ENTITY, &mut DclReader::new(b"A")));

        let values: Vec<_> = state.0[&ENTITY]
            .iter()
            .map(|entry| entry.data.as_slice())
            .collect();
        assert_eq!(values, [b"A".as_slice(), b"B".as_slice()]);
    }

    #[test]
    fn duplicates_at_capacity_do_not_evict_or_refresh_values() {
        let mut state = CrdtGOState::default();
        for value in 0..SET_SIZE as u32 {
            assert!(state.append(ENTITY, &mut DclReader::new(&value.to_le_bytes())));
        }

        for value in [0_u32, 50, 99, 0] {
            assert!(!state.append(ENTITY, &mut DclReader::new(&value.to_le_bytes())));
        }
        assert!(state.append(ENTITY, &mut DclReader::new(&100_u32.to_le_bytes())));

        let values: Vec<_> = state.0[&ENTITY]
            .iter()
            .map(|entry| u32::from_le_bytes(entry.data.as_slice().try_into().unwrap()))
            .collect();
        assert_eq!(values, (1..=100).collect::<Vec<_>>());
    }

    #[test]
    fn identical_values_on_other_entities_and_generations_are_independent() {
        let mut state = CrdtGOState::default();
        for entity in [
            ENTITY,
            SceneEntityId {
                id: 2,
                generation: 0,
            },
            SceneEntityId {
                id: 1,
                generation: 1,
            },
        ] {
            assert!(state.append(entity, &mut DclReader::new(b"event")));
            assert!(!state.append(entity, &mut DclReader::new(b"event")));
            assert_eq!(state.0[&entity].len(), 1);
        }
    }

    #[test]
    fn distinct_values_with_shared_prefixes_are_preserved() {
        let mut state = CrdtGOState::default();
        for value in [b"a".as_slice(), b"ab".as_slice(), b"ac".as_slice()] {
            assert!(state.append(ENTITY, &mut DclReader::new(value)));
        }
        assert_eq!(state.0[&ENTITY].len(), 3);
    }
}
