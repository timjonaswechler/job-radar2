use super::{mismatch, CardinalityDescriptor, CardinalityError, CardinalityOutcome};

pub(super) const DESCRIPTOR: CardinalityDescriptor = CardinalityDescriptor { key: "optional" };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Optional;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptionalPlan;

pub(super) const fn compile(_: Optional) -> OptionalPlan {
    OptionalPlan
}

pub(super) fn execute<T>(
    _: OptionalPlan,
    mut values: Vec<T>,
) -> Result<CardinalityOutcome<T>, CardinalityError> {
    match values.len() {
        0 => Ok(CardinalityOutcome::Scalar(None)),
        1 => Ok(CardinalityOutcome::Scalar(values.pop())),
        count => Err(mismatch("optional", count)),
    }
}
