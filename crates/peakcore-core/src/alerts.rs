//! Threshold alerts evaluated by the daemon on every tray sample.

use crate::RingBuffer;
use crate::config::{AlertMetric, AlertRule};
use serde::{Deserialize, Serialize};

pub const ALERT_HISTORY_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlertEvent {
    pub ts_ms: u64,
    pub metric: AlertMetric,
    pub threshold_pct: u8,
    pub value_pct: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlertStatus {
    pub metric: AlertMetric,
    pub threshold_pct: u8,
    pub duration_s: u32,
    pub cooldown_min: u32,
    pub value_pct: f32,
    pub triggered: bool,
    pub above_since_ms: Option<u64>,
    pub last_fired_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default)]
struct RuleState {
    above_since_ms: Option<u64>,
    last_fired_ms: Option<u64>,
}

/// Rules plus the state needed to fire once per sustained breach and cooldown.
#[derive(Debug)]
pub struct Alerts {
    rules: Vec<AlertRule>,
    states: Vec<RuleState>,
    history: RingBuffer<AlertEvent>,
}

impl Default for Alerts {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Alerts {
    pub fn new(rules: Vec<AlertRule>) -> Self {
        Self {
            states: vec![RuleState::default(); rules.len()],
            rules,
            history: RingBuffer::new(ALERT_HISTORY_CAPACITY),
        }
    }

    pub fn rules(&self) -> &[AlertRule] {
        &self.rules
    }

    pub fn history(&self) -> impl DoubleEndedIterator<Item = &AlertEvent> + ExactSizeIterator {
        self.history.iter()
    }

    /// Applies new rules and forgets everything learned under the old ones.
    pub fn replace_rules(&mut self, rules: Vec<AlertRule>) {
        self.rules = rules;
        self.states = vec![RuleState::default(); self.rules.len()];
        self.history = RingBuffer::new(ALERT_HISTORY_CAPACITY);
    }

    /// Feeds one sample and returns the alerts that fired because of it.
    pub fn evaluate(&mut self, now_ms: u64, cpu_pct: f32, mem_pct: f32) -> Vec<AlertEvent> {
        let mut fired = Vec::new();
        for index in 0..self.rules.len() {
            let rule = &self.rules[index];
            let value = match rule.metric {
                AlertMetric::Cpu => cpu_pct,
                AlertMetric::Ram => mem_pct,
            };
            let state = &mut self.states[index];
            if value < f32::from(rule.threshold_pct) {
                state.above_since_ms = None;
                continue;
            }
            let since = *state.above_since_ms.get_or_insert(now_ms);
            let sustained_ms = now_ms.saturating_sub(since);
            let cooldown_ms = u64::from(rule.cooldown_min) * 60_000;
            let cooldown_elapsed = state
                .last_fired_ms
                .is_none_or(|last| now_ms.saturating_sub(last) >= cooldown_ms);
            if sustained_ms < u64::from(rule.duration_s) * 1_000 || !cooldown_elapsed {
                continue;
            }
            let event = AlertEvent {
                ts_ms: now_ms,
                metric: rule.metric,
                threshold_pct: rule.threshold_pct,
                value_pct: value,
            };
            state.last_fired_ms = Some(now_ms);
            // Re-arm the window so the rule needs another full duration before firing again.
            state.above_since_ms = Some(now_ms);
            self.history.push(event);
            fired.push(event);
        }
        fired
    }

