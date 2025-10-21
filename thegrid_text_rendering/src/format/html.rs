extern crate markup5ever_rcdom as rcdom;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crate::node;
use crate::node::{
    ImageNode, InlineNode, LinkMark, NodeContext, Paragraph, Table, TableRow, TextMark,
};
use gpui::{DefiniteLength, SharedString, px, relative};
use html5ever::tendril::TendrilSink;
use html5ever::{LocalName, ParseOpts, local_name, parse_document};
use markup5ever_rcdom::{Node, NodeData, RcDom};

const BLOCK_ELEMENTS: [&str; 35] = [
    "html",
    "body",
    "head",
    "address",
    "article",
    "aside",
    "blockquote",
    "details",
    "summary",
    "dialog",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "ul",
    "style",
    "script",
];

const IGNORED_ELEMENTS: [&str; 1] = ["mx-reply"];

/// Parse HTML into AST Node.
pub(crate) fn parse(source: &str, cx: &mut NodeContext) -> Result<node::Node, SharedString> {
    let opts = ParseOpts {
        ..Default::default()
    };

    let bytes = cleanup_html(&source);
    let mut cursor = std::io::Cursor::new(bytes);
    // Ref
    // https://github.com/servo/html5ever/blob/main/rcdom/examples/print-rcdom.rs
    let dom = parse_document(RcDom::default(), opts)
        .from_utf8()
        .read_from(&mut cursor)
        .map_err(|e| SharedString::from(format!("{:?}", e)))?;

    let mut paragraph = Paragraph::default();
    // NOTE: The outer paragraph is not used.
    let node: node::Node =
        parse_node(&dom.document, &mut paragraph, cx).unwrap_or(node::Node::Unknown);
    let node = node.compact();

    Ok(node)
}

fn cleanup_html(source: &str) -> Vec<u8> {
    let mut w = std::io::Cursor::new(vec![]);
    let mut r = std::io::Cursor::new(source);
    let mut minify = super::html5minify::Minifier::new(&mut w);
    minify.omit_doctype(true);
    if let Ok(()) = minify.minify(&mut r) {
        w.into_inner()
    } else {
        source.bytes().collect()
    }
}

fn attr_value(attrs: &RefCell<Vec<html5ever::Attribute>>, name: LocalName) -> Option<String> {
    attrs.borrow().iter().find_map(|attr| {
        if attr.name.local == name {
            Some(attr.value.to_string())
        } else {
            None
        }
    })
}

/// Get style properties to HashMap
/// TODO: Use cssparser to parse style attribute.
fn style_attrs(attrs: &RefCell<Vec<html5ever::Attribute>>) -> HashMap<String, String> {
    let mut styles = HashMap::new();
    let Some(css_text) = attr_value(attrs, local_name!("style")) else {
        return styles;
    };

    for decl in css_text.split(';') {
        let mut parts = decl.splitn(2, ':');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            styles.insert(
                key.trim().to_lowercase().to_string(),
                value.trim().to_string(),
            );
        }
    }

    styles
}

/// Parse length value from style attribute.
///
/// When is percentage, it will be converted to relative length.
/// Else, it will be converted to pixels.
fn value_to_length(value: &str) -> Option<DefiniteLength> {
    if value.ends_with("%") {
        value
            .trim_end_matches("%")
            .parse::<f32>()
            .ok()
            .map(|v| relative(v / 100.))
    } else {
        value
            .trim_end_matches("px")
            .parse()
            .ok()
            .map(|v| px(v).into())
    }
}

/// Get width, height from attributes or parse them from style attribute.
fn attr_width_height(
    attrs: &RefCell<Vec<html5ever::Attribute>>,
) -> (Option<DefiniteLength>, Option<DefiniteLength>) {
    let mut width = None;
    let mut height = None;

    if let Some(value) = attr_value(attrs, local_name!("width")) {
        width = value_to_length(&value);
    }

    if let Some(value) = attr_value(attrs, local_name!("height")) {
        height = value_to_length(&value);
    }

    if width.is_none() || height.is_none() {
        let styles = style_attrs(attrs);
        if width.is_none() {
            width = styles.get("width").and_then(|v| value_to_length(&v));
        }
        if height.is_none() {
            height = styles.get("height").and_then(|v| value_to_length(&v));
        }
    }

    (width, height)
}

