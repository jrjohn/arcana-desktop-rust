//! Minimal XML element tree (the subset of `System.Xml.Linq` the draw.io codec needs).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

/// An XML element with attributes, child elements and concatenated text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Builder: add an attribute.
    #[allow(clippy::needless_pass_by_value)] // takes numbers and strings alike
    #[must_use]
    pub fn attr(mut self, key: &str, value: impl ToString) -> Self {
        self.attrs.push((key.into(), value.to_string()));
        self
    }

    /// Builder: add a child.
    #[must_use]
    pub fn child(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// First direct child with a name (C# `Element(name)`).
    #[must_use]
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Depth-first descendants with a name, document order (C# `Descendants(name)`).
    #[must_use]
    pub fn descendants<'a>(&'a self, name: &str) -> Vec<&'a Element> {
        let mut out = Vec::new();
        self.collect(name, &mut out);
        out
    }

    fn collect<'a>(&'a self, name: &str, out: &mut Vec<&'a Element>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.collect(name, out);
        }
    }

    /// Self first, then descendants.
    #[must_use]
    pub fn find_self_or_descendant<'a>(&'a self, name: &str) -> Option<&'a Element> {
        if self.name == name {
            return Some(self);
        }
        self.descendants(name).into_iter().next()
    }

    /// Indented serialization, like `XElement.ToString()`.
    #[must_use]
    pub fn to_xml(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, 0);
        s
    }

    fn write(&self, s: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        s.push_str(&pad);
        s.push('<');
        s.push_str(&self.name);
        for (k, v) in &self.attrs {
            s.push(' ');
            s.push_str(k);
            s.push_str("=\"");
            s.push_str(&escape(v));
            s.push('"');
        }
        if self.children.is_empty() && self.text.is_empty() {
            s.push_str(" />");
            return;
        }
        s.push('>');
        if self.children.is_empty() {
            s.push_str(&escape(&self.text));
        } else {
            for c in &self.children {
                s.push('\n');
                c.write(s, depth + 1);
            }
            s.push('\n');
            s.push_str(&pad);
        }
        s.push_str("</");
        s.push_str(&self.name);
        s.push('>');
    }
}

/// Escapes text for an attribute or element body.
#[must_use]
pub fn escape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for ch in v.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            '\t' => out.push_str("&#x9;"),
            c => out.push(c),
        }
    }
    out
}

fn resolve_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// Resolves `&…;` references; `None` on an unknown/unterminated reference.
fn unescape(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';')?;
        out.push(resolve_entity(&after[..end])?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

fn start(e: &BytesStart<'_>) -> Option<Element> {
    let name = String::from_utf8(e.name().as_ref().to_vec()).ok()?;
    let mut el = Element::new(&name);
    for a in e.attributes() {
        let a = a.ok()?;
        let k = String::from_utf8(a.key.as_ref().to_vec()).ok()?;
        let raw = std::str::from_utf8(&a.value).ok()?;
        el.attrs.push((k, unescape(raw)?));
    }
    Some(el)
}

/// Parses a document; `None` unless it is well-formed with exactly one root element.
#[must_use]
pub fn parse(xml: &str) -> Option<Element> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) => stack.push(start(&e)?),
            Event::Empty(e) => {
                let el = start(&e)?;
                attach(&mut stack, &mut root, el)?;
            }
            Event::End(_) => {
                let el = stack.pop()?;
                attach(&mut stack, &mut root, el)?;
            }
            Event::Text(t) => {
                let s = t.decode().ok()?;
                match stack.last_mut() {
                    Some(top) => top.text.push_str(&s),
                    None if s.trim().is_empty() => {}
                    None => return None,
                }
            }
            Event::CData(t) => {
                let s = t.decode().ok()?;
                stack.last_mut()?.text.push_str(&s);
            }
            Event::GeneralRef(r) => {
                let name = r.decode().ok()?;
                let ch = resolve_entity(&name)?;
                stack.last_mut()?.text.push(ch);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.is_empty() { root } else { None }
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, el: Element) -> Option<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(el);
    } else if root.is_none() {
        *root = Some(el);
    } else {
        return None;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_escapes() {
        let el = Element::new("a")
            .attr("v", "x<\"&\ny")
            .child(Element::new("b").attr("k", 1));
        let back = parse(&el.to_xml());
        assert_eq!(back.as_ref().and_then(|e| e.get("v")), Some("x<\"&\ny"));
        assert_eq!(
            back.and_then(|e| e.element("b").cloned())
                .map(|b| b.attrs.len()),
            Some(1)
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("not valid xml").is_none());
        assert!(parse("<a><b></a>").is_none());
        assert!(parse("<a/><b/>").is_none());
    }

    #[test]
    fn text_and_entities() {
        let e = parse("<d>a&amp;b&#65;</d>");
        assert_eq!(e.map(|e| e.text), Some("a&bA".to_string()));
    }
}
