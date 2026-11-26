//! The local player's situational reactions, as the avatar crate reports them
//! ([`PlayerReacted`]): `react_situation` with the emoji's index on the wire.

use bevy::prelude::*;
use common::structs::PlayerReacted;

use crate::{
    data_definition::{SegmentEvent, SegmentEventReactSituation},
    segment_system::{SegmentConfig, SegmentMetricsEvents},
};

/// Nothing is reported in an app with no analytics session.
pub(crate) fn report_reactions(
    mut reactions: EventReader<PlayerReacted>,
    config: Option<Res<SegmentConfig>>,
    metrics: Option<ResMut<SegmentMetricsEvents>>,
) {
    let (Some(config), Some(mut metrics)) = (config, metrics) else {
        reactions.clear();
        return;
    };
    for reaction in reactions.read() {
        metrics.add_event(
            SegmentEvent::ReactSituation(SegmentEventReactSituation {
                reaction_id: reaction.emoji_index,
            }),
            config.common(),
        );
    }
}

#[cfg(test)]
mod tests {
    use common::rpc::{RpcStreamReceiver, RpcStreamSender};
    use system_bridge::AnalyticsEvent;

    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_event::<PlayerReacted>()
            .add_systems(Update, report_reactions);
        app
    }

    fn listen(app: &mut App) -> RpcStreamReceiver<AnalyticsEvent> {
        let (sender, receiver) = RpcStreamSender::channel();
        let mut metrics = SegmentMetricsEvents::default();
        metrics.add_listener(sender);
        app.insert_resource(metrics)
            .insert_resource(SegmentConfig::new(
                "user".into(),
                "session".into(),
                "1.0".into(),
                None,
                false,
            ));
        receiver
    }

    fn sent(receiver: &mut RpcStreamReceiver<AnalyticsEvent>) -> Vec<(String, serde_json::Value)> {
        std::iter::from_fn(|| receiver.try_recv().ok())
            .map(|got| (got.event, got.properties))
            .collect()
    }

    #[test]
    fn each_reaction_is_reported_with_its_wire_index() {
        let mut app = app();
        let mut receiver = listen(&mut app);
        for emoji_index in [7, 7, 1024] {
            app.world_mut().send_event(PlayerReacted { emoji_index });
        }
        app.update();
        let sent = sent(&mut receiver);
        assert_eq!(sent.len(), 3);
        for ((event, properties), index) in sent.into_iter().zip([7, 7, 1024]) {
            assert_eq!(event, "react_situation");
            assert_eq!(properties["reaction_id"], index);
        }
        // each once
        app.update();
        assert!(self::sent(&mut receiver).is_empty());
    }

    #[test]
    fn an_app_with_no_analytics_session_reports_nothing() {
        let mut app = app();
        app.world_mut().send_event(PlayerReacted { emoji_index: 7 });
        app.update();
        // a session that starts later hears of nothing from before it
        let mut receiver = listen(&mut app);
        app.update();
        assert!(sent(&mut receiver).is_empty());
    }
}
