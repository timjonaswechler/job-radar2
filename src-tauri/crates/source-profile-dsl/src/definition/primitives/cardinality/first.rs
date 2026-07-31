use super::{CardinalityDescriptor, CardinalityError, CardinalityOutcome};

pub(super) const DESCRIPTOR: CardinalityDescriptor = CardinalityDescriptor { key: "first" };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct First;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirstPlan;

pub(super) const fn compile(_: First) -> FirstPlan {
    FirstPlan
}

pub(super) fn execute<T>(
    _: FirstPlan,
    values: Vec<T>,
) -> Result<CardinalityOutcome<T>, CardinalityError> {
    Ok(CardinalityOutcome::Scalar(values.into_iter().next()))
}
