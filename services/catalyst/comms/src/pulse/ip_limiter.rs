//! Concurrent connections per observed source IP, owned by the server event loop.
use std::{collections::HashMap, net::IpAddr};

use super::hardening::IpWhitelist;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpLimiterOptions {
    pub enabled: bool,
    pub max_concurrency: usize,
    pub scene_listener_max_concurrency: usize,
    pub whitelist: IpWhitelist,
}

impl Default for IpLimiterOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_concurrency: 10,
            scene_listener_max_concurrency: 2,
            whitelist: IpWhitelist::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Player,
    Listener,
}

#[derive(Default)]
pub struct IpLimiter {
    counts: HashMap<IpAddr, [usize; 2]>,
    reservations: HashMap<u32, (IpAddr, Class)>,
}

impl IpLimiter {
    pub fn contains_peer(&self, peer: u32) -> bool {
        self.reservations.contains_key(&peer)
    }
    /// Acquire and bind together: the loop already has a transport ID and owns every rollback.
    /// Unidentified connections remain uncounted while enforcement is off, as in upstream.
    pub fn acquire(&mut self, peer: u32, ip: &str, options: &IpLimiterOptions) -> bool {
        if self.reservations.contains_key(&peer) {
            return false;
        }
        let enforcing = options.enabled && options.max_concurrency > 0;
        let Ok(ip) = ip.parse::<IpAddr>() else {
            return !enforcing;
        };
        let ip = ip.to_canonical();
        let count = self.counts.get(&ip).map_or(0, |counts| counts[0]);
        if enforcing
            && count >= options.max_concurrency
            && !options.whitelist.contains(&ip.to_string())
        {
            return false;
        }
        self.counts.entry(ip).or_default()[0] += 1;
        self.reservations.insert(peer, (ip, Class::Player));
        true
    }

    /// Validate the target before changing either counter. Refusal preserves the player slot.
    pub fn reclassify_listener(&mut self, peer: u32, options: &IpLimiterOptions) -> bool {
        let Some(&(ip, class)) = self.reservations.get(&peer) else {
            return true;
        };
        if class == Class::Listener {
            return true;
        }
        let counts = self.counts.get_mut(&ip).expect("reservation has counts");
        let cap = options.scene_listener_max_concurrency;
        if options.enabled
            && cap > 0
            && counts[1] >= cap
            && !options.whitelist.contains(&ip.to_string())
        {
            return false;
        }
        counts[0] -= 1;
        counts[1] += 1;
        self.reservations.insert(peer, (ip, Class::Listener));
        true
    }

    pub fn release(&mut self, peer: u32) {
        let Some((ip, class)) = self.reservations.remove(&peer) else {
            return;
        };
        let counts = self.counts.get_mut(&ip).expect("reservation has counts");
        counts[usize::from(class == Class::Listener)] -= 1;
        if *counts == [0, 0] {
            self.counts.remove(&ip);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn enabled() -> IpLimiterOptions {
        IpLimiterOptions {
            enabled: true,
            max_concurrency: 2,
            scene_listener_max_concurrency: 1,
            ..Default::default()
        }
    }

    #[test]
    fn shared_transport_ids_and_mapped_addresses_use_one_budget() {
        let mut limiter = IpLimiter::default();
        let options = enabled();
        assert!(limiter.acquire(1, "192.0.2.1", &options));
        assert!(limiter.acquire(4096, "::ffff:192.0.2.1", &options));
        assert!(!limiter.acquire(2, "192.0.2.1", &options));
        limiter.release(1);
        limiter.release(1);
        assert!(limiter.acquire(2, "192.0.2.1", &options));
        assert!(!limiter.acquire(3, "192.0.2.1", &options));
    }

    #[test]
    fn listener_move_is_atomic_and_failed_move_still_releases_player() {
        let mut limiter = IpLimiter::default();
        let options = enabled();
        assert!(limiter.acquire(1, "192.0.2.1", &options));
        assert!(limiter.reclassify_listener(1, &options));
        assert!(limiter.reclassify_listener(1, &options));
        assert!(limiter.acquire(2, "192.0.2.1", &options));
        assert!(!limiter.reclassify_listener(2, &options));
        assert!(limiter.acquire(3, "192.0.2.1", &options));
        assert!(!limiter.acquire(4, "192.0.2.1", &options));
        limiter.release(2);
        assert!(limiter.acquire(4, "192.0.2.1", &options));
        limiter.release(1);
        assert!(limiter.reclassify_listener(3, &options));
        limiter.release(3);
        limiter.release(4);
        assert!(limiter.counts.is_empty());
        assert!(limiter.reservations.is_empty());
    }

    #[test]
    fn enabling_removing_exemption_and_lowering_caps_use_existing_counts_without_eviction() {
        let mut limiter = IpLimiter::default();
        let mut options = enabled();
        options.enabled = false;
        for peer in 0..4 {
            assert!(limiter.acquire(peer, "192.0.2.1", &options));
        }
        options.enabled = true;
        assert!(!limiter.acquire(4, "192.0.2.1", &options));
        options.whitelist = IpWhitelist::parse("::ffff:192.0.2.1").unwrap();
        assert!(limiter.acquire(4, "192.0.2.1", &options));
        options.whitelist = IpWhitelist::new();
        options.max_concurrency = 1;
        assert!(!limiter.acquire(5, "192.0.2.1", &options));
        for peer in 0..4 {
            limiter.release(peer);
        }
        assert!(!limiter.acquire(5, "192.0.2.1", &options));
        limiter.release(4);
        assert!(limiter.acquire(5, "192.0.2.1", &options));
    }

    #[test]
    fn zero_caps_count_without_refusal_and_unknown_ip_is_refused_only_when_enforcing() {
        let mut limiter = IpLimiter::default();
        let mut options = enabled();
        assert!(!limiter.acquire(1, "", &options));
        assert!(!limiter.acquire(1, "not-an-ip", &options));
        options.max_concurrency = 0;
        assert!(limiter.acquire(1, "", &options));
        for peer in 2..6 {
            assert!(limiter.acquire(peer, "192.0.2.1", &options));
        }
        options.max_concurrency = 2;
        assert!(!limiter.acquire(6, "192.0.2.1", &options));
        options.scene_listener_max_concurrency = 0;
        for peer in 2..6 {
            assert!(limiter.reclassify_listener(peer, &options));
        }
        options.scene_listener_max_concurrency = 1;
        assert!(limiter.acquire(6, "192.0.2.1", &options));
        assert!(!limiter.reclassify_listener(6, &options));
    }
}