    /// Current rule state for the dashboard; mirrors the last `evaluate` call.
    pub fn status(&self, cpu_pct: f32, mem_pct: f32) -> Vec<AlertStatus> {
        self.rules
            .iter()
            .zip(&self.states)
            .map(|(rule, state)| AlertStatus {
                metric: rule.metric,
                threshold_pct: rule.threshold_pct,
                duration_s: rule.duration_s,
                cooldown_min: rule.cooldown_min,
                value_pct: match rule.metric {
                    AlertMetric::Cpu => cpu_pct,
                    AlertMetric::Ram => mem_pct,
                },
                triggered: state.above_since_ms.is_some(),
                above_since_ms: state.above_since_ms,
                last_fired_ms: state.last_fired_ms,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: u64 = 1_000;
    const MINUTE: u64 = 60 * SECOND;

    fn cpu_rule() -> AlertRule {
        AlertRule {
            metric: AlertMetric::Cpu,
            threshold_pct: 90,
            duration_s: 30,
            cooldown_min: 10,
        }
    }

    #[test]
    fn alert_fires_only_after_the_threshold_is_sustained() {
        let mut alerts = Alerts::new(vec![cpu_rule()]);

        assert!(alerts.evaluate(0, 95.0, 0.0).is_empty());
        assert!(alerts.evaluate(29 * SECOND, 95.0, 0.0).is_empty());

        let fired = alerts.evaluate(30 * SECOND, 95.0, 0.0);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].metric, AlertMetric::Cpu);
        assert_eq!(fired[0].value_pct, 95.0);
        assert_eq!(fired[0].threshold_pct, 90);
    }

    #[test]
    fn alert_restarts_its_window_when_the_value_drops_below_the_threshold() {
        let mut alerts = Alerts::new(vec![cpu_rule()]);

        alerts.evaluate(0, 95.0, 0.0);
        alerts.evaluate(20 * SECOND, 95.0, 0.0);
        assert!(alerts.evaluate(21 * SECOND, 20.0, 0.0).is_empty());

        alerts.evaluate(22 * SECOND, 95.0, 0.0);
        assert!(
            alerts.evaluate(40 * SECOND, 95.0, 0.0).is_empty(),
            "the window restarted at 22s, so 18s above the threshold is not enough"
        );

        let fired = alerts.evaluate(52 * SECOND, 95.0, 0.0);
        assert_eq!(fired.len(), 1);
    }

    #[test]
    fn alert_respects_the_cooldown_after_firing() {
        let mut alerts = Alerts::new(vec![cpu_rule()]);
        alerts.evaluate(0, 95.0, 0.0);
        assert_eq!(alerts.evaluate(30 * SECOND, 95.0, 0.0).len(), 1);

        // Sustained again, but inside the ten minute cooldown.
        alerts.evaluate(31 * SECOND, 95.0, 0.0);
        assert!(alerts.evaluate(5 * MINUTE, 95.0, 0.0).is_empty());

        assert_eq!(alerts.evaluate(11 * MINUTE, 95.0, 0.0).len(), 1);
    }

    #[test]
    fn alert_status_mirrors_the_last_evaluation() {
        let mut alerts = Alerts::new(vec![cpu_rule()]);
        alerts.evaluate(0, 95.0, 0.0);
        alerts.evaluate(10 * SECOND, 95.0, 0.0);
        assert_eq!(alerts.evaluate(30 * SECOND, 95.0, 0.0).len(), 1);

        alerts.evaluate(40 * SECOND, 50.0, 70.0);
        let status = alerts.status(50.0, 70.0);
        assert_eq!(status.len(), 1);
        assert!(!status[0].triggered, "the metric is below the threshold");
        assert_eq!(status[0].above_since_ms, None);
        assert_eq!(status[0].threshold_pct, 90);
        assert_eq!(status[0].last_fired_ms, Some(30 * SECOND));
        assert_eq!(status[0].value_pct, 50.0);

        alerts.evaluate(45 * SECOND, 95.0, 70.0);
        let status = alerts.status(95.0, 70.0);
        assert!(status[0].triggered);
        assert_eq!(status[0].above_since_ms, Some(45 * SECOND));
    }

    #[test]
    fn evaluating_memory_rules_uses_the_memory_metric() {
        let mut alerts = Alerts::new(vec![AlertRule {
            metric: AlertMetric::Ram,
            threshold_pct: 80,
            duration_s: 5,
            cooldown_min: 0,
        }]);

        assert!(alerts.evaluate(0, 99.0, 50.0).is_empty());
        assert!(alerts.evaluate(5 * SECOND, 99.0, 85.0).is_empty());
        let fired = alerts.evaluate(10 * SECOND, 99.0, 85.0);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].metric, AlertMetric::Ram);
        assert_eq!(fired[0].value_pct, 85.0);
    }

    #[test]
    fn firing_records_history_newest_last() {
        let mut alerts = Alerts::new(vec![cpu_rule()]);
        alerts.evaluate(0, 95.0, 0.0);
        alerts.evaluate(30 * SECOND, 95.0, 0.0);
        alerts.evaluate(31 * SECOND, 95.0, 0.0);
        alerts.evaluate(30 * MINUTE, 95.0, 0.0);

        let history: Vec<_> = alerts.history().copied().collect();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].ts_ms, 30 * SECOND);
        assert_eq!(history[1].ts_ms, 30 * MINUTE);
    }

    #[test]
    fn replacing_rules_clears_previous_state() {
        let mut alerts = Alerts::new(vec![cpu_rule()]);
        alerts.evaluate(0, 95.0, 0.0);
        alerts.evaluate(30 * SECOND, 95.0, 0.0);
        assert_eq!(alerts.history().count(), 1);

        alerts.replace_rules(vec![AlertRule {
            metric: AlertMetric::Cpu,
            threshold_pct: 50,
            duration_s: 1,
            cooldown_min: 0,
        }]);
        assert_eq!(alerts.history().count(), 0);
        assert!(alerts.evaluate(31 * SECOND, 40.0, 0.0).is_empty());
        assert_eq!(alerts.evaluate(32 * SECOND, 60.0, 0.0).len(), 0);
        assert_eq!(alerts.evaluate(33 * SECOND, 60.0, 0.0).len(), 1);
    }
}
