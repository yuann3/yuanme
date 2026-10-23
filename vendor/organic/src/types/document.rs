use super::Element;
use super::NodeProperty;
use super::Object;
use super::PostBlank;
use super::StandardProperties;
use super::Timestamp;

pub type PriorityCookie = u8;
pub type HeadlineLevel = u16;

#[derive(Debug)]
pub struct Document<'s> {
    pub source: &'s str,
    pub category: Option<String>,
    pub zeroth_section: Option<Section<'s>>,
    pub children: Vec<Heading<'s>>,
    pub contents: &'s str,
}

#[derive(Debug)]
pub struct Heading<'s> {
    pub source: &'s str,
    pub level: HeadlineLevel,
    pub todo_keyword: Option<(TodoKeywordType, &'s str)>,
    pub priority_cookie: Option<PriorityCookie>,
    pub title: Vec<Object<'s>>,
    pub tags: Vec<&'s str>,
    pub children: Vec<DocumentElement<'s>>,
    pub is_comment: bool,
    pub is_archived: bool,
    pub is_footnote_section: bool,
    pub scheduled: Option<Timestamp<'s>>,
    pub deadline: Option<Timestamp<'s>>,
    pub closed: Option<Timestamp<'s>>,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct Section<'s> {
    pub source: &'s str,
    pub post_blank: Option<&'s str>,
    pub children: Vec<Element<'s>>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum DocumentElement<'s> {
    Heading(Heading<'s>),
    Section(Section<'s>),
}

#[derive(Debug)]
pub enum TodoKeywordType {
    Todo,
    Done,
}

impl<'s> StandardProperties<'s> for Document<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        0
    }
}

impl<'s> StandardProperties<'s> for Section<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.source)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Heading<'s> {
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

impl<'s> Heading<'s> {
    pub fn get_raw_value(&self) -> String {
        // TODO: I think this could just return a string slice instead of an owned string.
        let title_source: String = self.title.iter().map(|obj| obj.get_source()).collect();
        title_source
    }

    pub fn get_additional_properties(&self) -> impl Iterator<Item = &NodeProperty<'s>> {
        self.children
            .iter()
            .take(1)
            .filter_map(|c| match c {
                DocumentElement::Section(section) => Some(section),
                _ => None,
            })
            .flat_map(|section| section.children.iter())
            .take_while(|element| {
                matches!(element, Element::Planning(_) | Element::PropertyDrawer(_))
            })
            .find_map(|element| match element {
                Element::PropertyDrawer(property_drawer) => Some(property_drawer),
                _ => None,
            })
            .into_iter()
            .flat_map(|property_drawer| property_drawer.children.iter())
    }
}

impl<'s> Document<'s> {
    pub fn get_additional_properties(&self) -> impl Iterator<Item = &NodeProperty<'s>> {
        let zeroth_section_children = self
            .zeroth_section
            .iter()
            .flat_map(|zeroth_section| zeroth_section.children.iter());
        let property_drawer = zeroth_section_children
            .take_while(|element| {
                matches!(element, Element::Comment(_) | Element::PropertyDrawer(_))
            })
            .find_map(|element| match element {
                Element::PropertyDrawer(property_drawer) => Some(property_drawer),
                _ => None,
            });
        property_drawer
            .into_iter()
            .flat_map(|property_drawer| property_drawer.children.iter())
    }
}

impl<'s> StandardProperties<'s> for DocumentElement<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        match self {
            DocumentElement::Heading(inner) => inner.get_source(),
            DocumentElement::Section(inner) => inner.get_source(),
        }
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        match self {
            DocumentElement::Heading(inner) => inner.get_contents(),
            DocumentElement::Section(inner) => inner.get_contents(),
        }
    }

    fn get_post_blank(&self) -> PostBlank {
        match self {
            DocumentElement::Heading(inner) => inner.get_post_blank(),
            DocumentElement::Section(inner) => inner.get_post_blank(),
        }
    }
}
