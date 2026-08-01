use super::{CardinalityDescriptor, CardinalityError, CardinalityOutcome};

pub(super) const DESCRIPTOR: CardinalityDescriptor = CardinalityDescriptor { key: "all" };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct All;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllPlan;

pub(super) const fn compile(_: All) -> AllPlan {
    AllPlan
}

pub(super) fn execute<T>(
    _: AllPlan,
    values: Vec<T>,
) -> Result<CardinalityOutcome<T>, CardinalityError> {
    Ok(CardinalityOutcome::Sequence(values))
}
