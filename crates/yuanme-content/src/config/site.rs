//! `[site]`: what the Site says of itself.

use super::check::Checker;
use super::{Table, string, to_value};

#[derive(Debug)]
pub struct Site {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub author: Option<String>,
    pub timezone: String,
    pub extra: toml::Table,
}

pub(super) fn read(site: &Table<'_>, checker: &mut Checker) -> Site {
    if let Some(tz) = site.get("timezone") {
        let name = tz.get_ref().as_str().expect("checked");
        // The bundled database, so a build never depends on the machine's.
        if jiff::tz::TimeZone::get(name).is_err() {
            checker.invalid(
                format!("`{name}` is not an IANA time zone, such as \"Asia/Singapore\" or \"UTC\""),
                tz.span(),
            );
        }
    }
    let extra = match site.get("extra").map(|e| to_value(e.get_ref())) {
        Some(toml::Value::Table(extra)) => extra,
        Some(_) => unreachable!("checked as a table"),
        None => toml::Table::new(),
    };
    Site {
        url: string(site, "url").expect("required"),
        title: string(site, "title"),
        description: string(site, "description"),
        language: string(site, "language"),
        author: string(site, "author"),
        timezone: string(site, "timezone").unwrap_or_else(|| "UTC".into()),
        extra,
    }
}
