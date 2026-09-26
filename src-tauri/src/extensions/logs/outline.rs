//! Lines and blocks as a tree: bookmarks hold the indices inside them, and every line
//! appears exactly once, in its innermost block (qa-log-tui's build_outline).

use super::parser::{Block, Line};
use super::Kind;
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Node {
    Line { line: usize },
    Block { kind: Kind, name: String, first_line: usize, last_line: usize, has_error: bool, children: Vec<Node> },
}

struct Span<'a> {
    block: &'a Block,
    /// last line, a block still open at the end of the file running to it
    end: usize,
}

pub fn build(lines: &[Line], blocks: &[Block]) -> Vec<Node> {
    if lines.is_empty() {
        return vec![];
    }
    let span = |b| Span { block: b, end: b.last_line.unwrap_or(lines.len() - 1) };
    // Stable sorts: blocks starting on the same line keep their order
    let mut bookmarks: Vec<Span> = blocks.iter().filter(|b| b.kind == Kind::Bookmark).map(span).collect();
    let mut indices: Vec<Span> = blocks.iter().filter(|b| b.kind == Kind::Index).map(span).collect();
    bookmarks.sort_by_key(|s| s.block.first_line);
    indices.sort_by_key(|s| s.block.first_line);

    let contains = |outer: &Span, inner: &Span| outer.block.first_line <= inner.block.first_line && inner.end <= outer.end;
    // Each index goes into the first bookmark that contains it, or stays at the top
    let mut children_of: Vec<Vec<&Span>> = bookmarks.iter().map(|_| vec![]).collect();
    let mut top: Vec<(Option<usize>, &Span)> = bookmarks.iter().enumerate().map(|(i, s)| (Some(i), s)).collect();
    for index in &indices {
        match bookmarks.iter().position(|bm| contains(bm, index)) {
            Some(i) => children_of[i].push(index),
            None => top.push((None, index)),
        }
    }
    top.sort_by_key(|(_, s)| s.block.first_line);

    let builder = Builder { lines, children_of };
    builder.sequence(0, lines.len() - 1, &top)
}

struct Builder<'a> {
    lines: &'a [Line],
    children_of: Vec<Vec<&'a Span<'a>>>,
}

impl Builder<'_> {
    /// Sibling blocks with the lines between them, in order. Neighbouring blocks often share
    /// one boundary line (a step's end is the next step's start); it's shown once, as the
    /// first line of the block that opens there.
    fn sequence(&self, mut cursor: usize, end_inclusive: usize, blocks: &[(Option<usize>, &Span)]) -> Vec<Node> {
        let mut out = Vec::new();
        for (bookmark, s) in blocks {
            let start = s.block.first_line.max(cursor);
            if s.end < start {
                continue; // used up by the boundary it shares with the block before
            }
            out.extend((cursor..start).map(|line| Node::Line { line }));
            out.push(self.block(*bookmark, s, start));
            cursor = s.end + 1;
        }
        if cursor <= end_inclusive {
            out.extend((cursor..=end_inclusive).map(|line| Node::Line { line }));
        }
        out
    }

    fn block(&self, bookmark: Option<usize>, s: &Span, start: usize) -> Node {
        let nested: Vec<(Option<usize>, &Span)> = match bookmark {
            Some(i) => {
                let mut list: Vec<&Span> = self.children_of[i].clone();
                list.sort_by_key(|c| c.block.first_line);
                list.into_iter().map(|c| (None, c)).collect()
            }
            None => vec![],
        };
        Node::Block {
            kind: s.block.kind,
            name: if s.block.name.is_empty() {
                (if s.block.kind == Kind::Bookmark { "Bookmark" } else { "Index" }).into()
            } else {
                s.block.name.clone()
            },
            first_line: start,
            last_line: s.end,
            has_error: self.lines[start..=s.end].iter().any(|l| l.level == "ERROR"),
            children: self.sequence(start, s.end, &nested),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::logs::parser::{self, tests::{sections_setup, SAMPLE}};

    fn outline() -> Vec<Node> {
        let lines = parser::parse(SAMPLE, &sections_setup());
        build(&lines, &parser::blocks(&lines, &sections_setup().blocks))
    }

    fn flatten(nodes: &[Node]) -> Vec<&Node> {
        nodes
            .iter()
            .flat_map(|n| match n {
                Node::Block { children, .. } => std::iter::once(n).chain(flatten(children)).collect(),
                Node::Line { .. } => vec![n],
            })
            .collect()
    }

    fn bookmark<'a>(nodes: &'a [Node], wanted: &str) -> &'a Node {
        flatten(nodes)
            .into_iter()
            .find(|n| matches!(n, Node::Block { kind: Kind::Bookmark, name, .. } if name == wanted))
            .unwrap()
    }

    #[test]
    fn every_line_appears_exactly_once() {
        let o = outline();
        let mut seen: Vec<usize> =
            flatten(&o).into_iter().filter_map(|n| if let Node::Line { line } = n { Some(*line) } else { None }).collect();
        assert_eq!(seen.len(), 28);
        seen.sort();
        assert_eq!(seen, (0..28).collect::<Vec<_>>());
    }

    #[test]
    fn bookmarks_are_not_dropped_by_a_shared_boundary() {
        let o = outline();
        let names: Vec<&str> = flatten(&o)
            .into_iter()
            .filter_map(|n| match n {
                Node::Block { kind: Kind::Bookmark, name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, ["Login works", "Logout works"]);
    }

    #[test]
    fn errors_bubble_up_to_the_bookmark() {
        let o = outline();
        assert!(matches!(bookmark(&o, "Login works"), Node::Block { has_error: true, .. }));
        assert!(matches!(bookmark(&o, "Logout works"), Node::Block { has_error: false, .. }));
    }

    #[test]
    fn indices_nest_under_their_bookmark() {
        let o = outline();
        let Node::Block { children, .. } = bookmark(&o, "Login works") else { panic!() };
        let names: Vec<&str> = children
            .iter()
            .filter_map(|c| match c {
                Node::Block { kind: Kind::Index, name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, ["Setup", "Given I am on the login page", "When I submit valid credentials",
                           "Then I should see the dashboard", "Teardown"]);
        assert!(matches!(&o[0], Node::Block { kind: Kind::Index, name, .. } if name == "Environment setup"));
    }

    #[test]
    fn a_flat_log_is_just_lines() {
        let lines = parser::parse(SAMPLE, &crate::extensions::logs::Setup::default());
        let o = build(&lines, &[]);
        assert_eq!(o.len(), 28);
        assert!(o.iter().all(|n| matches!(n, Node::Line { .. })));
        assert!(build(&[], &[]).is_empty());
    }
}
