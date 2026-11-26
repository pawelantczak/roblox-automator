use std::time::{Duration, Instant};

use crate::models::{DetectedText, FailureReason, InstanceId, InstanceState};

/// Tracks per-window state transitions, deadlines, cooldowns, and frame caches.
#[derive(Debug, Clone)]
pub struct InstanceTracker {
    pub instance_id: InstanceId,
    pub pid: u32,
    pub title: String,
    pub state: InstanceState,
    pub last_transition: Instant,
    pub failure_reason: Option<FailureReason>,
    pub next_afk_deadline: Option<Instant>,
    pub reconnect_cooldown_until: Option<Instant>,
    pub event_cooldown_until: Option<Instant>,
    pub last_frame_hash: Option<u64>,
    pub cached_thumbnail: Option<String>,
    pub event_crop_base64: Option<String>,
    pub reconnect_crop_base64: Option<String>,
    pub cached_ocr: Option<Vec<DetectedText>>,
    pub restarts: u32,
    pub ocr_failures: u32,
    pub event_status: String,
}

impl InstanceTracker {
    pub fn new(instance_id: InstanceId, pid: u32, title: String, now: Instant) -> Self {
        Self {
            instance_id,
            pid,
            title,
            state: InstanceState::Discovering,
            last_transition: now,
            failure_reason: None,
            next_afk_deadline: None,
            reconnect_cooldown_until: None,
            event_cooldown_until: None,
            last_frame_hash: None,
            cached_thumbnail: None,
            event_crop_base64: None,
            reconnect_crop_base64: None,
            cached_ocr: None,
            restarts: 0,
            ocr_failures: 0,
            event_status: "Standby".to_string(),
        }
    }

    pub fn transition_to(
        &mut self,
        new_state: InstanceState,
        reason: Option<FailureReason>,
        now: Instant,
    ) {
        if self.state != new_state || self.failure_reason != reason {
            self.state = new_state;
            self.last_transition = now;
            self.failure_reason = reason;
        }
    }

    /// Stops anti-AFK timers while watcher is idle.
    pub fn enter_idle(&mut self, now: Instant, standby_label: &str) {
        self.transition_to(InstanceState::Idle, None, now);
        self.next_afk_deadline = None;
        self.event_status = standby_label.to_string();
        self.event_crop_base64 = None;
        self.reconnect_crop_base64 = None;
    }

    /// Initializes anti-AFK deadline safely for newly active or discovered instances.
    pub fn ensure_afk_deadline(&mut self, now: Instant, enabled: bool, interval: Duration) {
        if !enabled {
            self.next_afk_deadline = None;
            return;
        }
        if self.next_afk_deadline.is_none() {
            self.next_afk_deadline = Some(now + interval);
        }
    }

    pub fn start_reconnect_cooldown(&mut self, now: Instant, cooldown: Duration) {
        self.reconnect_cooldown_until = Some(now + cooldown);
    }

    pub fn is_cooldown_active(&self, now: Instant) -> bool {
        self.reconnect_cooldown_until
            .map(|until| now < until)
            .unwrap_or(false)
    }

    pub fn cooldown_remaining_secs(&self, now: Instant) -> Option<u64> {
        self.reconnect_cooldown_until.and_then(|until| {
            if now < until {
                Some((until - now).as_secs().max(1))
            } else {
                None
            }
        })
    }

    pub fn start_event_cooldown(&mut self, now: Instant, cooldown: Duration) {
        self.event_cooldown_until = Some(now + cooldown);
    }

    pub fn is_event_cooldown_active(&self, now: Instant) -> bool {
        self.event_cooldown_until
            .map(|until| now < until)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions_and_idle_timer_stop() {
        let t0 = Instant::now();
        let mut tracker = InstanceTracker::new(42, 1001, "Roblox".to_string(), t0);
        assert_eq!(tracker.state, InstanceState::Discovering);
        assert!(tracker.next_afk_deadline.is_none());

        tracker.ensure_afk_deadline(t0, true, Duration::from_secs(600));
        assert!(tracker.next_afk_deadline.is_some());

        tracker.transition_to(InstanceState::Healthy, None, t0 + Duration::from_secs(5));
        assert_eq!(tracker.state, InstanceState::Healthy);

        tracker.enter_idle(t0 + Duration::from_secs(10), "Standby");
        assert_eq!(tracker.state, InstanceState::Idle);
        assert!(tracker.next_afk_deadline.is_none());
    }

    #[test]
    fn test_deterministic_reconnect_cooldown() {
        let t0 = Instant::now();
        let mut tracker = InstanceTracker::new(7, 2002, "Roblox".to_string(), t0);
        assert!(!tracker.is_cooldown_active(t0));
        assert_eq!(tracker.cooldown_remaining_secs(t0), None);

        tracker.start_reconnect_cooldown(t0, Duration::from_secs(30));
        assert!(tracker.is_cooldown_active(t0 + Duration::from_secs(10)));
        assert_eq!(
            tracker.cooldown_remaining_secs(t0 + Duration::from_secs(10)),
            Some(20)
        );
        assert!(!tracker.is_cooldown_active(t0 + Duration::from_secs(30)));
        assert_eq!(
            tracker.cooldown_remaining_secs(t0 + Duration::from_secs(30)),
            None
        );
    }
}
