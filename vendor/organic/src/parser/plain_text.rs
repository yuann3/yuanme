use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::one_of;
use nom::combinator::eof;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many1;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::radio_link::RematchObject;
use super::util::exit_matcher_parser;
use super::util::get_consumed;
use super::util::org_space_or_line_ending;
use crate::context::parser_with_context;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::types::Object;
use crate::types::PlainText;

pub(crate) fn plain_text<F>(
    end_condition: F,
) -> impl for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, PlainText<'s>>
where
    F: for<'bb, 'gg, 'rr, 'ss> Fn(
        RefContext<'bb, 'gg, 'rr, 'ss>,
        OrgSource<'ss>,
    ) -> Res<OrgSource<'ss>, ()>,
{
    move |context, input| _plain_text(&end_condition, context, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, end_condition))
)]
fn _plain_text<'b, 'g, 'r, 's, F>(
    end_condition: F,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PlainText<'s>>
where
    F: for<'bb, 'gg, 'rr, 'ss> Fn(
        RefContext<'bb, 'gg, 'rr, 'ss>,
        OrgSource<'ss>,
    ) -> Res<OrgSource<'ss>, ()>,
{
    let (remaining, source) = recognize(verify(
        many_till(
            anychar,
            peek(alt((
                parser_with_context!(exit_matcher_parser)(context),
                recognize(parser_with_context!(end_condition)(context)),
            ))),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    ))(input)?;

    Ok((
        remaining,
        PlainText {
            source: source.into(),
        },
    ))
}

impl<'x> RematchObject<'x> for PlainText<'x> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(ret, level = "debug", skip(_context))
    )]
    fn rematch_object<'b, 'g, 'r, 's>(
        &'x self,
        _context: RefContext<'b, 'g, 'r, 's>,
        input: OrgSource<'s>,
    ) -> Res<OrgSource<'s>, Object<'s>> {
        let mut remaining = input;
        let mut goal = self.source;

        loop {
            if goal.is_empty() {
                break;
            }

            let is_not_whitespace = is_not::<_, _, CustomError>(" \t\r\n")(goal);
            if let Ok((new_goal, payload)) = is_not_whitespace {
                let (new_remaining, _) = tuple((
                    tag_no_case(payload),
                    // TODO: Test to see what the REAL condition is. Checking for not-alphabetic works fine for now, but the real criteria might be something like the plain text exit matcher.
                    peek(alt((
                        recognize(verify(anychar, |c| !c.is_alphanumeric())),
                        eof,
                    ))),
                ))(remaining)?;
                remaining = new_remaining;
                goal = new_goal;
                continue;
            }

            let is_whitespace = recognize(many1(alt((
                recognize(one_of::<_, _, CustomError>(" \t")),
                line_ending,
            ))))(goal);
            if let Ok((new_goal, _)) = is_whitespace {
                let (new_remaining, _) = many1(org_space_or_line_ending)(remaining)?;
                remaining = new_remaining;
                goal = new_goal;
                continue;
            }

            return Err(nom::Err::Error(CustomError::Static(
                "Target does not match.",
            )));
        }

        let source = get_consumed(input, remaining);
        Ok((
            remaining,
            Object::PlainText(PlainText {
                source: Into::<&str>::into(source),
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use nom::combinator::map;

    use super::*;
    use crate::context::bind_context;
    use crate::context::Context;
    use crate::context::ContextElement;
    use crate::context::GlobalSettings;
    use crate::context::List;
    use crate::parser::object_parser::detect_standard_set_object_sans_plain_text;
    use crate::types::StandardProperties;

    #[test]
    fn plain_text_simple() {
        let input = OrgSource::new("foobarbaz");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let (remaining, result) = map(
            bind_context!(
                plain_text(detect_standard_set_object_sans_plain_text),
                &initial_context
            ),
            Object::PlainText,
        )(input)
        .unwrap();
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(result.get_source(), Into::<&str>::into(input));
    }
}
