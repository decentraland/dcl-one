//! Bounded source-scoped selection discovery. Entries are hints, never admission proofs.
use crate::interfaces::PulseGenerationBinding;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_VIEW_ENTRIES: usize = 49_146;
pub const MAX_VIEW_BYTES: usize = 8 * 1024 * 1024;
pub const VIEW_PAGE_ENTRIES: usize = 256;
pub const VIEW_LIFETIME: std::time::Duration = std::time::Duration::from_secs(30);

fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
fn token(value: &str) -> bool {
    super::wallet_token(value).as_deref() == Some(value)
}
fn encode<T: Serialize>(value: &T) -> Option<Vec<u8>> {
    let bytes = serde_json::to_vec(value).ok()?;
    (bytes.len() <= super::cluster::MAX_REPLY_BYTES).then_some(bytes)
}
fn digest(entries: &[SelectionTarget]) -> Option<String> {
    let bytes = serde_json::to_vec(entries).ok()?;
    (bytes.len() <= MAX_VIEW_BYTES).then(|| hex::encode(Sha256::digest(bytes)))
}
pub fn source_subject(binding: &PulseGenerationBinding, suffix: &str) -> String {
    format!(
        "pulse.{}.{}",
        binding.source_incarnation.replace('-', ""),
        suffix
    )
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionTarget {
    pub wallet: String,
    pub session: String,
    pub assignment_version: u64,
}
impl SelectionTarget {
    pub fn valid(&self) -> bool {
        token(&self.wallet) && token(&self.session) && self.assignment_version > 0
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FencedLookup {
    pub generation: PulseGenerationBinding,
    pub wallet: String,
    pub session: String,
}
impl FencedLookup {
    pub fn subject(&self) -> String {
        source_subject(&self.generation, "selection_lookup")
    }
    pub fn encode(&self) -> Option<Vec<u8>> {
        (self.generation.valid() && token(&self.wallet) && token(&self.session))
            .then(|| encode(self))
            .flatten()
    }
    pub fn decode(subject: &str, bytes: &[u8]) -> Option<Self> {
        if bytes.len() > super::cluster::MAX_REPLY_BYTES {
            return None;
        }
        let value: Self = serde_json::from_slice(bytes).ok()?;
        (value.subject() == subject && value.encode().is_some()).then_some(value)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewRequest {
    pub generation: PulseGenerationBinding,
    pub request_id: String,
    pub view_id: Option<String>,
    pub offset: usize,
}
impl ViewRequest {
    pub fn subject(&self) -> String {
        source_subject(&self.generation, "selection_view")
    }
    pub fn valid(&self) -> bool {
        self.generation.valid()
            && uuid(&self.request_id)
            && self.offset <= MAX_VIEW_ENTRIES
            && self
                .view_id
                .as_ref()
                .map_or(self.offset == 0, |id| uuid(id))
    }
    pub fn encode(&self) -> Option<Vec<u8>> {
        self.valid().then(|| encode(self)).flatten()
    }
    pub fn decode(subject: &str, bytes: &[u8]) -> Option<Self> {
        if bytes.len() > super::cluster::MAX_REPLY_BYTES {
            return None;
        }
        let value: Self = serde_json::from_slice(bytes).ok()?;
        (value.valid() && value.subject() == subject).then_some(value)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewPage {
    pub generation: PulseGenerationBinding,
    pub request_id: String,
    pub view_id: String,
    pub pass: u64,
    pub total: usize,
    pub offset: usize,
    pub digest: String,
    pub entries: Vec<SelectionTarget>,
}
impl ViewPage {
    pub fn encode(&self) -> Option<Vec<u8>> {
        encode(self)
    }
    pub fn decode(request: &ViewRequest, bytes: &[u8]) -> Option<Self> {
        if bytes.len() > super::cluster::MAX_REPLY_BYTES {
            return None;
        }
        let page: Self = serde_json::from_slice(bytes).ok()?;
        (request.valid()
            && page.generation == request.generation
            && page.request_id == request.request_id
            && uuid(&page.view_id)
            && request
                .view_id
                .as_ref()
                .is_none_or(|id| id == &page.view_id)
            && page.pass > 0
            && page.total <= MAX_VIEW_ENTRIES
            && page.offset == request.offset
            && page.entries.len() <= VIEW_PAGE_ENTRIES
            && page
                .offset
                .checked_add(page.entries.len())
                .is_some_and(|end| end <= page.total)
            && (!page.entries.is_empty() || page.offset == page.total)
            && page.entries.iter().all(SelectionTarget::valid)
            && page.digest.len() == 64
            && page
                .digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
        .then_some(page)
    }
}
pub struct SelectionView {
    generation: PulseGenerationBinding,
    id: String,
    pass: u64,
    entries: Vec<SelectionTarget>,
    digest: String,
}
impl SelectionView {
    pub fn new(
        generation: PulseGenerationBinding,
        pass: u64,
        mut entries: Vec<SelectionTarget>,
    ) -> Option<Self> {
        if !generation.valid()
            || pass == 0
            || entries.len() > MAX_VIEW_ENTRIES
            || !entries.iter().all(SelectionTarget::valid)
        {
            return None;
        }
        entries.sort_by(|a, b| a.wallet.cmp(&b.wallet));
        if entries
            .windows(2)
            .any(|pair| pair[0].wallet == pair[1].wallet)
        {
            return None;
        }
        let digest = digest(&entries)?;
        Some(Self {
            generation,
            id: uuid::Uuid::new_v4().to_string(),
            pass,
            entries,
            digest,
        })
    }
    /// One immutable live capture shared by all readers; a new initial request cannot
    /// invalidate another reader's outstanding pages.
    pub fn cached_page(
        cache: &mut Option<(tokio::time::Instant, Self)>,
        request: &ViewRequest,
        capture: impl FnOnce() -> Option<Self>,
    ) -> Option<Vec<u8>> {
        if request.view_id.is_none()
            && cache
                .as_ref()
                .is_none_or(|(created, _)| created.elapsed() >= VIEW_LIFETIME)
        {
            *cache = Some((tokio::time::Instant::now(), capture()?));
        }
        let (created, view) = cache.as_ref()?;
        if created.elapsed() >= VIEW_LIFETIME {
            return None;
        }
        view.page(request)?.encode()
    }
    pub fn page(&self, request: &ViewRequest) -> Option<ViewPage> {
        if !request.valid()
            || request.generation != self.generation
            || request.offset > self.entries.len()
            || request.view_id.as_ref().is_some_and(|id| id != &self.id)
        {
            return None;
        }
        let end = self.entries.len().min(request.offset + VIEW_PAGE_ENTRIES);
        let page = ViewPage {
            generation: self.generation.clone(),
            request_id: request.request_id.clone(),
            view_id: self.id.clone(),
            pass: self.pass,
            total: self.entries.len(),
            offset: request.offset,
            digest: self.digest.clone(),
            entries: self.entries[request.offset..end].to_vec(),
        };
        page.encode()?;
        Some(page)
    }
}
/// One transfer only. No partial view can establish completeness or admission.
pub struct ViewAssembly {
    header: ViewPage,
    entries: Vec<SelectionTarget>,
}
impl ViewAssembly {
    pub fn begin(page: ViewPage) -> Option<Self> {
        if page.offset != 0
            || page.total > MAX_VIEW_ENTRIES
            || page.entries.len() > VIEW_PAGE_ENTRIES
            || !page.generation.valid()
        {
            return None;
        }
        let entries = Vec::with_capacity(page.total);
        let mut assembly = Self {
            header: page.clone(),
            entries,
        };
        assembly.push(page)?;
        Some(assembly)
    }
    pub fn push(&mut self, page: ViewPage) -> Option<()> {
        if page.entries.len() > VIEW_PAGE_ENTRIES
            || (page.entries.is_empty() && self.header.total != 0)
            || !page.entries.iter().all(SelectionTarget::valid)
            || page.generation != self.header.generation
            || page.view_id != self.header.view_id
            || page.pass != self.header.pass
            || page.total != self.header.total
            || page.digest != self.header.digest
            || page.offset != self.entries.len()
            || self.entries.len().checked_add(page.entries.len())? > self.header.total
            || self
                .entries
                .last()
                .zip(page.entries.first())
                .is_some_and(|(a, b)| a.wallet >= b.wallet)
            || page
                .entries
                .windows(2)
                .any(|pair| pair[0].wallet >= pair[1].wallet)
        {
            return None;
        }
        self.entries.extend(page.entries);
        if self.entries.len() == self.header.total
            && digest(&self.entries).as_deref() != Some(&self.header.digest)
        {
            return None;
        }
        Some(())
    }
    pub fn next_request(&self) -> ViewRequest {
        ViewRequest {
            generation: self.header.generation.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            view_id: Some(self.header.view_id.clone()),
            offset: self.entries.len(),
        }
    }
    pub fn complete(&self) -> bool {
        self.entries.len() == self.header.total
            && digest(&self.entries).as_deref() == Some(&self.header.digest)
    }
    pub fn into_targets(self) -> Option<Vec<SelectionTarget>> {
        self.complete().then_some(self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> PulseGenerationBinding {
        PulseGenerationBinding {
            audience: "fixture".into(),
            epoch: 1,
            source_incarnation: uuid::Uuid::new_v4().to_string(),
        }
    }
    fn entries(count: usize) -> Vec<SelectionTarget> {
        (0..count)
            .map(|index| SelectionTarget {
                wallet: format!("0x{index:040x}"),
                session: "0xffffffffffffffffffffffffffffffffffffffff".into(),
                assignment_version: u64::MAX,
            })
            .collect()
    }
    #[test]
    fn bounded_selection_view_transfers_more_than_one_reply_and_refuses_mixed_or_missing_pages() {
        let generation = binding();
        let view = SelectionView::new(generation.clone(), 1, entries(1400)).unwrap();
        let mut request = ViewRequest {
            generation,
            request_id: uuid::Uuid::new_v4().to_string(),
            view_id: None,
            offset: 0,
        };
        let first =
            ViewPage::decode(&request, &view.page(&request).unwrap().encode().unwrap()).unwrap();
        let mut assembly = ViewAssembly::begin(first.clone()).unwrap();
        assert!(!assembly.complete());
        assert!(assembly.push(first).is_none());
        while !assembly.complete() {
            request = assembly.next_request();
            let page = ViewPage::decode(&request, &view.page(&request).unwrap().encode().unwrap())
                .unwrap();
            let mut wrong = page.clone();
            wrong.generation.epoch += 1;
            assert!(assembly.push(wrong).is_none());
            assembly.push(page).unwrap();
        }
        assert_eq!(assembly.into_targets().unwrap().len(), 1400);
    }
    #[test]
    fn two_interleaved_readers_share_one_immutable_multpage_view() {
        let generation = binding();
        let mut cache = Some((
            tokio::time::Instant::now(),
            SelectionView::new(generation.clone(), 1, entries(1400)).unwrap(),
        ));
        let first = |id| ViewRequest {
            generation: generation.clone(),
            request_id: id,
            view_id: None,
            offset: 0,
        };
        let left_request = first(uuid::Uuid::new_v4().to_string());
        let mut left = ViewAssembly::begin(
            ViewPage::decode(
                &left_request,
                &SelectionView::cached_page(&mut cache, &left_request, || {
                    panic!("initial reader must reuse live capture")
                })
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let right_request = first(uuid::Uuid::new_v4().to_string());
        let mut right = ViewAssembly::begin(
            ViewPage::decode(
                &right_request,
                &SelectionView::cached_page(&mut cache, &right_request, || {
                    panic!("second reader must reuse live capture")
                })
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        while !left.complete() || !right.complete() {
            for reader in [&mut left, &mut right] {
                if !reader.complete() {
                    let request = reader.next_request();
                    reader
                        .push(
                            ViewPage::decode(
                                &request,
                                &SelectionView::cached_page(&mut cache, &request, || {
                                    panic!("continuation must not capture")
                                })
                                .unwrap(),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                }
            }
        }
        assert_eq!(left.into_targets(), right.into_targets());
    }
    #[test]
    fn complete_bad_digest_fails_at_the_last_page_and_valid_empty_views_complete() {
        let generation = binding();
        let view = SelectionView::new(generation.clone(), 1, entries(700)).unwrap();
        let request = ViewRequest {
            generation,
            request_id: uuid::Uuid::new_v4().to_string(),
            view_id: None,
            offset: 0,
        };
        let mut first = view.page(&request).unwrap();
        let bad_digest = "0".repeat(64);
        assert_ne!(first.digest, bad_digest);
        first.digest = bad_digest.clone();
        let mut assembly = ViewAssembly::begin(first).unwrap();
        loop {
            let request = assembly.next_request();
            let mut page = view.page(&request).unwrap();
            page.digest = bad_digest.clone();
            if page.offset + page.entries.len() == page.total {
                assert!(
                    assembly.push(page).is_none(),
                    "the final mismatch must stop the transfer instead of requesting empty continuation pages"
                );
                break;
            }
            assembly.push(page).unwrap();
        }
        let empty = SelectionView::new(binding(), 1, vec![]).unwrap();
        let request = ViewRequest {
            generation: empty.generation.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            view_id: None,
            offset: 0,
        };
        let page = empty.page(&request).unwrap();
        assert_eq!(
            ViewAssembly::begin(page.clone()).unwrap().into_targets(),
            Some(vec![])
        );
        let mut corrupt = page;
        corrupt.digest = bad_digest;
        assert!(ViewAssembly::begin(corrupt).is_none());
        let mut stalled = view
            .page(&ViewRequest {
                generation: view.generation.clone(),
                request_id: uuid::Uuid::new_v4().to_string(),
                view_id: None,
                offset: 0,
            })
            .unwrap();
        stalled.entries.clear();
        assert!(
            ViewAssembly::begin(stalled).is_none(),
            "a nonempty transfer cannot accept a page with no progress"
        );
    }
    #[test]
    fn worst_case_selection_entry_and_page_encoding_fit_the_declared_bounds() {
        let entry = SelectionTarget {
            wallet: "0xffffffffffffffffffffffffffffffffffffffff".into(),
            session: "0xffffffffffffffffffffffffffffffffffffffff".into(),
            assignment_version: u64::MAX,
        };
        assert_eq!(serde_json::to_vec(&entry).unwrap().len(), 152);
        assert!(MAX_VIEW_ENTRIES * 153 + 1 < MAX_VIEW_BYTES);
        let view = SelectionView::new(binding(), u64::MAX, entries(MAX_VIEW_ENTRIES)).unwrap();
        let request = ViewRequest {
            generation: view.generation.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            view_id: None,
            offset: 0,
        };
        assert!(
            view.page(&request).unwrap().encode().unwrap().len()
                < super::super::cluster::MAX_REPLY_BYTES
        );
        let mut invalid = request.clone();
        invalid.generation.source_incarnation = "unregistered-shape".into();
        assert!(invalid.encode().is_none());
        assert!(
            ViewRequest::decode("pulse.foreign.selection_view", &request.encode().unwrap())
                .is_none()
        );
    }
}
