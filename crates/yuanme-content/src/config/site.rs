//! `[site]`: what the Site says of itself.

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
