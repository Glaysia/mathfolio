//! Area-local inert result presentation. Source/default changes parse formats
//! once; preparing views never evaluates input or mutates worksheet XML (0135).
use super::area_ui::{AreaChildView, child_key};
use super::{
    MathFormat, RegionEvaluation, SavedCalculationRow, SavedCalculationStatus, numeric_cache,
};
use crate::result_display::{self, FormattedResult};
use mathcraft_document::native_visual::AreaChildVisualEntry;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Default)]
pub(super) struct AreaPresentationCache {
    source: Arc<str>,
    defaults: MathFormat,
    formats: HashMap<String, MathFormat>,
    initialized: bool,
    failure: Option<String>,
    #[cfg(test)]
    parses: usize,
}

impl AreaPresentationCache {
    /// No per-frame XML clone/parse: copy source only when its bytes change.
    /// Invalid input is remembered too, until source or defaults changes.
    pub(super) fn refresh(&mut self, source: &str, defaults: MathFormat) -> Result<bool, String> {
        if self.initialized && self.source.as_ref() == source && self.defaults == defaults {
            return self.failure.clone().map_or(Ok(false), Err);
        }
        if self.source.as_ref() != source {
            self.source = Arc::from(source);
        }
        self.defaults = defaults;
        self.initialized = true;
        #[cfg(test)]
        {
            self.parses += 1;
        }
        let read = || -> Result<HashMap<String, MathFormat>, String> {
            let mut formats = HashMap::new();
            for child in mathcraft_document::area::area_math_children_xml(source, &[])? {
                let doc = roxmltree::Document::parse(&child.math_xml).map_err(|e| e.to_string())?;
                formats.insert(
                    child_key(&child.region_path),
                    mathcraft_document::formatting::read_math_format_with_defaults(
                        doc.root_element(),
                        defaults,
                    ),
                );
            }
            Ok(formats)
        };
        match read() {
            Ok(formats) => {
                self.formats = formats;
                self.failure = None;
                Ok(true)
            }
            Err(error) => {
                self.formats.clear();
                self.failure = Some(error.clone());
                Err(error)
            }
        }
    }

    pub(super) fn prepare(
        &self,
        owner: u64,
        live: &HashMap<String, RegionEvaluation>,
        saved: &HashMap<String, SavedCalculationRow>,
        pending: &HashSet<(u64, String)>,
    ) -> PreparedAreaViews {
        let mut rows = HashMap::with_capacity(self.formats.len());
        for (key, format) in &self.formats {
            let result = live.get(key);
            let fresh_error = result.and_then(|r| r.error.as_ref());
            let fresh_value = result.and_then(|r| r.display_value.as_ref().or(r.value.as_ref()));
            let fallback = saved.get(key);
            let fresh = fresh_error.is_some() || fresh_value.is_some();
            let error = if let Some(error) = fresh_error {
                Some(error.detail.clone())
            } else if !fresh {
                fallback.and_then(|row| row.error.clone())
            } else {
                None
            };
            let display = if error.is_some() {
                None
            } else {
                fresh_value
                    .map(|v| result_display::format(v, format))
                    .or_else(|| {
                        fallback.and_then(|row| {
                            numeric_cache::display(row, format).or_else(|| {
                                row.display.as_deref().and_then(FormattedResult::from_saved)
                            })
                        })
                    })
            };
            let is_pending = pending.contains(&(owner, key.clone()))
                || (!fresh
                    && fallback.is_some_and(|row| row.status == SavedCalculationStatus::Pending));
            rows.insert(
                key.clone(),
                PreparedAreaRow {
                    display,
                    error,
                    pending: is_pending,
                    font_points: format.font_points,
                },
            );
        }
        PreparedAreaViews { rows }
    }
}

struct PreparedAreaRow {
    display: Option<FormattedResult>,
    error: Option<String>,
    pending: bool,
    font_points: f32,
}

