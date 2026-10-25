//! Sidecar selectors scoped to their Template (ADR 0006), following the
//! spike on `spike/scoped-styles`: the Template's attribute goes on every
//! compound selector of a Sidecar's rules, and `:global()` is unwrapped.

use std::collections::BTreeMap;

use lightningcss::css_modules::{Config, Pattern};
use lightningcss::rules::{CssRule, CssRuleList, Location as RuleLocation};
use lightningcss::selector::{Combinator, Component, PseudoClass, Selector};

use super::Location;

/// A compound selector the attribute went on, for `check` to match against
/// the tags its Template writes.
#[derive(Debug)]
pub struct Compound {
    /// The rule it is in: its stylesheet, line and column, as in
    /// [`Location`].
    pub at: Location,
    /// Lowercase.
    pub name: Option<String>,
    pub classes: Vec<String>,
    pub ids: Vec<String>,
}

/// `:global()` only parses with CSS modules on. `[local]` and every
/// renaming switched off keep each name as written: keyframe, container
/// and layer names stay global, as in Astro, since Scripts name them.
pub(super) fn modules() -> Config {
    Config {
        pattern: Pattern::parse("[local]").expect("a valid pattern"),
        dashed_idents: false,
        animation: false,
        grid: false,
        custom_idents: false,
        container: false,
        pure: false,
    }
}

/// Scopes the rules of each stylesheet `scopes` names by the attribute it
/// gives, `sources` naming each rule's stylesheet by its source index.
/// `:global()` is unwrapped in every rule. Rules in `@scope` are left as
/// written: native scoping is the Author's opt-in.
pub(super) fn scope(
    rules: &mut CssRuleList<'_>,
    sources: &[String],
    scopes: &BTreeMap<String, String>,
    compounds: &mut Vec<Compound>,
) -> Result<(), (String, Location)> {
    for rule in rules.0.iter_mut() {
        let (selectors, nested, loc) = match rule {
            CssRule::Style(r) => (Some(&mut r.selectors.0), &mut r.rules, r.loc),
            CssRule::Nesting(r) => (Some(&mut r.style.selectors.0), &mut r.style.rules, r.loc),
            CssRule::Media(r) => (None, &mut r.rules, r.loc),
            CssRule::Supports(r) => (None, &mut r.rules, r.loc),
            CssRule::Container(r) => (None, &mut r.rules, r.loc),
            CssRule::LayerBlock(r) => (None, &mut r.rules, r.loc),
            CssRule::StartingStyle(r) => (None, &mut r.rules, r.loc),
            CssRule::MozDocument(r) => (None, &mut r.rules, r.loc),
            _ => continue,
        };
        let file = &sources[loc.source_index as usize];
        let at = location(file, loc);
        let attribute = scopes.get(file).map(String::as_str);
        for selector in selectors.into_iter().flatten() {
            scope_selector(selector, attribute, &at, compounds)?;
        }
        scope(nested, sources, scopes, compounds)?;
    }
    Ok(())
}

fn location(file: &str, loc: RuleLocation) -> Location {
    Location {
        file: file.to_string(),
        line: loc.line + 1,
        column: loc.column,
    }
}

/// Adds `[attribute]` to every compound of `selector` but those that are
/// only `:global()`, `:root`, or hold `&` (whose parent rule has the
/// attribute already), and unwraps `:global()`. A pseudo-element stays
/// last: `.a::before` becomes `.a[h]::before`. Arguments of `:is()`,
/// `:not()`, `:has()` and `:where()` are left alone, so `:has(img)` can see
/// content; the element styled is still the Template's own.
fn scope_selector<'i>(
    selector: &mut Selector<'i>,
    attribute: Option<&str>,
    at: &Location,
    compounds: &mut Vec<Compound>,
) -> Result<(), (String, Location)> {
    // Each compound in parse order, and the combinator after it.
    // `iter_raw_parse_order_from` yields compounds left to right, each one's
    // simple selectors reversed, and `Selector::from` wants them in order:
    // the #19 probe skipped that and printed `:focus-visiblea[h]`.
    let mut segments: Vec<(Vec<Component<'i>>, Option<Component<'i>>)> = Vec::new();
    let mut compound: Vec<Component<'i>> = Vec::new();
    for c in selector.iter_raw_parse_order_from(0).cloned() {
        if c.as_combinator().is_some() {
            compound.reverse();
            segments.push((std::mem::take(&mut compound), Some(c)));
        } else {
            compound.push(c);
        }
    }
    compound.reverse();
    segments.push((compound, None));

    let is_global =
        |c: &Component<'_>| matches!(c, Component::NonTSPseudoClass(PseudoClass::Global { .. }));
    let before_pseudo = |c: &Option<Component<'_>>| {
        matches!(
            c.as_ref().and_then(Component::as_combinator),
            Some(Combinator::PseudoElement | Combinator::Part | Combinator::SlotAssignment)
        )
    };
    let mut changed = false;
    let mut out: Vec<Component<'i>> = Vec::new();
    let mut after_pseudo = false;
    for (mut compound, combinator) in segments {
        if compound
            .iter()
            .any(|c| matches!(c, Component::NonTSPseudoClass(PseudoClass::Local { .. })))
        {
            return Err((
                "`:local()` isn't supported: a Sidecar's selectors are scoped already".into(),
                at.clone(),
            ));
        }
        let only_global = !compound.is_empty() && compound.iter().all(is_global);
        let skip = compound
            .iter()
            .any(|c| matches!(c, Component::Nesting | Component::Root));
        // `::selection` alone is an empty compound before its pseudo-element.
        let scoped = !after_pseudo
            && (!compound.is_empty() || before_pseudo(&combinator))
            && !only_global
            && !skip;
        if let (true, Some(attribute)) = (scoped, attribute) {
            compounds.push(Compound {
                at: at.clone(),
                name: compound.iter().find_map(|c| match c {
                    Component::LocalName(n) => Some(n.lower_name.to_string()),
                    _ => None,
                }),
                classes: compound
                    .iter()
                    .filter_map(|c| match c {
                        Component::Class(c) => Some(c.to_string()),
                        _ => None,
                    })
                    .collect(),
                ids: compound
                    .iter()
                    .filter_map(|c| match c {
                        Component::ID(c) => Some(c.to_string()),
                        _ => None,
                    })
                    .collect(),
            });
            compound.push(Component::AttributeInNoNamespaceExists {
                local_name: attribute.to_string().into(),
                local_name_lower: attribute.to_string().into(),
            });
            changed = true;
        }
        after_pseudo = before_pseudo(&combinator);
        // Left in place, `:global(x)` prints as `x`, but lightningcss's
        // compatibility check doesn't know it: at older targets it wraps
        // the selector list in `:is()`, whose specificity is its highest
        // argument's.
        if compound.iter().any(is_global) {
            let mut flat: Vec<Component<'i>> = Vec::new();
            for c in compound {
                let Component::NonTSPseudoClass(PseudoClass::Global { selector }) = c else {
                    flat.push(c);
                    continue;
                };
                let mut inner: Vec<Component<'i>> = Vec::new();
                for c in selector.iter_raw_parse_order_from(0).cloned() {
                    if c.as_combinator().is_some() {
                        inner.reverse();
                        flat.append(&mut inner);
                        flat.push(c);
                    } else {
                        inner.push(c);
                    }
                }
                inner.reverse();
                flat.append(&mut inner);
            }
            compound = flat;
            changed = true;
        }
        out.append(&mut compound);
        out.extend(combinator);
    }
    if changed {
        *selector = Selector::from(out);
    }
    Ok(())
}
