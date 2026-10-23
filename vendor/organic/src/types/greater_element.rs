use super::affiliated_keyword::GetAffiliatedKeywords;
use super::element::Element;
use super::lesser_element::TableCell;
use super::AffiliatedKeywords;
use super::Keyword;
use super::Object;
use super::PostBlank;
use super::StandardProperties;

#[derive(Debug)]
pub struct PlainList<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub list_type: PlainListType,
    pub children: Vec<PlainListItem<'s>>,
    pub contents: Option<&'s str>, // TODO: Can contents ever be None?
    pub post_blank: Option<&'s str>,
}

#[derive(Debug, Copy, Clone)]
pub enum PlainListType {
    Unordered,
    Ordered,
    Descriptive,
}

/// The width that something is indented. For example, a single tab character could be a value of 4 or 8.
pub type IndentationLevel = u16;

#[derive(Debug)]
pub struct PlainListItem<'s> {
    pub source: &'s str,
    pub indentation: IndentationLevel,
    pub bullet: &'s str,
    pub counter: Option<PlainListItemCounter>,
    pub checkbox: Option<(CheckboxType, &'s str)>,
    pub tag: Vec<Object<'s>>,
    pub pre_blank: PlainListItemPreBlank,
    pub children: Vec<Element<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

pub type PlainListItemCounter = u64;
pub type PlainListItemPreBlank = u8;

#[derive(Debug)]
pub enum CheckboxType {
    On,
    Trans,
    Off,
}

#[derive(Debug)]
pub struct CenterBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub children: Vec<Element<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct QuoteBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub children: Vec<Element<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct SpecialBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub block_type: &'s str,
    pub parameters: Option<&'s str>,
    pub children: Vec<Element<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct DynamicBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub block_name: &'s str,
    pub parameters: Option<&'s str>,
    pub children: Vec<Element<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct FootnoteDefinition<'s> {
    pub source: &'s str,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub label: &'s str,
    pub children: Vec<Element<'s>>,
}

#[derive(Debug)]
pub struct Drawer<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub drawer_name: &'s str,
    pub children: Vec<Element<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct PropertyDrawer<'s> {
    pub source: &'s str,
    pub children: Vec<NodeProperty<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct NodeProperty<'s> {
    pub source: &'s str,
    pub property_name: &'s str,
    pub value: Option<&'s str>,
}

#[derive(Debug)]
pub struct Table<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub formulas: Vec<Keyword<'s>>,
    pub children: Vec<TableRow<'s>>,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct TableRow<'s> {
    pub source: &'s str,
    pub children: Vec<TableCell<'s>>,
    pub contents: Option<&'s str>,
}

#[derive(Debug)]
pub enum TableRowType {
    Standard,
    Rule,
}

impl<'s> StandardProperties<'s> for PlainList<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for PlainListItem<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for CenterBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for QuoteBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for SpecialBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for DynamicBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for FootnoteDefinition<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Drawer<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for PropertyDrawer<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for NodeProperty<'s> {
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

impl<'s> StandardProperties<'s> for Table<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for TableRow<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        0
    }
}

impl<'s> PlainListItem<'s> {
    pub fn get_checkbox(&self) -> Option<&'s str> {
        self.checkbox.as_ref().map(|(_, checkbox)| *checkbox)
    }

    pub fn get_checkbox_type(&self) -> Option<&CheckboxType> {
        self.checkbox
            .as_ref()
            .map(|(checkbox_type, _)| checkbox_type)
    }
}

impl<'s> TableRow<'s> {
    pub fn get_type(&self) -> TableRowType {
        if self.children.is_empty() {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        }
    }
}

impl<'s> GetAffiliatedKeywords<'s> for PlainList<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for Table<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for Drawer<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for FootnoteDefinition<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for DynamicBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for SpecialBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for CenterBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for QuoteBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}
