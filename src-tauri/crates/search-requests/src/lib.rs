//! Search Request authored lifecycle, validation, persistence, and execution admission.

mod catalog;

pub use catalog::{
    Catalog, Error, Execution, Id, Input, Record, Status, Validation, ValidationIssue,
    ValidationIssueCode,
};
