//! DeepSeek Harness usage replacement fold.
//!
//! The fold mirrors DSH's durable `tokenUsage` projection: a later usage
//! sample for the same `(turn, step)` replaces the earlier sample, identical
//! buckets are ignored, and `llm/retry-started` closes the replacement slot so
//! a retried attempt contributes separately.

#![allow(
    dead_code,
    reason = "usage fold is wired into the collector in a later chunk"
)]

use super::event_parser::{SessionEvent, SessionModelRoute, UsageEvent, UsageSettlementKind};
use crate::domain::usage::TokenUsage;

/// One exact usage contribution after replacement and retry semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsageObservation {
    pub(crate) seq: u64,
    pub(crate) observed_at_ms: i64,
    pub(crate) turn: u64,
    pub(crate) step: u64,
    pub(crate) route: Option<SessionModelRoute>,
    pub(crate) tokens: TokenUsage,
}

/// Fold parsed session events into exact usage contributions.
pub(crate) fn fold_events(events: &[SessionEvent]) -> Vec<UsageObservation> {
    let mut observations: Vec<UsageObservation> = Vec::new();
    let mut current_route: Option<SessionModelRoute> = None;
    let mut last_contribution: Option<usize> = None;

    for event in events {
        match event {
            SessionEvent::Route(route) => {
                current_route = Some(route.clone());
            }
            SessionEvent::RetryStarted { turn, step } => {
                if let Some(index) = last_contribution {
                    let previous = &observations[index];
                    if previous.turn == *turn && previous.step == *step {
                        last_contribution = None;
                    }
                }
            }
            SessionEvent::Usage(event) => {
                if let Some(index) = last_contribution {
                    let previous = &observations[index];
                    if previous.turn == event.turn && previous.step == event.step {
                        if previous.tokens == event.tokens {
                            continue;
                        }
                        observations[index] = UsageObservation {
                            seq: event.seq,
                            observed_at_ms: event.observed_at_ms,
                            turn: event.turn,
                            step: event.step,
                            route: resolved_route(event, &current_route),
                            tokens: event.tokens.clone(),
                        };
                        continue;
                    }
                }

                observations.push(UsageObservation {
                    seq: event.seq,
                    observed_at_ms: event.observed_at_ms,
                    turn: event.turn,
                    step: event.step,
                    route: resolved_route(event, &current_route),
                    tokens: event.tokens.clone(),
                });
                last_contribution = Some(observations.len() - 1);
            }
        }
    }

    observations
}

fn resolved_route(
    event: &UsageEvent,
    current_route: &Option<SessionModelRoute>,
) -> Option<SessionModelRoute> {
    match event.kind {
        UsageSettlementKind::AssistantMessage => event.route.clone(),
        UsageSettlementKind::AssistantAttempt => {
            event.route.clone().or_else(|| current_route.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::collectors::deepseek_harness::event_parser::UsageEvent;

    fn tokens(total: u64) -> TokenUsage {
        TokenUsage::new(Some(total - 1), Some(1), Some(0), Some(0), total)
            .expect("valid token usage")
    }

    fn usage_with_kind(
        seq: u64,
        turn: u64,
        step: u64,
        total: u64,
        kind: UsageSettlementKind,
        route: Option<SessionModelRoute>,
    ) -> SessionEvent {
        SessionEvent::Usage(UsageEvent {
            seq,
            observed_at_ms: 1_000 + seq as i64,
            turn,
            step,
            kind,
            route,
            tokens: tokens(total),
        })
    }

    fn usage(
        seq: u64,
        turn: u64,
        step: u64,
        total: u64,
        route: Option<SessionModelRoute>,
    ) -> SessionEvent {
        usage_with_kind(
            seq,
            turn,
            step,
            total,
            UsageSettlementKind::AssistantMessage,
            route,
        )
    }

    fn attempt_usage(seq: u64, turn: u64, step: u64, total: u64) -> SessionEvent {
        usage_with_kind(
            seq,
            turn,
            step,
            total,
            UsageSettlementKind::AssistantAttempt,
            None,
        )
    }

    fn route(provider: &str, model: &str) -> SessionModelRoute {
        SessionModelRoute {
            provider: provider.to_owned(),
            model: model.to_owned(),
        }
    }

    #[test]
    fn replaces_usage_for_the_same_turn_and_step() {
        let events = [usage(1, 1, 1, 10, None), usage(2, 1, 1, 20, None)];

        let observations = fold_events(&events);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].seq, 2);
        assert_eq!(observations[0].tokens.total_tokens(), 20);
    }

    #[test]
    fn deduplicates_identical_usage_for_the_same_turn_and_step() {
        let events = [usage(1, 1, 1, 10, None), usage(2, 1, 1, 10, None)];

        let observations = fold_events(&events);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].seq, 1);
    }

    #[test]
    fn keeps_retried_attempts_as_separate_contributions() {
        let events = [
            usage(1, 1, 1, 10, None),
            SessionEvent::RetryStarted { turn: 1, step: 1 },
            usage(2, 1, 1, 10, None),
        ];

        let observations = fold_events(&events);

        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].seq, 1);
        assert_eq!(observations[1].seq, 2);
    }

    #[test]
    fn keeps_usage_for_different_steps_separate() {
        let events = [usage(1, 1, 1, 10, None), usage(2, 1, 2, 20, None)];

        let observations = fold_events(&events);

        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].step, 1);
        assert_eq!(observations[1].step, 2);
    }

    #[test]
    fn applies_latest_route_to_attempts_without_a_message_route() {
        let events = [
            SessionEvent::Route(route("provider", "model")),
            attempt_usage(1, 1, 1, 10),
        ];

        let observations = fold_events(&events);

        assert_eq!(observations[0].route, Some(route("provider", "model")));
    }

    #[test]
    fn message_without_route_does_not_fall_back_to_request_route() {
        let events = [
            SessionEvent::Route(route("provider", "model")),
            usage(1, 1, 1, 10, None),
        ];

        let observations = fold_events(&events);

        assert_eq!(observations[0].route, None);
    }

    #[test]
    fn message_route_wins_over_current_request_route() {
        let events = [
            SessionEvent::Route(route("current-provider", "current-model")),
            usage(
                1,
                1,
                1,
                10,
                Some(route("message-provider", "message-model")),
            ),
        ];

        let observations = fold_events(&events);

        assert_eq!(
            observations[0].route,
            Some(route("message-provider", "message-model"))
        );
    }
}
