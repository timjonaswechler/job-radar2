use source_engine::definition::Diagnostics;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SourceFailure {
    message: String,
    diagnostics: Diagnostics,
}

impl SourceFailure {
    pub(super) fn new(message: String, diagnostics: Diagnostics) -> Self {
        Self {
            message,
            diagnostics,
        }
    }

    pub(super) fn message(&self) -> String {
        self.message.clone()
    }

    pub(super) fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.clone()
    }
}
