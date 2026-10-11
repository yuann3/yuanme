pub struct Seed {
    pub what: &'static str,
    pub file: &'static str,
    pub find: &'static str,
    pub replace: &'static str,
    pub warn_ok: bool,
}
pub fn seeds() -> Vec<Seed> {
    vec![]
}
