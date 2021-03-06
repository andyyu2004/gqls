#![deny(rust_2018_idioms)]

mod nodes;
mod traverse;

pub use self::nodes::NodeKind;
pub use self::traverse::{Traverse, TraverseEvent};

pub use tree_sitter::{Language, Node, Parser, Point, Query, QueryCursor, Range, Tree, TreeCursor};
use vfs::FileId;

use std::fmt::{self, Debug};

pub fn traverse(tree: &Tree) -> Traverse<'_> {
    Traverse::new(tree.walk())
}

pub fn traverse_preorder(tree: &Tree) -> impl Iterator<Item = Node<'_>> {
    tree_sitter_traversal::traverse_tree(tree, tree_sitter_traversal::Order::Pre)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    pub file: FileId,
    pub point: Point,
}

impl Position {
    pub fn new(file: FileId, point: Point) -> Self {
        Self { file, point }
    }
}

pub struct Parents<'tree> {
    node: Option<Node<'tree>>,
}

impl<'tree> Iterator for Parents<'tree> {
    type Item = Node<'tree>;

    fn next(&mut self) -> Option<Self::Item> {
        let parent = self.node?.parent();
        self.node = parent;
        parent
    }
}

pub type NodeIterator<'a, 'tree> = Box<dyn Iterator<Item = Node<'tree>> + 'a>;

pub trait RangeExt {
    fn is_empty(&self) -> bool;
    fn contains(&self, point: Point) -> bool;
    fn intersects(&self, other: Self) -> bool;
    fn debug(&self) -> RangeDebug;
}

pub struct RangeDebug(Range);

impl Debug for RangeDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Range { start_point, end_point, .. } = self.0;
        write!(
            f,
            "{}:{}..{}:{}",
            start_point.row, start_point.column, end_point.row, end_point.column
        )
    }
}

impl RangeExt for Range {
    fn contains(&self, point: Point) -> bool {
        self.start_point <= point && point < self.end_point
    }

    fn intersects(&self, other: Self) -> bool {
        self.end_byte.min(other.end_byte) > self.start_byte.max(other.start_byte)
    }

    fn debug(&self) -> RangeDebug {
        RangeDebug(*self)
    }

    fn is_empty(&self) -> bool {
        assert!(self.start_byte <= self.end_byte);
        self.start_byte == self.end_byte
    }
}

pub trait QueryExt: Sized {
    fn is_match(&self, cursor: &mut QueryCursor, node: Node<'_>, text_provider: &[u8]) -> bool;
}

impl QueryExt for Query {
    fn is_match(&self, cursor: &mut QueryCursor, node: Node<'_>, text_provider: &[u8]) -> bool {
        cursor.matches(self, node, text_provider).next().is_some()
    }
}

// FIXME avoid boxed iterators once impl trait type alias etc is stable
pub trait NodeExt<'tree>: Sized {
    fn parents(self) -> Parents<'tree>;
    fn parent_of_kind(self, kind: &'static str) -> Option<Self>;
    fn has_parent_of_kind(self, kind: &'static str) -> bool;
    fn sole_named_child(self) -> Option<Node<'tree>>;
    fn text(self, text: &str) -> &str;
    fn find_descendant(self, f: impl FnMut(&Self) -> bool) -> Option<Self>;
    fn name_node(self) -> Option<Self>;
    fn child_of_kind(self, kind: &'static str) -> Option<Self>;
    fn named_node_at(self, at: Point) -> Option<Self>;
    fn named_descendant_for_range(self, range: Range) -> Option<Self>;
    fn relevant_children<'a>(self, cursor: &'a mut TreeCursor<'tree>) -> NodeIterator<'a, 'tree>;
    fn children_of_kind<'a>(
        self,
        cursor: &'a mut TreeCursor<'tree>,
        kind: &'static str,
    ) -> NodeIterator<'a, 'tree>;
}

impl<'tree> NodeExt<'tree> for Node<'tree> {
    fn parents(self) -> Parents<'tree> {
        Parents { node: Some(self) }
    }

    fn parent_of_kind(self, kind: &'static str) -> Option<Self> {
        self.parents().find(|node| node.kind() == kind)
    }

    fn has_parent_of_kind(self, kind: &'static str) -> bool {
        self.parent_of_kind(kind).is_some()
    }

    #[track_caller]
    fn sole_named_child(self) -> Option<Node<'tree>> {
        // ERROR nodes seem to be included in `named_child_count` for whatever reason so we hack around it
        if self.named_child_count() > 1 {
            let mut cursor = self.walk();
            let mut children = self.named_children(&mut cursor).filter(|child| !child.is_error());
            let child = children.next();
            assert!(
                children.next().is_none(),
                "node `{}` had more than one named child",
                self.to_sexp()
            );
            child
        } else {
            self.named_child(0)
        }
    }

    fn text(self, source: &str) -> &str {
        self.utf8_text(source.as_bytes()).expect("text was not valid utf8")
    }

    fn find_descendant(self, f: impl FnMut(&Node<'tree>) -> bool) -> Option<Node<'tree>> {
        tree_sitter_traversal::traverse(self.walk(), tree_sitter_traversal::Order::Pre).find(f)
    }

    fn name_node(self) -> Option<Self> {
        self.child_of_kind(NodeKind::NAME).or_else(|| self.child_of_kind(NodeKind::DIRECTIVE_NAME))
    }

    fn child_of_kind(self, kind: &'static str) -> Option<Self> {
        self.named_children(&mut self.walk()).find(|node| node.kind() == kind)
    }

    fn named_node_at(self, at: Point) -> Option<Self> {
        self.named_descendant_for_point_range(at, at)
    }

    fn named_descendant_for_range(self, range: Range) -> Option<Self> {
        self.named_descendant_for_point_range(range.start_point, range.end_point)
    }

    fn relevant_children<'a>(self, cursor: &'a mut TreeCursor<'tree>) -> NodeIterator<'a, 'tree> {
        Box::new(self.named_children(cursor).filter(|node| !node.is_extra()))
    }

    fn children_of_kind<'a>(
        self,
        cursor: &'a mut TreeCursor<'tree>,
        kind: &'static str,
    ) -> NodeIterator<'a, 'tree> {
        Box::new(self.relevant_children(cursor).filter(move |node| node.kind() == kind))
    }
}

extern "C" {
    fn tree_sitter_graphql() -> Language;
}

pub fn language() -> Language {
    unsafe { tree_sitter_graphql() }
}

pub fn parse_fresh(text: &str) -> Tree {
    parse(text, None)
}

pub fn parse(text: &str, old_tree: Option<&Tree>) -> Tree {
    let mut parser = make_parser();
    parser.parse(text, old_tree).unwrap()
}

pub fn query(query: &str) -> Query {
    Query::new(language(), query).unwrap()
}

fn make_parser() -> Parser {
    let mut parser = Parser::new();
    parser.set_language(language()).unwrap();
    parser
}

#[cfg(test)]
mod node_generator;

#[cfg(test)]
mod tests;
