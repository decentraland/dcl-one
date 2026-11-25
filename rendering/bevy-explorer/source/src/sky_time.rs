//! The time of day for the HUD's Skybox menu: the day/night systems keep it in
//! [`SkyTime`], and this sends it on `SystemApi::GetSkyTimeStream`.

use bevy::prelude::*;
use common::{
    rpc::RpcStreamSender,
    structs::{SkyTime, SkyTimeState},
};
use system_bridge::SystemApi;

pub struct SkyTimePlugin;

impl Plugin for SkyTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkyTime>()
            .add_systems(Update, pipe_sky_time);
    }
}

fn pipe_sky_time(
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<SkyTimeState>>>,
    current: Res<SkyTime>,
) {
    let new_senders = requests
        .read()
        .filter_map(|ev| {
            if let SystemApi::GetSkyTimeStream(sender) = ev {
                Some(sender.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    // a new subscriber gets the state as it is
    for sender in &new_senders {
        let _ = sender.send(current.0);
    }
    senders.retain(|sender| !sender.is_closed());
    if current.is_changed() {
        for sender in senders.iter() {
            let _ = sender.send(current.0);
        }
    }
    senders.extend(new_senders);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subscriber_gets_the_state_first_and_then_each_change() {
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .init_resource::<SkyTime>()
            .add_systems(Update, pipe_sky_time);
        let locked = SkyTimeState {
            locked: true,
            speed: 12.,
            fixed_hours: Some(22.),
        };
        app.world_mut().resource_mut::<SkyTime>().0 = locked;
        // a frame with nobody listening
        app.update();

        let (sender, mut receiver) = RpcStreamSender::channel();
        app.world_mut()
            .send_event(SystemApi::GetSkyTimeStream(sender));
        let mut received = |app: &mut App| {
            app.update();
            std::iter::from_fn(|| receiver.try_recv().ok()).collect::<Vec<_>>()
        };
        assert_eq!(received(&mut app), [locked]);
        // nothing changed: nothing sent
        assert_eq!(received(&mut app), []);

        let unlocked = SkyTimeState {
            locked: false,
            speed: 12.,
            fixed_hours: None,
        };
        app.world_mut().resource_mut::<SkyTime>().0 = unlocked;
        assert_eq!(received(&mut app), [unlocked]);
        assert_eq!(received(&mut app), []);
    }
}
