use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::combinator::all_consuming;
use nom::combinator::consumed;
use nom::combinator::map_parser;
use nom::combinator::verify;
use nom::multi::many1;

use super::object_parser::minimal_set_object;
use super::org_source::OrgSource;
use super::util::confine_context;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use super::util::text_until_exit;
use crate::context::parser_with_context;
use crate::context::Context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::List;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::types::Object;
use crate::types::RadioLink;
use crate::types::RadioTarget;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn radio_link<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, RadioLink<'s>> {
    for radio_target in &context.get_global_settings().radio_targets {
        let rematched_target = rematch_target(context, radio_target, input);
        if let Ok((remaining, rematched_target)) = rematched_target {
            let path = get_consumed(input, remaining);
            let (remaining, post_blank) = space0(remaining)?;
            let source = get_consumed(input, remaining);
            return Ok((
                remaining,
                RadioLink {
                    source: source.into(),
                    children: rematched_target,
                    path: path.into(),
                    post_blank: if post_blank.len() > 0 {
                        Some(Into::<&str>::into(post_blank))
                    } else {
                        None
                    },
                },
            ));
        }
    }
    Err(nom::Err::Error(CustomError::Static("NoRadioLink")))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn rematch_target<'x, 'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    target: &'x Vec<Object<'x>>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Object<'s>>> {
    let mut remaining = input;
    let mut new_matches = Vec::with_capacity(target.len());
    for original_object in target {
        match original_object {
            // TODO: The rest of the minimal set of objects.
            Object::Bold(bold) => {
                let (new_remaining, new_match) = bold.rematch_object(context, remaining)?;
                remaining = new_remaining;
                new_matches.push(new_match);
            }
            Object::Italic(italic) => {
                let (new_remaining, new_match) = italic.rematch_object(context, remaining)?;
                remaining = new_remaining;
                new_matches.push(new_match);
            }
            Object::Underline(underline) => {
                let (new_remaining, new_match) = underline.rematch_object(context, remaining)?;
                remaining = new_remaining;
                new_matches.push(new_match);
            }
            Object::StrikeThrough(strikethrough) => {
                let (new_remaining, new_match) =
                    strikethrough.rematch_object(context, remaining)?;
                remaining = new_remaining;
                new_matches.push(new_match);
            }
            Object::PlainText(plaintext) => {
                let (new_remaining, new_match) = plaintext.rematch_object(context, remaining)?;
                remaining = new_remaining;
                new_matches.push(new_match);
            }
            _ => {
                return Err(nom::Err::Error(CustomError::Static(
                    "OnlyMinimalSetObjectsAllowed",
                )));
            }
        };
    }
    Ok((remaining, new_matches))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn radio_target<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, RadioTarget<'s>> {
    let (remaining, _opening) = tag("<<<")(input)?;
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &radio_target_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let initial_context = ContextElement::document_context();
    let initial_context = Context::new(context.get_global_settings(), List::new(&initial_context));

    let (remaining, (raw_value, children)) = consumed(map_parser(
        verify(
            parser_with_context!(text_until_exit)(&parser_context),
            |text| text.len() > 0,
        ),
        confine_context(|i| {
            all_consuming(many1(parser_with_context!(minimal_set_object)(
                &initial_context,
            )))(i)
        }),
    ))(remaining)?;

    let (remaining, _closing) = tag(">>>")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        RadioTarget {
            source: source.into(),
            value: raw_value.into(),
            post_blank: post_blank.map(Into::<&str>::into),
            children,
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn radio_target_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alt((tag("<"), tag(">"), line_ending))(input)
}

pub(crate) trait RematchObject<'x> {
    fn rematch_object<'b, 'g, 'r, 's>(
        &'x self,
        context: RefContext<'b, 'g, 'r, 's>,
        input: OrgSource<'s>,
    ) -> Res<OrgSource<'s>, Object<'s>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::GlobalSettings;
    use crate::parser::element_parser::element;
    use crate::types::Bold;
    use crate::types::Element;
    use crate::types::PlainText;
    use crate::types::StandardProperties;

    #[test]
    fn plain_text_radio_target() -> Result<(), Box<dyn std::error::Error>> {
        let input = OrgSource::new("foo bar baz");
        let radio_target_match = vec![Object::PlainText(PlainText { source: "bar" })];
        let global_settings = GlobalSettings {
            radio_targets: vec![&radio_target_match],
            ..Default::default()
        };
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let (remaining, first_paragraph) =
            element(true)(&initial_context, input).expect("Parse first paragraph");
        let first_paragraph = match first_paragraph {
            Element::Paragraph(paragraph) => paragraph,
            _ => panic!("Should be a paragraph!"),
        };
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(first_paragraph.get_source(), "foo bar baz");
        assert_eq!(first_paragraph.children.len(), 3);
        match first_paragraph
            .children
            .get(1)
            .expect("Len already asserted to be 3.")
        {
            Object::RadioLink(inner) => {
                assert_eq!(inner.get_source(), "bar ");
                assert_eq!(inner.path, "bar");
                assert_eq!(inner.children.len(), 1);
                let child = inner
                    .children
                    .first()
                    .expect("Length already asserted to be 1.");
                assert!(matches!(child, Object::PlainText(_)));
                assert_eq!(child.get_source(), "bar");
            }
            _ => {
                return Err("Child should be a radio link.".into());
            }
        };
        Ok(())
    }

    #[test]
    fn bold_radio_target() -> Result<(), Box<dyn std::error::Error>> {
        let input = OrgSource::new("foo *bar* baz");
        let radio_target_match = vec![Object::Bold(Bold {
            source: "*bar*",
            contents: "bar",
            post_blank: Some(" "),
            children: vec![Object::PlainText(PlainText { source: "bar" })],
        })];
        let global_settings = GlobalSettings {
            radio_targets: vec![&radio_target_match],
            ..Default::default()
        };
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));

        let (remaining, first_paragraph) =
            element(true)(&initial_context, input).expect("Parse first paragraph");
        let first_paragraph = match first_paragraph {
            Element::Paragraph(paragraph) => paragraph,
            _ => panic!("Should be a paragraph!"),
        };
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(first_paragraph.get_source(), "foo *bar* baz");
        assert_eq!(first_paragraph.children.len(), 3);
        match first_paragraph
            .children
            .get(1)
            .expect("Len already asserted to be 3.")
        {
            Object::RadioLink(inner) => {
                assert_eq!(inner.get_source(), "*bar* ");
                assert_eq!(inner.path, "*bar* ");
                assert_eq!(inner.children.len(), 1);
                let child = inner
                    .children
                    .first()
                    .expect("Length already asserted to be 1.");
                assert!(matches!(child, Object::Bold(_)));
                assert_eq!(child.get_source(), "*bar* ");
            }
            _ => {
                return Err("Child should be a radio link.".into());
            }
        };
        Ok(())
    }
}
