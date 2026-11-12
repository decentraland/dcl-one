use std::ops::Bound::{Excluded, Included, Unbounded};

use common::memory_reclaim::{ReclaimBudget, ReclaimTurn};

use super::*;

const IDLE_GRACE: Duration = Duration::from_secs(5);

pub(super) struct RegisteredEntity {
    pub collection: ContentMap,
    pub metadata: Option<String>,
    metadata_evicted: bool,
    metadata_accessed: Instant,
    generation: u64,
}

#[derive(Default)]
pub(super) struct MetadataRetention {
    generation: u64,
    resident: usize,
    cursor: Option<String>,
    scan_end: Option<String>,
    scan_released: u64,
}

impl IpfsContext {
    pub(super) fn register_entity(&mut self, hash: String, entity: IpfsEntity) {
        self.metadata_retention.generation += 1;
        self.metadata_retention.resident += usize::from(entity.metadata.is_some());
        let previous = self.entities.insert(
            hash,
            RegisteredEntity {
                collection: entity.collection,
                metadata: entity.metadata,
                metadata_evicted: false,
                metadata_accessed: Instant::now(),
                generation: self.metadata_retention.generation,
            },
        );
        if previous.is_some_and(|entry| entry.metadata.is_some()) {
            self.metadata_retention.resident -= 1;
        }
    }

    fn reclaim_metadata(&mut self, now: Instant, turn: &mut ReclaimTurn) {
        let retention = &mut self.metadata_retention;
        if retention.scan_end.is_none() {
            retention.scan_end = self.entities.last_key_value().map(|(hash, _)| hash.clone());
        }
        while turn.inspect() {
            let next = retention.scan_end.as_deref().and_then(|end| {
                self.entities
                    .range_mut::<str, _>((
                        retention.cursor.as_deref().map_or(Unbounded, Excluded),
                        Included(end),
                    ))
                    .next()
            });
            let Some((hash, entry)) = next else {
                retention.cursor = None;
                retention.scan_end = None;
                turn.complete_scan(std::mem::take(&mut retention.scan_released));
                break;
            };
            retention.cursor = Some(hash.clone());
            if entry.metadata.is_some()
                && now.saturating_duration_since(entry.metadata_accessed) >= IDLE_GRACE
            {
                entry.metadata = None;
                entry.metadata_evicted = true;
                retention.resident -= 1;
                retention.scan_released += 1;
                turn.released();
            }
        }
    }
}

pub(super) fn reclaim_metadata(ipfs: Res<IpfsResource>, mut budget: ResMut<ReclaimBudget>) {
    // Async content reads must never make the cleanup schedule wait for this lock.
    let Ok(mut context) = ipfs.context.try_write() else {
        return;
    };
    if let Some(mut turn) = budget.begin("entity_metadata", context.metadata_retention.resident) {
        context.reclaim_metadata(Instant::now(), &mut turn);
        budget.finish_turn(turn, context.metadata_retention.resident);
    }
}

pub(super) async fn entity_definition(
    ipfs: &Arc<IpfsIo>,
    hash: &str,
) -> Option<(IpfsEntity, String)> {
    loop {
        let (generation, source) = {
            let mut context = ipfs.context.write().await;
            let entry = context.entities.get_mut(hash)?;
            entry.metadata_accessed = Instant::now();
            if !entry.metadata_evicted {
                let definition = IpfsEntity {
                    collection: entry.collection.clone(),
                    metadata: entry.metadata.clone(),
                };
                return Some((
                    definition,
                    context.contents_base_url(hash).unwrap_or_default(),
                ));
            }
            (entry.generation, context.contents_base_url(hash))
        };

        let mut path = IpfsPath::new(IpfsType::Entity {
            hash: hash.to_owned(),
            ext: EntityDefinition::ext().to_owned(),
        });
        if let Some(source) = &source {
            path = path.with_keyvalue(IpfsKey::BaseUrl, source.clone());
        }
        let path = PathBuf::from(&path);
        let loaded = async {
            let mut reader = ipfs.read(&path).await?;
            let entity = EntityDefinitionLoader
                .load_internal(&mut reader, &(), || hash.to_owned())
                .await?;
            Ok::<_, anyhow::Error>(entity.metadata.map(|metadata| metadata.to_string()))
        }
        .await;

        let mut context = ipfs.context.write().await;
        if context.contents_base_url(hash) != source {
            continue;
        }
        let entry = context.entities.get_mut(hash)?;
        // A newer registration may have different content or a different source URL.
        if entry.generation != generation {
            continue;
        }
        if entry.metadata_evicted {
            let metadata = match loaded {
                Ok(metadata) => metadata,
                Err(error) => {
                    warn!("failed to reload entity metadata for {hash}: {error}");
                    return None;
                }
            };
            let resident = metadata.is_some();
            entry.metadata = metadata;
            entry.metadata_evicted = false;
            entry.metadata_accessed = Instant::now();
            context.metadata_retention.resident += usize::from(resident);
        }
        // Read the current routing and metadata together, including concurrent reloads.
        let entry = context.entities.get(hash)?;
        return Some((
            IpfsEntity {
                collection: entry.collection.clone(),
                metadata: entry.metadata.clone(),
            },
            context.contents_base_url(hash).unwrap_or_default(),
        ));
    }
}

#[cfg(test)]
#[path = "metadata_cache_tests.rs"]
mod tests;
