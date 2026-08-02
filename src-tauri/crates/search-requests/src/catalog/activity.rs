use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use super::{Error, Id};

#[derive(Default)]
pub(super) struct Activity {
    busy: Mutex<HashSet<Id>>,
}

impl Activity {
    pub(super) fn reserve(self: &Arc<Self>, id: Id) -> Result<Reservation, Error> {
        let mut busy = self.busy.lock().map_err(|_| Error::InternalInvariant {
            message: "Search Request activity state is unavailable".into(),
        })?;
        if !busy.insert(id) {
            return Err(Error::Busy { id });
        }
        Ok(Reservation {
            activity: Arc::clone(self),
            id,
        })
    }

    fn release(&self, id: Id) {
        if let Ok(mut busy) = self.busy.lock() {
            busy.remove(&id);
        }
    }
}

pub(super) struct Reservation {
    activity: Arc<Activity>,
    id: Id,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.activity.release(self.id);
    }
}
