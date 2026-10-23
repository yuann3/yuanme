use std::borrow::Borrow;
use std::borrow::Cow;

use super::util::coalesce_whitespace_escaped;
use super::util::coalesce_whitespace_if_line_break;
use super::util::remove_line_break;
use super::util::remove_whitespace_if_line_break;
use super::util::to_lowercase;
use super::PostBlank;
use super::StandardProperties;

#[derive(Debug)]
pub enum Object<'s> {
    Bold(Bold<'s>),
    Italic(Italic<'s>),
    Underline(Underline<'s>),
    StrikeThrough(StrikeThrough<'s>),
    Code(Code<'s>),
    Verbatim(Verbatim<'s>),
    PlainText(PlainText<'s>),
    RegularLink(RegularLink<'s>),
    RadioLink(RadioLink<'s>),
    RadioTarget(RadioTarget<'s>),
    PlainLink(PlainLink<'s>),
    AngleLink(AngleLink<'s>),
    OrgMacro(OrgMacro<'s>),
    Entity(Entity<'s>),
    LatexFragment(LatexFragment<'s>),
    ExportSnippet(ExportSnippet<'s>),
    FootnoteReference(FootnoteReference<'s>),
    Citation(Citation<'s>),
    CitationReference(CitationReference<'s>),
    InlineBabelCall(InlineBabelCall<'s>),
    InlineSourceBlock(InlineSourceBlock<'s>),
    LineBreak(LineBreak<'s>),
    Target(Target<'s>),
    StatisticsCookie(StatisticsCookie<'s>),
    Subscript(Subscript<'s>),
    Superscript(Superscript<'s>),
    Timestamp(Timestamp<'s>),
}

#[derive(Debug)]
pub struct Bold<'s> {
    pub source: &'s str,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct Italic<'s> {
    pub source: &'s str,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct Underline<'s> {
    pub source: &'s str,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct StrikeThrough<'s> {
    pub source: &'s str,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct Code<'s> {
    pub source: &'s str,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct Verbatim<'s> {
    pub source: &'s str,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct PlainText<'s> {
    pub source: &'s str,
}

#[derive(Debug)]
pub struct RegularLink<'s> {
    pub source: &'s str,
    pub link_type: LinkType<'s>,
    /// The path after templates have been applied.
    ///
    /// This does not take into account the post-processing that you would get from the upstream emacs org-mode AST. Use `get_raw_link` for an equivalent value.
    pub path: Cow<'s, str>,

    /// The raw link after templates have been applied.
    ///
    /// This does not take into account the post-processing that you would get from the upstream emacs org-mode AST. Use `get_raw_link` for an equivalent value.
    pub raw_link: Cow<'s, str>,

    /// The search_option after templates have been applied.
    ///
    /// This does not take into account the post-processing that you would get from the upstream emacs org-mode AST. Use `get_search_option` for an equivalent value.
    pub search_option: Option<Cow<'s, str>>,

    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
    pub application: Option<Cow<'s, str>>,
}

#[derive(Debug)]
pub struct RadioTarget<'s> {
    pub source: &'s str,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct RadioLink<'s> {
    pub source: &'s str,
    pub path: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct PlainLink<'s> {
    pub source: &'s str,
    pub link_type: LinkType<'s>,
    pub path: &'s str,
    pub raw_link: &'s str,
    pub search_option: Option<&'s str>,
    pub application: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct AngleLink<'s> {
    pub source: &'s str,
    pub link_type: LinkType<'s>,

    /// The path from the source.
    ///
    /// This does not take into account the post-processing that you would get from the upstream emacs org-mode AST. Use `get_raw_link` for an equivalent value.
    pub path: &'s str,
    pub raw_link: &'s str,

    /// The search_option from the source.
    ///
    /// This does not take into account the post-processing that you would get from the upstream emacs org-mode AST. Use `get_search_option` for an equivalent value.
    pub search_option: Option<&'s str>,
    pub application: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct OrgMacro<'s> {
    pub source: &'s str,

    /// The key from the source.
    ///
    /// This does not take into account the post-processing that you would get from the upstream emacs org-mode AST. Use `get_key` for an equivalent value.
    pub key: &'s str,

    /// The args from the source.
    ///
    /// This does not take into account the post-processing that you would get from the upstream emacs org-mode AST. Use `get_args` for an equivalent value.
    pub args: Vec<&'s str>,

    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct Entity<'s> {
    pub source: &'s str,
    pub name: &'s str,
    pub latex_math_mode: bool,
    pub latex: &'s str,
    pub html: &'s str,
    pub ascii: &'s str,
    // Skipping latin1 because it is detrimental to the future. If anyone out there is using latin1, take a long look in the mirror and change your ways.
    pub utf8: &'s str,
    pub use_brackets: bool,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct LatexFragment<'s> {
    pub source: &'s str,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct ExportSnippet<'s> {
    pub source: &'s str,
    pub backend: &'s str,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct FootnoteReference<'s> {
    pub source: &'s str,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
    pub label: Option<&'s str>,
    pub definition: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct Citation<'s> {
    pub source: &'s str,
    pub style: Option<&'s str>,
    pub prefix: Vec<Object<'s>>,
    pub suffix: Vec<Object<'s>>,
    pub children: Vec<CitationReference<'s>>,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct CitationReference<'s> {
    pub source: &'s str,
    pub key: &'s str,
    pub prefix: Vec<Object<'s>>,
    pub suffix: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct InlineBabelCall<'s> {
    pub source: &'s str,
    pub value: &'s str,
    pub call: &'s str,
    pub inside_header: Option<&'s str>,
    pub arguments: Option<&'s str>,
    pub end_header: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct InlineSourceBlock<'s> {
    pub source: &'s str,
    pub language: &'s str,
    pub parameters: Option<&'s str>,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct LineBreak<'s> {
    pub source: &'s str,
}

#[derive(Debug)]
pub struct Target<'s> {
    pub source: &'s str,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct StatisticsCookie<'s> {
    pub source: &'s str,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct Subscript<'s> {
    pub source: &'s str,
    pub use_brackets: bool,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct Superscript<'s> {
    pub source: &'s str,
    pub use_brackets: bool,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Object<'s>>,
}

// TODO: Perhaps there is an optimization of converting to unix time we can do to shrink this struct. (ref: clippy::large_enum_variant on Element)
#[derive(Debug, Clone)]
pub struct Timestamp<'s> {
    pub source: &'s str,
    pub timestamp_type: TimestampType,
    pub range_type: TimestampRangeType,
    pub start: Option<Date<'s>>,
    pub end: Option<Date<'s>>,
    pub start_time: Option<Time<'s>>,
    pub end_time: Option<Time<'s>>,
    pub repeater: Option<Repeater>,
    pub warning_delay: Option<WarningDelay>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug, Clone)]
pub enum TimestampType {
    Diary,
    Active,
    Inactive,
    ActiveRange,
    InactiveRange,
}

#[derive(Debug, Clone)]
pub enum TimestampRangeType {
    None,
    DateRange,
    TimeRange,
}

pub type YearInner = u16;
pub type MonthInner = u8;
pub type DayOfMonthInner = u8;
pub type HourInner = u8;
pub type MinuteInner = u8;

#[derive(Debug, Clone)]
pub struct Year(pub YearInner);

#[derive(Debug, Clone)]
pub struct Month(pub MonthInner);

#[derive(Debug, Clone)]
pub struct DayOfMonth(pub DayOfMonthInner);

#[derive(Debug, Clone)]
pub struct Hour(pub HourInner);

#[derive(Debug, Clone)]
pub struct Minute(pub MinuteInner);

impl Year {
    // TODO: Make a real error type instead of a boxed any error.
    pub fn new(source: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let year = source.parse::<YearInner>()?;
        Ok(Year(year))
    }

    pub fn get_value(&self) -> YearInner {
        self.0
    }
}

impl Month {
    // TODO: Make a real error type instead of a boxed any error.
    pub fn new(source: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let month = source.parse::<MonthInner>()?;
        if !(1..=12).contains(&month) {
            Err("Month exceeds possible range.")?;
        }
        Ok(Month(month))
    }

    pub fn get_value(&self) -> MonthInner {
        self.0
    }
}

impl DayOfMonth {
    // TODO: Make a real error type instead of a boxed any error.
    pub fn new(source: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let day_of_month = source.parse::<DayOfMonthInner>()?;
        if !(1..=31).contains(&day_of_month) {
            Err("Day of month exceeds possible range.")?;
        }
        Ok(DayOfMonth(day_of_month))
    }

    pub fn get_value(&self) -> DayOfMonthInner {
        self.0
    }
}

impl Hour {
    // TODO: Make a real error type instead of a boxed any error.
    pub fn new(source: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let hour = source.parse::<HourInner>()?;
        if hour > 23 {
            Err("Hour exceeds possible range.")?;
        }
        Ok(Hour(hour))
    }

    pub fn get_value(&self) -> HourInner {
        self.0
    }
}

impl Minute {
    // TODO: Make a real error type instead of a boxed any error.
    pub fn new(source: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let minute = source.parse::<MinuteInner>()?;
        if minute > 59 {
            Err("Minute exceeds possible range.")?;
        }
        Ok(Minute(minute))
    }

    pub fn get_value(&self) -> MinuteInner {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Date<'s> {
    year: Year,
    month: Month,
    day_of_month: DayOfMonth,
    day_name: Option<&'s str>,
}

impl<'s> Date<'s> {
    // TODO: Make a real error type instead of a boxed any error.
    pub fn new(
        year: Year,
        month: Month,
        day_of_month: DayOfMonth,
        day_name: Option<&'s str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: Does org-mode support non-gregorian calendars?
        // TODO: Do I want to validate leap year?
        match (month.get_value(), day_of_month.get_value()) {
            (1, 1..=31) => {}
            (2, 1..=29) => {}
            (3, 1..=31) => {}
            (4, 1..=30) => {}
            (5, 1..=31) => {}
            (6, 1..=30) => {}
            (7, 1..=31) => {}
            (8, 1..=31) => {}
            (9, 1..=30) => {}
            (10, 1..=31) => {}
            (11, 1..=30) => {}
            (12, 1..=31) => {}
            _ => Err("Invalid day of month for the month.")?,
        };
        Ok(Date {
            year,
            month,
            day_of_month,
            day_name,
        })
    }

    pub fn get_year(&self) -> &Year {
        &self.year
    }

    pub fn get_month(&self) -> &Month {
        &self.month
    }

    pub fn get_day_of_month(&self) -> &DayOfMonth {
        &self.day_of_month
    }

    pub fn get_day_name(&self) -> Option<&'s str> {
        self.day_name
    }
}

#[derive(Debug, Clone)]
pub struct Time<'s> {
    hour: Hour,
    minute: Minute,
    postfix: Option<&'s str>,
}

impl<'s> Time<'s> {
    // TODO: Make a real error type instead of a boxed any error.
    pub fn new(
        hour: Hour,
        minute: Minute,
        postfix: Option<&'s str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Time {
            hour,
            minute,
            postfix,
        })
    }

    pub fn get_hour(&self) -> &Hour {
        &self.hour
    }

    pub fn get_minute(&self) -> &Minute {
        &self.minute
    }

    pub fn get_postfix(&self) -> Option<&'s str> {
        self.postfix
    }
}

#[derive(Debug, Clone)]
pub enum RepeaterType {
    Cumulative,
    CatchUp,
    Restart,
}

#[derive(Debug, Clone)]
pub enum WarningDelayType {
    All,
    First,
}

#[derive(Debug, Clone)]
pub enum TimeUnit {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

pub type RepeaterWarningDelayValueType = u16;

#[derive(Debug, Clone)]
pub struct Repeater {
    pub repeater_type: RepeaterType,
    pub value: RepeaterWarningDelayValueType,
    pub unit: TimeUnit,
}

#[derive(Debug, Clone)]
pub struct WarningDelay {
    pub warning_delay_type: WarningDelayType,
    pub value: RepeaterWarningDelayValueType,
    pub unit: TimeUnit,
}

impl<'s> StandardProperties<'s> for Bold<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Italic<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Underline<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for StrikeThrough<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Code<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Verbatim<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for RegularLink<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for RadioLink<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.path)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for RadioTarget<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.value)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for PlainLink<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for AngleLink<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for OrgMacro<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Entity<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for LatexFragment<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for ExportSnippet<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for FootnoteReference<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Citation<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for CitationReference<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        0
    }
}

impl<'s> StandardProperties<'s> for InlineBabelCall<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for InlineSourceBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for LineBreak<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        0
    }
}

impl<'s> StandardProperties<'s> for Target<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for StatisticsCookie<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|post_blank| post_blank.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Subscript<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Superscript<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Timestamp<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.chars().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for PlainText<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        // This field does not actually exist in emacs for plaintext
        Some(self.source)
    }

    fn get_post_blank(&self) -> PostBlank {
        // This field does not actually exist in emacs for plaintext
        0
    }
}

impl<'s> Timestamp<'s> {
    pub fn get_raw_value(&self) -> &'s str {
        self.source.trim_end()
    }
}

#[derive(Debug)]
pub enum LinkType<'s> {
    File,
    Protocol(Cow<'s, str>),
    Id,
    CustomId,
    CodeRef,
    Fuzzy,
}

impl<'s> RegularLink<'s> {
    /// Coalesce whitespace if the raw_link contains line breaks.
    ///
    /// This corresponds to the output you would get from the upstream emacs org-mode AST.
    pub fn get_raw_link(&self) -> Cow<'_, str> {
        coalesce_whitespace_if_line_break(&self.raw_link)
    }

    /// Coalesce whitespace if the path contains line breaks.
    ///
    /// This corresponds to the output you would get from the upstream emacs org-mode AST.
    pub fn get_path(&self) -> Cow<'_, str> {
        coalesce_whitespace_if_line_break(&self.path)
    }

    /// Coalesce whitespace if the search_option contains line breaks.
    ///
    /// This corresponds to the output you would get from the upstream emacs org-mode AST.
    pub fn get_search_option(&self) -> Option<Cow<'_, str>> {
        self.search_option
            .as_ref()
            .map(|search_option| coalesce_whitespace_if_line_break(search_option.borrow()))
    }
}

impl<'s> RadioLink<'s> {
    pub fn get_raw_link(&self) -> &'s str {
        self.path
    }
}

impl<'s> AngleLink<'s> {
    /// Remove line breaks but preserve multiple consecutive spaces.
    ///
    /// This corresponds to the output you would get from the upstream emacs org-mode AST.
    pub fn get_path(&self) -> Cow<'s, str> {
        remove_line_break(self.path)
    }

    /// Remove all whitespace but only if search_option contains a line break.
    ///
    /// This corresponds to the output you would get from the upstream emacs org-mode AST.
    pub fn get_search_option(&self) -> Option<Cow<'s, str>> {
        self.search_option.map(remove_whitespace_if_line_break)
    }
}

impl<'s> OrgMacro<'s> {
    pub fn get_key<'b>(&'b self) -> Cow<'s, str> {
        to_lowercase(self.key)
    }

    pub fn get_args<'b>(&'b self) -> impl Iterator<Item = Cow<'s, str>> + 'b {
        self.args
            .iter()
            .map(|arg| coalesce_whitespace_escaped('\\', |c| ",".contains(c))(arg))
    }
}

#[derive(Debug)]
pub enum FootnoteReferenceType {
    Standard,
    Inline,
}

impl<'s> FootnoteReference<'s> {
    pub fn get_type(&self) -> FootnoteReferenceType {
        if self.definition.is_empty() {
            FootnoteReferenceType::Standard
        } else {
            FootnoteReferenceType::Inline
        }
    }
}

impl<'s> StandardProperties<'s> for Object<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        match self {
            Object::Bold(inner) => inner.get_source(),
            Object::Italic(inner) => inner.get_source(),
            Object::Underline(inner) => inner.get_source(),
            Object::StrikeThrough(inner) => inner.get_source(),
            Object::Code(inner) => inner.get_source(),
            Object::Verbatim(inner) => inner.get_source(),
            Object::PlainText(inner) => inner.get_source(),
            Object::RegularLink(inner) => inner.get_source(),
            Object::RadioLink(inner) => inner.get_source(),
            Object::RadioTarget(inner) => inner.get_source(),
            Object::PlainLink(inner) => inner.get_source(),
            Object::AngleLink(inner) => inner.get_source(),
            Object::OrgMacro(inner) => inner.get_source(),
            Object::Entity(inner) => inner.get_source(),
            Object::LatexFragment(inner) => inner.get_source(),
            Object::ExportSnippet(inner) => inner.get_source(),
            Object::FootnoteReference(inner) => inner.get_source(),
            Object::Citation(inner) => inner.get_source(),
            Object::CitationReference(inner) => inner.get_source(),
            Object::InlineBabelCall(inner) => inner.get_source(),
            Object::InlineSourceBlock(inner) => inner.get_source(),
            Object::LineBreak(inner) => inner.get_source(),
            Object::Target(inner) => inner.get_source(),
            Object::StatisticsCookie(inner) => inner.get_source(),
            Object::Subscript(inner) => inner.get_source(),
            Object::Superscript(inner) => inner.get_source(),
            Object::Timestamp(inner) => inner.get_source(),
        }
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        match self {
            Object::Bold(inner) => inner.get_contents(),
            Object::Italic(inner) => inner.get_contents(),
            Object::Underline(inner) => inner.get_contents(),
            Object::StrikeThrough(inner) => inner.get_contents(),
            Object::Code(inner) => inner.get_contents(),
            Object::Verbatim(inner) => inner.get_contents(),
            Object::PlainText(inner) => inner.get_contents(),
            Object::RegularLink(inner) => inner.get_contents(),
            Object::RadioLink(inner) => inner.get_contents(),
            Object::RadioTarget(inner) => inner.get_contents(),
            Object::PlainLink(inner) => inner.get_contents(),
            Object::AngleLink(inner) => inner.get_contents(),
            Object::OrgMacro(inner) => inner.get_contents(),
            Object::Entity(inner) => inner.get_contents(),
            Object::LatexFragment(inner) => inner.get_contents(),
            Object::ExportSnippet(inner) => inner.get_contents(),
            Object::FootnoteReference(inner) => inner.get_contents(),
            Object::Citation(inner) => inner.get_contents(),
            Object::CitationReference(inner) => inner.get_contents(),
            Object::InlineBabelCall(inner) => inner.get_contents(),
            Object::InlineSourceBlock(inner) => inner.get_contents(),
            Object::LineBreak(inner) => inner.get_contents(),
            Object::Target(inner) => inner.get_contents(),
            Object::StatisticsCookie(inner) => inner.get_contents(),
            Object::Subscript(inner) => inner.get_contents(),
            Object::Superscript(inner) => inner.get_contents(),
            Object::Timestamp(inner) => inner.get_contents(),
        }
    }

    fn get_post_blank(&self) -> PostBlank {
        match self {
            Object::Bold(inner) => inner.get_post_blank(),
            Object::Italic(inner) => inner.get_post_blank(),
            Object::Underline(inner) => inner.get_post_blank(),
            Object::StrikeThrough(inner) => inner.get_post_blank(),
            Object::Code(inner) => inner.get_post_blank(),
            Object::Verbatim(inner) => inner.get_post_blank(),
            Object::PlainText(inner) => inner.get_post_blank(),
            Object::RegularLink(inner) => inner.get_post_blank(),
            Object::RadioLink(inner) => inner.get_post_blank(),
            Object::RadioTarget(inner) => inner.get_post_blank(),
            Object::PlainLink(inner) => inner.get_post_blank(),
            Object::AngleLink(inner) => inner.get_post_blank(),
            Object::OrgMacro(inner) => inner.get_post_blank(),
            Object::Entity(inner) => inner.get_post_blank(),
            Object::LatexFragment(inner) => inner.get_post_blank(),
            Object::ExportSnippet(inner) => inner.get_post_blank(),
            Object::FootnoteReference(inner) => inner.get_post_blank(),
            Object::Citation(inner) => inner.get_post_blank(),
            Object::CitationReference(inner) => inner.get_post_blank(),
            Object::InlineBabelCall(inner) => inner.get_post_blank(),
            Object::InlineSourceBlock(inner) => inner.get_post_blank(),
            Object::LineBreak(inner) => inner.get_post_blank(),
            Object::Target(inner) => inner.get_post_blank(),
            Object::StatisticsCookie(inner) => inner.get_post_blank(),
            Object::Subscript(inner) => inner.get_post_blank(),
            Object::Superscript(inner) => inner.get_post_blank(),
            Object::Timestamp(inner) => inner.get_post_blank(),
        }
    }
}
