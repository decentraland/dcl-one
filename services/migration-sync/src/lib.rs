//! Standalone migration coordination; never linked into the serving backend.
pub mod capture;
pub mod dispatch;
pub mod model;
pub mod store;

pub mod ops;

pub mod domain_capture;
pub mod events_schedules;
pub mod social_settings;

pub mod catalogue;
