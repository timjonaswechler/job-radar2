use std::{future::Future, pin::Pin};

use super::ResolvedLocation;

pub type GeoResolveFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<ResolvedLocation>, String>> + Send + 'a>>;

pub trait GeoResolver: Sync {
    fn resolve<'a>(&'a self, input: &'a str) -> GeoResolveFuture<'a>;
}
