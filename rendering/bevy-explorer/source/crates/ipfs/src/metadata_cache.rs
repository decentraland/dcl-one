use std::ops::Bound::{Excluded, Included, Unbounded};
use std::sync::atomic::{AtomicU64, Ordering};

use common::memory_reclaim::{ReclaimBudget, ReclaimTurn};

use super::*;

const IDLE_GRACE: Duration = Duration::from_secs(5);
const RELOAD_TIMEOUT: Duration = Duration::from_secs(30);

struct MetadataAccess {
    epoch: Instant,
    nanos: AtomicU64,
}

impl MetadataAccess {
    fn new(epoch: Instant) -> Self {
        Self {
            epoch,
            nanos: AtomicU64::new(0),
        }
    }

    fn touch(&self) {
        let elapsed = self.epoch.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.nanos.fetch_max(elapsed, Ordering::Relaxed);
    }

    fn idle_for(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.epoch)
            .saturating_sub(Duration::from_nanos(self.nanos.load(Ordering::Relaxed)))
    }
}

pub(super) struct RegisteredEntity {
    pub collection: ContentMap,
    pub metadata: Option<String>,
    metadata_evicted: bool,
    metadata_accessed: MetadataAccess,
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
                metadata_accessed: MetadataAccess::new(Instant::now()),
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
            if entry.metadata.is_some() && entry.metadata_accessed.idle_for(now) >= IDLE_GRACE {
                entry.metadata = None;
                entry.metadata_evicted = true;
                retention.resident -= 1;
                retention.scan_released += 1;
                turn.released();
            }
        }
    }

    fn definition(&self, hash: &str) -> Option<(IpfsEntity, String)> {
        let entry = self.entities.get(hash)?;
        Some((
            IpfsEntity {
                collection: entry.collection.clone(),
                metadata: entry.metadata.clone(),
            },
            self.contents_base_url(hash).unwrap_or_default(),
        ))
    }
}

pub(super) fn reclaim_metadata(ipfs: Res<IpfsResource>, mut budget: ResMut<ReclaimBudget>) {
    let Ok(context) = ipfs.context.try_read() else {
        return;
    };
    let entries = context.metadata_retention.resident;
    drop(context);
    let Some(mut turn) = budget.begin("entity_metadata", entries) else {
        return;
    };
    // Async content reads must never make the cleanup schedule wait for this lock.
    let Ok(mut context) = ipfs.context.try_write() else {
        budget.finish_turn(turn, entries);
        return;
    };
    context.reclaim_metadata(Instant::now(), &mut turn);
    budget.finish_turn(turn, context.metadata_retention.resident);
}

pub(super) async fn entity_definition(
    ipfs: &Arc<IpfsIo>,
    hash: &str,
) -> Option<(IpfsEntity, String)> {
    entity_definition_with_timeout(ipfs, hash, RELOAD_TIMEOUT).await
}

async fn entity_definition_with_timeout(
    ipfs: &Arc<IpfsIo>,
    hash: &str,
    timeout: Duration,
) -> Option<(IpfsEntity, String)> {
    match async_std::future::timeout(timeout, reload_definition(ipfs, hash)).await {
        Ok(definition) => definition,
        Err(_) => {
            warn!("entity metadata reload timed out for {hash}");
            ipfs.context.read().await.definition(hash)
        }
    }
}

async fn reload_definition(ipfs: &Arc<IpfsIo>, hash: &str) -> Option<(IpfsEntity, String)> {
    loop {
        let (generation, source) = {
            let context = ipfs.context.read().await;
            let entry = context.entities.get(hash)?;
            entry.metadata_accessed.touch();
            if !entry.metadata_evicted {
                return context.definition(hash);
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
            match reload_metadata(reader.as_mut(), hash).await {
                Ok(metadata) => Ok(metadata),
                Err(error) => {
                    // A rejected disk or service-worker entry must not prevent recovery.
                    // Keep the entity path so the normal download queue retains its owner.
                    warn!("reloading rejected entity metadata for {hash} without cache: {error}");
                    drop(reader);
                    let mut reader = ipfs.read_with_cache(&path, false).await?;
                    reload_metadata(reader.as_mut(), hash).await
                }
            }
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
                    return context.definition(hash);
                }
            };
            entry.metadata = Some(metadata);
            entry.metadata_evicted = false;
            entry.metadata_accessed.touch();
            context.metadata_retention.resident += 1;
        }
        // Read the current routing and metadata together, including concurrent reloads.
        return context.definition(hash);
    }
}

async fn reload_metadata(reader: &mut dyn Reader, hash: &str) -> anyhow::Result<String> {
    let entity = EntityDefinitionLoader
        .load_internal(reader, &(), || hash.to_owned())
        .await?;
    if entity.id != hash {
        return Err(anyhow!("reloaded entity identity does not match {hash}"));
    }
    let metadata = entity
        .metadata
        .ok_or_else(|| anyhow!("reloaded entity has no previously evicted metadata"))?;
    Ok(metadata.to_string())
}

#[cfg(test)]
#[path = "metadata_cache_tests.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "scene_collection_tests.rs"]
mod scene_collection_tests;