pub(super) struct PreparedAreaViews {
    rows: HashMap<String, PreparedAreaRow>,
}
impl PreparedAreaViews {
    /// The owned result rows outlive borrowed widget views. Native entries are
    /// borrowed from the open-time visual cache, never a self-referential map.
    pub(super) fn borrow_views<'a>(
        &'a self,
        native_entries: &'a [AreaChildVisualEntry],
    ) -> HashMap<String, AreaChildView<'a>> {
        let mut views: HashMap<_, _> = self
            .rows
            .iter()
            .map(|(key, row)| {
                (
                    key.clone(),
                    AreaChildView {
                        display: row.display.as_ref(),
                        error: row.error.as_deref(),
                        pending: row.pending,
                        font_points: Some(row.font_points),
                        ..Default::default()
                    },
                )
            })
            .collect();
        for entry in native_entries {
            let view = views.entry(entry.key.clone()).or_default();
            view.local = Some([entry.left, entry.top]);
            view.kind = Some(&entry.kind);
            match &entry.visual {
                Ok(visual) => view.native = Some(visual),
                Err(error) if view.error.is_none() && view.display.is_none() => {
                    view.error = Some(error)
                }
                Err(_) => {}
            }
        }
        views
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mathcraft_compat::{Fault, Value};
    use mathcraft_core::Quantity;
    const KEY: &str = "[0,0,0]";
    fn xml() -> String {
        r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50"><Area><regions><region region-id="7" left="9.6" top="4"><math resultRef="2"><ml:eval><ml:real>1</ml:real></ml:eval><formattingOverride FontSize="22"/><resultFormat><decimal precision="2"/></resultFormat></math></region></regions></Area></region>"#.into()
    }
    fn value(n: f64) -> Value {
        Value::Scalar {
            quantity: Quantity::scalar(n),
        }
    }
    fn live(n: Option<f64>) -> RegionEvaluation {
        RegionEvaluation {
            region_id: "7".into(),
            result_ref: None,
            left: 9.6,
            top: 4.,
            value: n.map(value),
            display_value: None,
            error: None,
            redefinition_warning: false,
            matrix_origin: None,
        }
    }
    fn saved(n: f64) -> SavedCalculationRow {
        SavedCalculationRow {
            region_id: 7,
            status: SavedCalculationStatus::Synchronized,
            display: Some(result_display::format(&value(n), &MathFormat::default()).to_saved()),
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            numeric_source: numeric_cache::source(&value(n)),
        }
    }
    fn plain(views: &HashMap<String, AreaChildView<'_>>) -> String {
        views[KEY].display.unwrap().cells[0].plain()
    }

    #[test]
    fn source_and_defaults_change_formats_once_and_failed_source_is_cached() {
        let mut cache = AreaPresentationCache::default();
        let source = xml();
        assert!(cache.refresh(&source, MathFormat::default()).unwrap());
        let source_arc = Arc::clone(&cache.source);
        assert!(!cache.refresh(&source, MathFormat::default()).unwrap());
        assert_eq!(cache.parses, 1);
        assert!(Arc::ptr_eq(&source_arc, &cache.source));
        let defaults = MathFormat {
            font_points: 18.,
            ..MathFormat::default()
        };
        assert!(cache.refresh(&source, defaults).unwrap());
        assert_eq!(cache.parses, 2);
        assert!(Arc::ptr_eq(&source_arc, &cache.source));
        assert_eq!(cache.formats[KEY].font_points, 22.);
        assert!(cache.refresh("<broken", defaults).is_err());
        assert!(cache.refresh("<broken", defaults).is_err());
        assert_eq!(cache.parses, 3);
        assert!(cache.formats.is_empty());
    }
    #[test]
    fn live_priority_saved_precision_fallback_pending_and_owner_isolation() {
        let mut cache = AreaPresentationCache::default();
        cache.refresh(&xml(), MathFormat::default()).unwrap();
        let saved = HashMap::from([(KEY.into(), saved(1.23456))]);
        let mut pending = HashSet::new();
        pending.insert((9, KEY.into()));
        let prepared = cache.prepare(9, &HashMap::new(), &saved, &pending);
        let views = prepared.borrow_views(&[]);
        assert_eq!(plain(&views), "1.23");
        assert!(views[KEY].pending);
        assert_eq!(views[KEY].font_points, Some(22.));
        let fresh = HashMap::from([(KEY.into(), live(Some(4.567)))]);
        let prepared = cache.prepare(8, &fresh, &saved, &pending);
        let views = prepared.borrow_views(&[]);
        assert_eq!(plain(&views), "4.57");
        assert!(!views[KEY].pending);
        let mut fault = live(None);
        fault.error = Some(Fault {
            code: "test".into(),
            detail: "fresh failure".into(),
            previous_region: None,
            unsupported: false,
        });
        let prepared = cache.prepare(9, &HashMap::from([(KEY.into(), fault)]), &saved, &pending);
        let views = prepared.borrow_views(&[]);
        assert!(views[KEY].display.is_none());
        assert_eq!(views[KEY].error, Some("fresh failure"));
    }
    #[test]
    fn saved_formatted_fallback_and_native_entries_borrow_without_source_mutation() {
        let source = xml();
        let mut cache = AreaPresentationCache::default();
        cache.refresh(&source, MathFormat::default()).unwrap();
        let mut row = saved(7.);
        row.numeric_source = None;
        row.status = SavedCalculationStatus::Pending;
        let prepared = cache.prepare(
            1,
            &HashMap::new(),
            &HashMap::from([(KEY.into(), row)]),
            &HashSet::new(),
        );
        let entries = vec![AreaChildVisualEntry {
            region_path: vec![0, 0, 1],
            key: "[0,0,1]".into(),
            left: 3.,
            top: 8.,
            kind: "text".into(),
            visual: Ok(mathcraft_document::native_visual::RegionVisual {
                size: [40., 20.],
                background: None,
                document: None,
            }),
        }];
        let views = prepared.borrow_views(&entries);
        assert_eq!(plain(&views), "7");
        assert!(views[KEY].pending);
        let nonmath = &views["[0,0,1]"];
        assert_eq!(nonmath.local, Some([3., 8.]));
        assert_eq!(nonmath.kind, Some("text"));
        assert!(std::ptr::eq(
            nonmath.native.unwrap(),
            entries[0].visual.as_ref().unwrap()
        ));
        assert_eq!(cache.source.as_ref(), source);
    }
}
