//! One shared computation per document snapshot and scoring configuration.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use lspf_analysis_core::{LANG, health::HealthConfig};
use tokio::sync::OnceCell;

use crate::{Analyzed, analyze};

struct Entry {
    text: String,
    version: Option<i32>,
    language: LANG,
    config: HealthConfig,
    result: OnceCell<Option<Arc<Analyzed>>>,
}

#[derive(Default)]
pub(crate) struct AnalysisCache(Mutex<HashMap<String, Arc<Entry>>>);

impl AnalysisCache {
    pub(crate) fn forget(&self, uri: &str) {
        self.0.lock().expect("analysis cache lock").remove(uri);
    }

    pub(crate) async fn report(
        &self,
        uri: &str,
        language: LANG,
        path: PathBuf,
        text: String,
        version: Option<i32>,
        config: HealthConfig,
    ) -> Option<Arc<Analyzed>> {
        let entry = {
            let mut entries = self.0.lock().expect("analysis cache lock");
            let entry = entries.entry(uri.to_owned()).or_insert_with(|| {
                Arc::new(Entry {
                    text: text.clone(),
                    version,
                    language,
                    config: config.clone(),
                    result: OnceCell::new(),
                })
            });
            // Text also distinguishes close/reopen cycles which reuse a version.
            if entry.text != text
                || entry.version != version
                || entry.language != language
                || entry.config != config
            {
                *entry = Arc::new(Entry {
                    text: text.clone(),
                    version,
                    language,
                    config: config.clone(),
                    result: OnceCell::new(),
                });
            }
            Arc::clone(entry)
        };
        // Old computations retain only their own entry: they cannot overwrite a
        // new revision or repopulate a document removed by didClose.
        entry
            .result
            .get_or_init(|| async move {
                let source = text.clone();
                let report =
                    tokio::task::spawn_blocking(move || analyze(language, source, &path, &config))
                        .await
                        .ok()??;
                Some(Arc::new(Analyzed {
                    report,
                    text,
                    version,
                }))
            })
            .await
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn report(
        cache: &AnalysisCache,
        text: &str,
        version: i32,
        config: HealthConfig,
    ) -> Arc<Analyzed> {
        cache
            .report(
                "file:///Box.java",
                LANG::Java,
                PathBuf::from("Box.java"),
                text.into(),
                Some(version),
                config,
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn concurrent_requests_share_analysis_and_changes_replace_it() {
        let cache = AnalysisCache::default();
        let source = "class Box { int area() { return 1; } }";
        let config = HealthConfig::default();
        let (first, concurrent) = tokio::join!(
            report(&cache, source, 1, config.clone()),
            report(&cache, source, 1, config.clone())
        );
        assert!(Arc::ptr_eq(&first, &concurrent));
        let next = report(&cache, source, 2, config.clone()).await;
        assert!(!Arc::ptr_eq(&first, &next));
        let edited = report(
            &cache,
            "class Box { int other() { return 2; } }",
            2,
            config.clone(),
        )
        .await;
        assert!(!Arc::ptr_eq(&next, &edited));
        let mut changed = config.clone();
        changed.complexity_threshold = 1.0;
        let reconfigured = report(&cache, source, 2, changed).await;
        let original_config = report(&cache, source, 2, config.clone()).await;
        assert!(!Arc::ptr_eq(&reconfigured, &original_config));
        cache.forget("file:///Box.java");
        let reopened = report(&cache, source, 2, config).await;
        assert!(!Arc::ptr_eq(&original_config, &reopened));
    }
}
