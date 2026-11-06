pub mod address;
mod audio_transmitter;
mod pinned;
pub mod plugin;
pub mod states;
#[cfg(test)]
mod tests;
mod video_receiver;
mod video_transmitter;

use bevy::prelude::*;

pub use crate::{audio_transmitter::*, pinned::*, video_receiver::*, video_transmitter::*};

#[derive(Event)]
pub struct TransmissionUpdated;

#[derive(Event)]
pub struct TransmissionStopped;
