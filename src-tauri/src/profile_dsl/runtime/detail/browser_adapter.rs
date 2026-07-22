use super::super::{
    browser_acquisition::BrowserAcquisition,
    browser_phase::{
        execute_canonical_browser_fetch, BrowserPhaseFetchInput, BrowserPhaseFetchProjection,
    },
    cancellation::RuntimePhase,
};

#[derive(Clone, Copy)]
pub struct DetailBrowserAdapter<'a> {
    acquisition: &'a dyn BrowserAcquisition,
}

impl<'a> DetailBrowserAdapter<'a> {
    pub fn new(acquisition: &'a dyn BrowserAcquisition) -> Self {
        Self { acquisition }
    }

    pub(crate) async fn fetch(
        &self,
        input: BrowserPhaseFetchInput<'_>,
    ) -> BrowserPhaseFetchProjection {
        execute_canonical_browser_fetch(self.acquisition, RuntimePhase::Detail, input).await
    }
}
