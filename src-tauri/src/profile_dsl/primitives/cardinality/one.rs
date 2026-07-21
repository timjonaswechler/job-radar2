use super::{mismatch, CardinalityDescriptor, CardinalityError, CardinalityOutcome};

pub(super) const DESCRIPTOR: CardinalityDescriptor = CardinalityDescriptor { key: "one" };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct One;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnePlan;

pub(super) const fn compile(_: One) -> OnePlan {
    OnePlan
}

pub(super) fn execute<T>(
    _: OnePlan,
    mut values: Vec<T>,
) -> Result<CardinalityOutcome<T>, CardinalityError> {
    match values.len() {
        0 => Ok(CardinalityOutcome::Scalar(None)),
        1 => Ok(CardinalityOutcome::Scalar(values.pop())),
        count => Err(mismatch("one", count)),
    }
}
