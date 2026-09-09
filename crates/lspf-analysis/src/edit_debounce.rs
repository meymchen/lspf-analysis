//! Per-document quiet periods shared by publication and on-demand analysis.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lspf::CancellationToken;
use tokio::time::Instant;

const EDIT_DELAY: Duration = Duration::from_millis(500);

pub(crate) struct PendingEdit {
    pub(crate) deadline: Instant,
    pub(crate) cancelled: CancellationToken,
}

#[derive(Default)]
pub(crate) struct EditDebounce(Mutex<HashMap<String, Arc<PendingEdit>>>);

impl EditDebounce {
    pub(crate) fn schedule(&self, uri: &str) -> Arc<PendingEdit> {
        let edit = Arc::new(PendingEdit {
            deadline: Instant::now() + EDIT_DELAY,
            cancelled: CancellationToken::new(),
        });
        if let Some(old) = self
            .0
            .lock()
            .expect("edit debounce lock")
            .insert(uri.into(), Arc::clone(&edit))
        {
            old.cancelled.cancel();
        }
        edit
    }

    pub(crate) fn cancel(&self, uri: &str) {
        if let Some(edit) = self.0.lock().expect("edit debounce lock").remove(uri) {
            edit.cancelled.cancel();
        }
    }

    pub(crate) fn finish(&self, uri: &str, edit: &Arc<PendingEdit>) {
        let mut pending = self.0.lock().expect("edit debounce lock");
        if pending
            .get(uri)
            .is_some_and(|current| Arc::ptr_eq(current, edit))
        {
            pending.remove(uri);
        }
    }

    pub(crate) async fn wait(&self, uri: &str) {
        loop {
            let edit = self.0.lock().expect("edit debounce lock").get(uri).cloned();
            let Some(edit) = edit else { return };
            if edit.deadline <= Instant::now() {
                return;
            }
            tokio::select! {
                () = edit.cancelled.cancelled() => {},
                () = tokio::time::sleep_until(edit.deadline) => {},
            }
        }
    }
}

impl Drop for EditDebounce {
    fn drop(&mut self) {
        for edit in self.0.get_mut().expect("edit debounce lock").values() {
            edit.cancelled.cancel();
        }
    }
}