fn parse_table_row(table: &mut Table, node: &Rc<Node>) {
    let mut row = TableRow::default();
    let mut count = 0;
    for child in node.children.borrow().iter() {
        match child.data {
            NodeData::Element {
                ref name,
                ref attrs,
                ..
            } if name.local == local_name!("td") || name.local == local_name!("th") => {
                if child.children.borrow().is_empty() {
                    continue;
                }

                count += 1;
                parse_table_cell(&mut row, child, attrs);
            }
            _ => {}
        }
    }

    if count > 0 {
        table.children.push(row);
    }
}

fn parse_table_cell(
    row: &mut node::TableRow,
    node: &Rc<Node>,
    attrs: &RefCell<Vec<html5ever::Attribute>>,
) {
    let mut paragraph = Paragraph::default();
    for child in node.children.borrow().iter() {
        parse_paragraph(&mut paragraph, child);
    }
    let width = attr_width_height(attrs).0;
    let table_cell = node::TableCell {
        children: paragraph,
        width,
    };
    row.children.push(table_cell);
}

/// Trim text but leave at least one space.
///
/// - Before: " \r\n Hello world \t "
/// - After: " Hello world "
#[allow(dead_code)]
fn trim_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for (i, c) in text.chars().enumerate() {
        if c.is_whitespace() {
            if i > 0 && out.ends_with(' ') {
                continue;
            }
        }
        out.push(c);
    }

    out
}

fn parse_paragraph(
    paragraph: &mut Paragraph,
    node: &Rc<Node>,
) -> (String, Vec<(Range<usize>, TextMark)>) {
    let mut text = String::new();
    let mut marks = vec![];

    /// Append new_text and new_marks to text and marks.
    fn merge_child_text(
        text: &mut String,
        marks: &mut Vec<(Range<usize>, TextMark)>,
        new_text: &str,
        new_marks: &[(Range<usize>, TextMark)],
    ) {
        let offset = text.len();
        text.push_str(new_text);
        for (range, style) in new_marks {
            marks.push((range.start + offset..new_text.len() + offset, style.clone()));
        }
    }

    match &node.data {
        NodeData::Text { contents } => {
            let part = &contents.borrow();
            text.push_str(&part);
            paragraph.push_str(&text);
        }
        NodeData::Element { name, attrs, .. } => match name.local {
            local_name!("em") | local_name!("i") => {
                let mut child_paragraph = Paragraph::default();
                for child in node.children.borrow().iter() {
                    let (child_text, child_marks) = parse_paragraph(&mut child_paragraph, &child);
                    merge_child_text(&mut text, &mut marks, &child_text, &child_marks);
                }
                marks.push((0..text.len(), TextMark::default().italic()));
                paragraph.push(InlineNode::new(&text).marks(marks.clone()));
            }
            local_name!("strong") | local_name!("b") => {
                let mut child_paragraph = Paragraph::default();
                for child in node.children.borrow().iter() {
                    let (child_text, child_marks) = parse_paragraph(&mut child_paragraph, &child);
                    merge_child_text(&mut text, &mut marks, &child_text, &child_marks);
                }
                marks.push((0..text.len(), TextMark::default().bold()));
                paragraph.push(InlineNode::new(&text).marks(marks.clone()));
            }
            local_name!("del") | local_name!("s") => {
                let mut child_paragraph = Paragraph::default();
                for child in node.children.borrow().iter() {
                    let (child_text, child_marks) = parse_paragraph(&mut child_paragraph, &child);
                    merge_child_text(&mut text, &mut marks, &child_text, &child_marks);
                }
                marks.push((0..text.len(), TextMark::default().strikethrough()));
                paragraph.push(InlineNode::new(&text).marks(marks.clone()));
            }
            local_name!("code") => {
                let mut child_paragraph = Paragraph::default();
                for child in node.children.borrow().iter() {
                    let (child_text, child_marks) = parse_paragraph(&mut child_paragraph, &child);
                    merge_child_text(&mut text, &mut marks, &child_text, &child_marks);
                }
                marks.push((0..text.len(), TextMark::default().code()));
                paragraph.push(InlineNode::new(&text).marks(marks.clone()));
            }
            local_name!("a") => {
                let mut child_paragraph = Paragraph::default();
                for child in node.children.borrow().iter() {
                    let (child_text, child_marks) = parse_paragraph(&mut child_paragraph, &child);
                    merge_child_text(&mut text, &mut marks, &child_text, &child_marks);
                }

                marks.push((
                    0..text.len(),
                    TextMark::default().link(LinkMark {
                        url: attr_value(&attrs, local_name!("href"))
                            .unwrap_or_default()
                            .into(),
                        title: attr_value(&attrs, local_name!("title")).map(Into::into),
                        ..Default::default()
                    }),
                ));
                paragraph.push(InlineNode::new(&text).marks(marks.clone()));
            }
            local_name!("img") => {
                let Some(src) = attr_value(attrs, local_name!("src")) else {
                    if cfg!(debug_assertions) {
                        tracing::warn!("Image node missing src attribute");
                    }
                    return (text, marks);
                };

                let alt = attr_value(attrs, local_name!("alt"));
                let title = attr_value(attrs, local_name!("title"));
                let (width, height) = attr_width_height(attrs);

                paragraph.push_image(ImageNode {
                    url: src.into(),
                    link: None,
                    alt: alt.map(Into::into),
                    width,
                    height,
                    title: title.map(Into::into),
                });
            }
            _ => {
                // All unknown tags to as text
                let mut child_paragraph = Paragraph::default();
                for child in node.children.borrow().iter() {
                    let (child_text, child_marks) = parse_paragraph(&mut child_paragraph, &child);
                    merge_child_text(&mut text, &mut marks, &child_text, &child_marks);
                }
                paragraph.push(InlineNode::new(&text).marks(marks.clone()));
            }
        },
        _ => {
            let mut child_paragraph = Paragraph::default();
            for child in node.children.borrow().iter() {
                let (child_text, child_marks) = parse_paragraph(&mut child_paragraph, &child);
                merge_child_text(&mut text, &mut marks, &child_text, &child_marks);
            }
            paragraph.push(InlineNode::new(&text).marks(marks.clone()));
        }
    }

    (text, marks)
}

