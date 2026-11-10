//! `[budgets]`: the Site budgets and Acknowledged overages.

use std::collections::BTreeMap;
use std::ops::Range;

use yuanme_diag::codes;

use super::check::Checker;
use super::{Table, Value, as_table, boolean, entries, float, int, table, text};

/// A Site budget (#15): one number every Page is measured against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Budget {
    Html,
    Css,
    Js,
    Fonts,
    MathFont,
    Requests,
}

impl Budget {
    pub const ALL: [Budget; 6] = [
        Budget::Html,
        Budget::Css,
        Budget::Js,
        Budget::Fonts,
        Budget::MathFont,
        Budget::Requests,
    ];

    /// Its key in `[budgets]`, which the build report and `budget` events
    /// name it by.
    pub fn key(self) -> &'static str {
        match self {
            Budget::Html => "html",
            Budget::Css => "css",
            Budget::Js => "js",
            Budget::Fonts => "fonts",
            Budget::MathFont => "math_font",
            Budget::Requests => "requests",
        }
    }

    /// The Starters' number (#15), which every Site gets unless its config
    /// changes it (ADR 0004).
    fn starter(self) -> u64 {
        match self {
            Budget::Html => 14_000,
            Budget::Css => 6_000,
            Budget::Js => 3_000,
            Budget::Fonts => 30_000,
            Budget::MathFont => 55_000,
            Budget::Requests => 8,
        }
    }
}

/// `[budgets]` (#15, ADR 0004). Limits are bytes of Brotli, or for
/// `requests` a count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budgets {
    /// `budgets.strict`: a breach is an error.
    pub strict: bool,
    /// Every Budget's limit, and where Site config sets it, if it does.
    pub limits: BTreeMap<Budget, (u64, Option<Range<usize>>)>,
    /// In the order written.
    pub overages: Vec<Overage>,
}

impl Budgets {
    pub fn limit(&self, budget: Budget) -> u64 {
        self.limits[&budget].0
    }

    /// How far over `budget` the Page at `url` may go.
    pub fn allowance(&self, url: &str, budget: Budget) -> u64 {
        self.overage(url, budget).map_or(0, |o| o.allowance)
    }

    pub fn overage(&self, url: &str, budget: Budget) -> Option<&Overage> {
        self.overages
            .iter()
            .find(|o| o.page == url && o.budget == budget)
    }
}

/// An Acknowledged overage: one Page's allowance above one Site budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overage {
    /// The Page's URL.
    pub page: String,
    pub page_at: Range<usize>,
    pub budget: Budget,
    pub allowance: u64,
    pub at: Range<usize>,
}

pub(super) fn budgets(budgets: Option<&Table<'_>>, checker: &mut Checker) -> Budgets {
    let limits = Budget::ALL
        .into_iter()
        .map(|b| {
            let limit = match budgets.and_then(|t| t.get(b.key())) {
                Some(v) => {
                    let path = format!("budgets.{}", b.key());
                    (amount(b, v, &path, checker), Some(v.span()))
                }
                None => (b.starter(), None),
            };
            (b, limit)
        })
        .collect();
    let mut overages = Vec::new();
    for (page, allowances) in entries(budgets.and_then(|t| table(t, "overages"))) {
        let url: &str = page.get_ref();
        if !url.starts_with('/') {
            checker.invalid(
                format!("`{url}` is not a Page's URL, which starts with `/`"),
                page.span(),
            );
        }
        let allowances = as_table(allowances);
        for b in Budget::ALL {
            let Some(v) = allowances.get(b.key()) else {
                continue;
            };
            let path = format!("budgets.overages.\"{url}\".{}", b.key());
            overages.push(Overage {
                page: url.to_string(),
                page_at: page.span(),
                budget: b,
                allowance: amount(b, v, &path, checker),
                at: v.span(),
            });
        }
    }
    Budgets {
        strict: budgets.is_some_and(|t| boolean(t, "strict")),
        limits,
        overages,
    }
}

/// kB as bytes, or a count of requests.
fn amount(budget: Budget, v: &Value<'_>, path: &str, checker: &mut Checker) -> u64 {
    if budget == Budget::Requests {
        return u64::try_from(int(v)).expect("checked: at least 0");
    }
    let kb = float(v);
    if kb < 0.0 {
        let message = format!("`{path}` must be at least 0, not {}", text(v));
        checker.error(&codes::CONFIG_OUT_OF_RANGE, message, v.span());
    }
    (kb.max(0.0) * 1000.0).round() as u64
}
