fn split_at_str<'a>(input: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let Some(end) = input.find(needle) else {
        return None;
    };
    let (head, tail) = input.split_at(end);
    Some((head, &tail[needle.len()..]))
}

fn strip_decl(input: &str) -> Option<&str> {
    if !input.trim_start().starts_with("<?xml") {
        return None;
    }
    split_at_str(input, "?>").map(|(_, tail)| tail.trim_start())
}

#[test]
fn test_strip_decl() {
    const WITH_DECL: &str = r#"
<?xml version="1.0" encoding="UTF-8"?>
<protocol name="wayland">
"#;
    assert!(
        strip_decl(WITH_DECL)
            .unwrap()
            .starts_with(r#"<protocol name="wayland">"#)
    );

    const WITHOUT_DECL: &str = r#"
<protocol name="wayland">
"#;
    assert_eq!(strip_decl(WITHOUT_DECL), None);
}

#[derive(Debug, PartialEq, Eq)]
pub struct Attribute<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

pub struct AttributeIterator<'a> {
    attrs: &'a str,
}

impl<'a> AttributeIterator<'a> {
    pub fn new(attrs: &'a str) -> Self {
        Self { attrs }
    }
}

impl<'a> Iterator for AttributeIterator<'a> {
    type Item = Attribute<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (key, rest) = split_at_str(self.attrs, "=")?;
        assert!(rest.starts_with("\""));
        let (value, rest) = split_at_str(&rest[1..], "\"")?;
        self.attrs = rest;
        Some(Attribute {
            key: key.trim(),
            value,
        })
    }
}

#[test]
fn test_attribute_iterator() {
    const ATTRS: &str = r#"name="wl_display" version="1""#;
    let mut iterator = AttributeIterator::new(ATTRS);
    assert_eq!(
        iterator.next(),
        Some(Attribute {
            key: "name",
            value: "wl_display"
        })
    );
    assert_eq!(
        iterator.next(),
        Some(Attribute {
            key: "version",
            value: "1"
        })
    );
    assert_eq!(iterator.next(), None);
}

#[derive(Debug, PartialEq, Eq)]
pub struct EmptyTag<'a> {
    pub name: &'a str,
    pub attrs: &'a str,
}

impl<'a> EmptyTag<'a> {
    pub fn iter_attrs(&self) -> AttributeIterator<'a> {
        AttributeIterator::new(self.attrs)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StartTag<'a> {
    pub name: &'a str,
    pub attrs: &'a str,
}

impl<'a> StartTag<'a> {
    pub fn iter_attrs(&self) -> AttributeIterator<'a> {
        AttributeIterator::new(self.attrs)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EndTag<'a> {
    pub name: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Element<'a> {
    EmptyTag(EmptyTag<'a>),
    StartTag(StartTag<'a>),
    EndTag(EndTag<'a>),
    Text(&'a str),
    Comment(&'a str),
}

pub struct ElementIterator<'a> {
    input: &'a str,
}

impl<'a> ElementIterator<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: strip_decl(input).unwrap_or(input),
        }
    }

    fn next_empty_tag(&mut self) -> Option<Element<'a>> {
        assert!(self.input.starts_with("<"));

        let (content, rest) = split_at_str(self.input, ">")?;
        let content = content.strip_suffix("/")?;
        let (name, attrs) = split_at_str(content, " ").unwrap_or((content, ""));

        self.input = rest;
        Some(Element::EmptyTag(EmptyTag {
            name: &name[1..],
            attrs: attrs.trim(),
        }))
    }

    fn next_end_tag(&mut self) -> Option<Element<'a>> {
        assert!(self.input.starts_with("<"));

        let content = self.input.strip_prefix("</")?;
        let (name, rest) = split_at_str(content, ">")?;

        self.input = rest;
        Some(Element::EndTag(EndTag { name }))
    }

    fn next_start_tag(&mut self) -> Option<Element<'a>> {
        assert!(self.input.starts_with("<"));

        let (content, rest) = split_at_str(&self.input[1..], ">")?;
        assert!(!content.ends_with("/"));
        let (name, attrs) = split_at_str(content, " ").unwrap_or((content, ""));

        self.input = rest;
        Some(Element::StartTag(StartTag { name, attrs }))
    }

    fn next_text(&mut self) -> Option<Element<'a>> {
        assert!(!self.input.starts_with("<"));
        assert!(!self.input.is_empty());

        if let Some(end) = self.input.find("<") {
            let content = &self.input[..end];
            self.input = &self.input[end..];
            return Some(Element::Text(content));
        } else {
            let ret = Some(Element::Text(self.input));
            self.input = "";
            ret
        }
    }

    fn next_comment(&mut self) -> Option<Element<'a>> {
        assert!(self.input.starts_with("<!--"));

        let (content, rest) = split_at_str(&self.input[4..], "-->")?;
        self.input = &rest;
        Some(Element::Comment(content))
    }
}

#[test]
fn test_next_empty_tag() {
    const INPUT: &str = r#"
<entry name="wheel" value="0" summary="a physical wheel rotation" />
"#;
    assert_eq!(
        ElementIterator::new(INPUT.trim()).next_empty_tag(),
        Some(Element::EmptyTag(EmptyTag {
            name: "entry",
            attrs: r#"name="wheel" value="0" summary="a physical wheel rotation""#
        }))
    );
}

#[test]
fn test_next_end_tag() {
    const INPUT: &str = r#"
</protocol>
"#;
    assert_eq!(
        ElementIterator::new(INPUT.trim()).next_end_tag(),
        Some(Element::EndTag(EndTag { name: "protocol" }))
    );
}

#[test]
fn test_next_start_tag() {
    const INPUT: &str = r#"
<interface name="wl_display" version="1">
"#;
    assert_eq!(
        ElementIterator::new(INPUT.trim()).next_start_tag(),
        Some(Element::StartTag(StartTag {
            name: "interface",
            attrs: r#"name="wl_display" version="1""#
        }))
    );
}

#[test]
fn test_next_text() {
    const INPUT: &str = r#"
    text
"#;
    assert_eq!(
        ElementIterator::new(INPUT.trim()).next_text(),
        Some(Element::Text("text"))
    );
}

#[test]
fn test_next_comment() {
    const INPUT: &str = r#"
<!-- SECTION: GL type definitions. -->
"#;
    assert_eq!(
        ElementIterator::new(INPUT.trim()).next_comment(),
        Some(Element::Comment(" SECTION: GL type definitions. ")),
    );
}

impl<'a> Iterator for ElementIterator<'a> {
    type Item = Element<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.input.is_empty() {
            None
        } else if self.input.starts_with("<!--") {
            self.next_comment()
        } else if self.input.starts_with("<") {
            self.next_empty_tag()
                .or_else(|| self.next_end_tag())
                .or_else(|| self.next_start_tag())
        } else {
            self.next_text()
        }
    }
}