fn parse_node(
    node: &Rc<Node>,
    paragraph: &mut Paragraph,
    cx: &mut NodeContext,
) -> Option<node::Node> {
    match node.data {
        NodeData::Text { ref contents } => {
            let text = contents.borrow().to_string();
            if text.len() > 0 {
                paragraph.push_str(&text);
            }

            None
        }
        NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => match name.local {
            local_name!("br") => Some(node::Node::Break { html: true }),
            local_name!("h1")
            | local_name!("h2")
            | local_name!("h3")
            | local_name!("h4")
            | local_name!("h5")
            | local_name!("h6") => {
                let mut children = vec![];
                consume_paragraph(&mut children, paragraph);

                let level = name
                    .local
                    .chars()
                    .last()
                    .unwrap_or('6')
                    .to_digit(10)
                    .unwrap_or(6) as u8;

                let mut paragraph = Paragraph::default();
                for child in node.children.borrow().iter() {
                    parse_paragraph(&mut paragraph, child);
                }

                let heading = node::Node::Heading {
                    level,
                    children: paragraph,
                };
                if children.len() > 0 {
                    children.push(heading);

                    Some(node::Node::Root { children })
                } else {
                    Some(heading)
                }
            }
            local_name!("img") => {
                let mut children = vec![];
                consume_paragraph(&mut children, paragraph);

                let Some(src) = attr_value(attrs, local_name!("src")) else {
                    if cfg!(debug_assertions) {
                        tracing::warn!("image node missing src attribute");
                    }
                    return None;
                };

                let alt = attr_value(&attrs, local_name!("alt"));
                let title = attr_value(&attrs, local_name!("title"));
                let (width, height) = attr_width_height(&attrs);

                let mut paragraph = Paragraph::default();
                paragraph.push_image(ImageNode {
                    url: src.into(),
                    link: None,
                    title: title.map(Into::into),
                    alt: alt.map(Into::into),
                    width,
                    height,
                });

                if children.len() > 0 {
                    children.push(node::Node::Paragraph(paragraph));
                    Some(node::Node::Root { children })
                } else {
                    Some(node::Node::Paragraph(paragraph))
                }
            }
            local_name!("ul") | local_name!("ol") => {
                let ordered = name.local == local_name!("ol");
                let children = consume_children_nodes(node, paragraph, cx);
                Some(node::Node::List { children, ordered })
            }
            local_name!("li") => {
                let mut children = vec![];
                consume_paragraph(&mut children, paragraph);

                for child in node.children.borrow().iter() {
                    let mut child_paragraph = Paragraph::default();
                    if let Some(child_node) = parse_node(child, &mut child_paragraph, cx) {
                        children.push(child_node);
                    }
                    if child_paragraph.text_len() > 0 {
                        // If last child is paragraph, merge child
                        if let Some(last_child) = children.last_mut() {
                            if let node::Node::Paragraph(last_paragraph) = last_child {
                                last_paragraph.merge(child_paragraph);
                                continue;
                            }
                        }

                        children.push(node::Node::Paragraph(child_paragraph));
                    }
                }

                consume_paragraph(&mut children, paragraph);

                Some(node::Node::ListItem {
                    children,
                    spread: false,
                    checked: None,
                })
            }
            local_name!("table") => {
                let mut children = vec![];
                consume_paragraph(&mut children, paragraph);

                let mut table = Table::default();
                for child in node.children.borrow().iter() {
                    match child.data {
                        NodeData::Element { ref name, .. }
                            if name.local == local_name!("tbody")
                                || name.local == local_name!("thead") =>
                        {
                            for sub_child in child.children.borrow().iter() {
                                parse_table_row(&mut table, &sub_child);
                            }
                        }
                        _ => {
                            parse_table_row(&mut table, &child);
                        }
                    }
                }
                consume_paragraph(&mut children, paragraph);

                let table = node::Node::Table(table);
                if children.len() > 0 {
                    children.push(table);
                    Some(node::Node::Root { children })
                } else {
                    Some(table)
                }
            }
            local_name!("blockquote") => {
                let children = consume_children_nodes(node, paragraph, cx);
                Some(node::Node::Blockquote { children })
            }
            local_name!("style") | local_name!("script") => None,
            _ => {
                if IGNORED_ELEMENTS.contains(&name.local.trim()) {
                    // Ignore this element
                    None
                } else if BLOCK_ELEMENTS.contains(&name.local.trim()) {
                    let mut children: Vec<node::Node> = vec![];

                    // Case:
                    //
                    // Hello <p>Inner text of block element</p> World

                    // Insert before text as a node -- The "Hello"
                    consume_paragraph(&mut children, paragraph);

                    // Inner of the block element -- The "Inner text of block element"
                    for child in node.children.borrow().iter() {
                        if let Some(child_node) = parse_node(child, paragraph, cx) {
                            children.push(child_node);
                        }
                    }
                    consume_paragraph(&mut children, paragraph);

                    if children.is_empty() {
                        None
                    } else {
                        Some(node::Node::Root { children })
                    }
                } else {
                    // Others to as Inline
                    parse_paragraph(paragraph, node);

                    if paragraph.is_image() {
                        Some(node::Node::Paragraph(paragraph.take()))
                    } else {
                        None
                    }
                }
            }
        },
        NodeData::Document => {
            let children = consume_children_nodes(node, paragraph, cx);
            Some(node::Node::Root { children })
        }
        NodeData::Doctype { .. }
        | NodeData::Comment { .. }
        | NodeData::ProcessingInstruction { .. } => None,
    }
}

fn consume_children_nodes(
    node: &Node,
    paragraph: &mut Paragraph,
    cx: &mut NodeContext,
) -> Vec<node::Node> {
    let mut children = vec![];
    consume_paragraph(&mut children, paragraph);
    for child in node.children.borrow().iter() {
        if let Some(child_node) = parse_node(child, paragraph, cx) {
            children.push(child_node);
        }
        consume_paragraph(&mut children, paragraph);
    }

    children
}

fn consume_paragraph(children: &mut Vec<node::Node>, paragraph: &mut Paragraph) {
    if paragraph.is_empty() {
        return;
    }

    children.push(node::Node::Paragraph(paragraph.take()));
}
