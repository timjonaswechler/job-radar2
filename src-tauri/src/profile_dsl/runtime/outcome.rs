use serde::{Deserialize, Serialize};

use crate::profile_dsl::{
    diagnostics::Diagnostics,
    occurrence::{
        DetailContributionEvidence, DetailPatch, DetailRejection, DiscoveryContributionEvidence,
        DiscoveryRejection, PostingOccurrence,
    },
};

use super::allowance::PhaseExecutionReport;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyUnsatisfiedCause {
    RejectedOnly,
    IncludesExecutionFailure,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PolicyOutcome<P> {
    Accepted { reduced_payload: P },
    PolicyUnsatisfied { cause: PolicyUnsatisfiedCause },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseExecutionFailure {
    Internal,
    InvalidCallerLimits,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PhaseOutcome<P> {
    Completed {
        policy_outcome: PolicyOutcome<P>,
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    BudgetExhausted {
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    ExecutionFailed {
        typed_failure: PhaseExecutionFailure,
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
}

impl<P> PhaseOutcome<P> {
    pub fn diagnostics(&self) -> &Diagnostics {
        match self {
            Self::Completed { diagnostics, .. }
            | Self::BudgetExhausted { diagnostics, .. }
            | Self::ExecutionFailed { diagnostics, .. } => diagnostics,
        }
    }

    pub fn complete_budget_report(&self) -> &PhaseExecutionReport {
        match self {
            Self::Completed {
                complete_budget_report,
                ..
            }
            | Self::BudgetExhausted {
                complete_budget_report,
                ..
            }
            | Self::ExecutionFailed {
                complete_budget_report,
                ..
            } => complete_budget_report,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseCancelled {
    pub complete_budget_report: PhaseExecutionReport,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhasePreStartFailure {
    PlanMismatch,
    RequestMismatch,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum PhaseRunError {
    NotStarted {
        failure: PhasePreStartFailure,
        diagnostics: Diagnostics,
    },
    Cancelled(PhaseCancelled),
}

impl PhaseRunError {
    pub fn diagnostics(&self) -> &Diagnostics {
        match self {
            Self::NotStarted { diagnostics, .. } => diagnostics,
            Self::Cancelled(cancelled) => &cancelled.diagnostics,
        }
    }
}

pub type PhaseRunResult<P> = Result<PhaseOutcome<P>, PhaseRunError>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryPhasePayload {
    pub candidates: Vec<PostingOccurrence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<DiscoveryContributionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<DiscoveryContributionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejections: Vec<DiscoveryRejection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailPhasePayload {
    pub patch: DetailPatch,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<DetailContributionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<DetailContributionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejections: Vec<DetailRejection>,
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::profile_dsl::runtime::allowance::{
        AllowanceDimension, AllowanceExhaustion, AllowanceLimitSource, PhaseCompletion, PhaseUsage,
    };

    fn report(completion: PhaseCompletion) -> PhaseExecutionReport {
        PhaseExecutionReport {
            usage: PhaseUsage::default(),
            completion,
        }
    }

    fn zero_usage() -> Value {
        json!({
            "strategyAttempts": 0,
            "requests": 0,
            "producedItems": 0,
            "durationMs": 0,
            "pages": 0,
            "browserActions": 0,
            "fanOut": 0,
            "responseBytes": 0,
            "browserRenderedBytes": 0
        })
    }

    #[test]
    fn whole_envelopes_use_exact_camel_case_and_only_accepted_contains_payload() {
        let accepted = PhaseOutcome::Completed {
            policy_outcome: PolicyOutcome::Accepted {
                reduced_payload: DiscoveryPhasePayload {
                    candidates: Vec::new(),
                    provenance: Vec::new(),
                    conflicts: Vec::new(),
                    rejections: Vec::new(),
                },
            },
            complete_budget_report: report(PhaseCompletion::Accepted),
            diagnostics: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(accepted).unwrap(),
            json!({
                "type": "completed",
                "policyOutcome": { "type": "accepted", "reducedPayload": { "candidates": [] } },
                "completeBudgetReport": { "usage": zero_usage(), "completion": { "type": "accepted" } },
                "diagnostics": []
            })
        );

        let unsatisfied = PhaseOutcome::<DiscoveryPhasePayload>::Completed {
            policy_outcome: PolicyOutcome::PolicyUnsatisfied {
                cause: PolicyUnsatisfiedCause::RejectedOnly,
            },
            complete_budget_report: report(PhaseCompletion::PolicyUnsatisfied),
            diagnostics: Vec::new(),
        };
        let unsatisfied = serde_json::to_value(unsatisfied).unwrap();
        assert_eq!(
            unsatisfied,
            json!({
                "type": "completed",
                "policyOutcome": { "type": "policy_unsatisfied", "cause": "rejected_only" },
                "completeBudgetReport": { "usage": zero_usage(), "completion": { "type": "policy_unsatisfied" } },
                "diagnostics": []
            })
        );

        let exhaustion = AllowanceExhaustion {
            dimension: AllowanceDimension::Requests,
            requested: 1,
            remaining: 0,
            limit_sources: vec![AllowanceLimitSource::Backend],
        };
        let budget = PhaseOutcome::<DiscoveryPhasePayload>::BudgetExhausted {
            complete_budget_report: report(PhaseCompletion::BudgetExhausted {
                exhaustion: exhaustion.clone(),
            }),
            diagnostics: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(budget).unwrap(),
            json!({
                "type": "budget_exhausted",
                "completeBudgetReport": {
                    "usage": zero_usage(),
                    "completion": { "type": "budget_exhausted", "exhaustion": {
                        "dimension": "requests", "requested": 1, "remaining": 0,
                        "limitSources": ["backend"]
                    }}
                },
                "diagnostics": []
            })
        );

        let failure = PhaseOutcome::<DetailPhasePayload>::ExecutionFailed {
            typed_failure: PhaseExecutionFailure::Internal,
            complete_budget_report: report(PhaseCompletion::ExecutionFailed),
            diagnostics: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(failure).unwrap(),
            json!({
                "type": "execution_failed",
                "typedFailure": "internal",
                "completeBudgetReport": { "usage": zero_usage(), "completion": { "type": "execution_failed" } },
                "diagnostics": []
            })
        );

        let cancelled = PhaseRunError::Cancelled(PhaseCancelled {
            complete_budget_report: report(PhaseCompletion::Cancelled {
                reason: super::super::allowance::PhaseCancellationReason::UserCancelled,
            }),
            diagnostics: Vec::new(),
        });
        assert_eq!(
            serde_json::to_value(cancelled).unwrap(),
            json!({ "cancelled": {
                "completeBudgetReport": { "usage": zero_usage(), "completion": {
                    "type": "cancelled", "reason": "user_cancelled"
                }},
                "diagnostics": []
            }})
        );

        let not_started = PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            diagnostics: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(not_started).unwrap(),
            json!({ "not_started": { "failure": "plan_mismatch", "diagnostics": [] } })
        );

        let payload_free = serde_json::to_string(&(unsatisfied, exhaustion)).unwrap();
        for forbidden in [
            "candidates",
            "patch",
            "provenance",
            "conflicts",
            "rejections",
            "disposition",
        ] {
            assert!(!payload_free.contains(forbidden), "leaked {forbidden}");
        }
    }
}
