//! Lifecycle and diagnostics hooks for the statistics host.
use super::{
    content,
    http::{Hooks, Lifecycle},
};
use crate::archipelago::AppState;
use std::{sync::Arc, sync::atomic::Ordering};

pub struct Runtime {
    connector: AppState,
}
impl Runtime {
    pub fn new(connector: AppState) -> Arc<Self> {
        Arc::new(Self { connector })
    }
}
#[async_trait::async_trait]
impl Hooks for Runtime {
    async fn probe(&self, startup: bool) -> Lifecycle {
        let state = &self.connector;
        Lifecycle {
            initialized: true,
            live: !state.restart.is_cancelled(),
            started: state.started.load(Ordering::Acquire),
            // Original server.startupProbe succeeds after construction; readiness is listening.
            checks: vec![("server".into(), startup || state.listener_ready())],
        }
    }
    async fn diagnostics(&self) -> Result<String, content::Error> {
        Ok(crate::archipelago::handlers::diagnostics(&self.connector))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn lifecycle_reports_real_listener_start_and_shutdown_without_dependency_probes() {
        let state = crate::archipelago::test_support::state_over(
            crate::archipelago::Settings::for_tests(),
            Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let hooks = Runtime::new(state.clone());
        let probe = hooks.probe(false).await;
        assert!(!probe.started);
        assert!(probe.live);
        assert_eq!(probe.checks, vec![("server".into(), false)]);
        state.started.store(true, Ordering::Release);
        assert!(hooks.probe(true).await.started);
        state.restart.cancel();
        assert!(!hooks.probe(false).await.live);
    }
}
