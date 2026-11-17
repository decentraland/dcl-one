//! One immutable source-scoped transfer, scoped to the existing subscriber job.
use super::*;
use crate::interfaces::PulseGenerationBinding;
use crate::protocol::bus::selection_view::{
    SelectionTarget, VIEW_LIFETIME, ViewAssembly, ViewPage, ViewRequest,
};

impl ClusterSubscriber {
    async fn fetch_selection_view(
        &self,
        binding: &PulseGenerationBinding,
    ) -> Option<Vec<SelectionTarget>> {
        let mut request = ViewRequest {
            generation: binding.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            view_id: None,
            offset: 0,
        };
        let bytes = self
            .bus
            .request(&request.subject(), request.encode()?)
            .await
            .ok()?;
        let first = ViewPage::decode(&request, &bytes)?;
        let mut assembly = ViewAssembly::begin(first)?;
        while !assembly.complete() {
            request = assembly.next_request();
            let bytes = self
                .bus
                .request(&request.subject(), request.encode()?)
                .await
                .ok()?;
            assembly.push(ViewPage::decode(&request, &bytes)?)?;
            tokio::task::yield_now().await;
        }
        assembly.into_targets()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub async fn recover_selection_views_for_test(self: &Arc<Self>) {
        self.recover_selection_views().await;
    }

    /// Independent cooperative future, not another task or unbounded work queue.
    /// A complete view seeds hints only: every admission still performs fresh scoped lookup/SQL checks.
    pub(crate) async fn recover_selection_views(self: &Arc<Self>) {
        loop {
            let Some(authority) = self
                .assignment_fence
                .as_ref()
                .filter(|fence| fence.requires_generation())
            else {
                std::future::pending::<()>().await;
                return;
            };
            if !self.is_ready() {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            let binding = tokio::time::timeout(
                super::super::fence::FENCE_CALL_TIMEOUT,
                authority.generation_binding(),
            )
            .await
            .ok()
            .flatten();
            let Some(binding) = binding else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            let targets = tokio::time::timeout(VIEW_LIFETIME, self.fetch_selection_view(&binding))
                .await
                .ok()
                .flatten();
            let Some(targets) = targets else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            if !tokio::time::timeout(
                super::super::fence::FENCE_CALL_TIMEOUT,
                authority.complete_generation_view(&binding),
            )
            .await
            .unwrap_or(false)
            {
                continue;
            }
            // Retain one complete bounded vector while rotating through every target. Never restart
            // at its first wallet on each tick, and never allocate a slot from a partial view.
            let mut offset = 0;
            loop {
                let current = tokio::time::timeout(
                    super::super::fence::FENCE_CALL_TIMEOUT,
                    authority.generation_binding(),
                )
                .await
                .ok()
                .flatten();
                if !self.is_ready() || current.as_ref() != Some(&binding) {
                    break;
                }
                authority
                    .retire_obsolete_generation(&binding, 128, &targets)
                    .await;
                let end = targets.len().min(offset + 128);
                for target in &targets[offset..end] {
                    if tokio::time::timeout(
                        super::super::fence::FENCE_CALL_TIMEOUT,
                        authority.recover_selected_target(&binding, target),
                    )
                    .await
                    .unwrap_or(false)
                    {
                        self.announce_class(&target.wallet, &target.session, true);
                    }
                    tokio::task::yield_now().await;
                }
                offset = end;
                tokio::time::sleep(Duration::from_secs(1)).await;
                if offset == targets.len() {
                    break;
                }
            }
        }
    }
}
