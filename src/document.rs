use html5ever::tendril::{ByteTendril, ReadExt, StrTendril};

use node::{self, Node};
use predicate::Predicate;
use selection::Selection;

use std::io;

/// An HTML document.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub nodes: Vec<node::Raw>,
}

impl Document {
    /// Returns a `Selection` containing nodes passing the given predicate `p`.
    pub fn find<P: Predicate>(&self, predicate: P) -> Find<P> {
        Find {
            document: self,
            next: 0,
            predicate: predicate,
        }
    }

    /// Returns the `n`th node of the document as a `Some(Node)`, indexed from
    /// 0, or `None` if n is greater than or equal to the number of nodes.
    pub fn nth(&self, n: usize) -> Option<Node> {
        Node::new(self, n)
    }

    pub fn from_read<R: io::Read>(mut readable: R) -> io::Result<Document> {
        let mut byte_tendril = ByteTendril::new();
        readable.read_to_tendril(&mut byte_tendril)?;

        match byte_tendril.try_reinterpret() {
            Ok(str_tendril) => Ok(Document::from(str_tendril)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            )),
        }
    }
}

impl From<StrTendril> for Document {
    /// Parses the given `StrTendril` into a `Document`.
    fn from(tendril: StrTendril) -> Document {
        use html5ever::tendril::stream::TendrilSink;
        use html5ever::{parse_document, rcdom};

        let mut document = Document { nodes: vec![] };

        let rc_dom = parse_document(rcdom::RcDom::default(), Default::default()).one(tendril);
        recur(&mut document, &rc_dom.document, None, None);
        return document;

        fn recur(
            document: &mut Document,
            node: &rcdom::Handle,
            parent: Option<usize>,
            prev: Option<usize>,
        ) -> Option<usize> {
            match node.data {
                rcdom::NodeData::Document => {
                    let mut prev = None;
                    for child in node.children.borrow().iter() {
                        prev = recur(document, &child, None, prev)
                    }
                    None
                }
                rcdom::NodeData::Text { ref contents } => {
                    let data = node::Data::Text((contents.clone().into_inner()).to_string());
                    Some(append(document, data, parent, prev))
                }
                rcdom::NodeData::Comment { ref contents } => {
                    let data = node::Data::Comment((&contents).to_string());
                    Some(append(document, data, parent, prev))
                }
                rcdom::NodeData::Element {
                    ref name,
                    ref attrs,
                    ..
                } => {
                    let name = (&name.local).to_string();
                    let attrs = attrs
                        .borrow()
                        .iter()
                        .map(|attr| ((&attr.name.local).to_string(), (&attr.value).to_string()))
                        .collect();
                    let data = node::Data::Element(name, attrs);
                    let index = append(document, data, parent, prev);
                    let mut prev = None;
                    for child in node.children.borrow().iter() {
                        prev = recur(document, &child, Some(index), prev)
                    }
                    Some(index)
                }
                _ => None,
            }
        }

        fn append(
            document: &mut Document,
            data: node::Data,
            parent: Option<usize>,
            prev: Option<usize>,
        ) -> usize {
            let index = document.nodes.len();

            document.nodes.push(node::Raw {
                index: index,
                parent: parent,
                prev: prev,
                next: None,
                first_child: None,
                last_child: None,
                data: data,
            });

            if let Some(parent) = parent {
                let parent = &mut document.nodes[parent];
                if parent.first_child.is_none() {
                    parent.first_child = Some(index);
                }
                parent.last_child = Some(index);
            }

            if let Some(prev) = prev {
                document.nodes[prev].next = Some(index);
            }

            index
        }
    }
}

impl<'a> From<&'a str> for Document {
    /// Parses the given `&str` into a `Document`.
    fn from(str: &str) -> Document {
        Document::from(StrTendril::from(str))
    }
}

pub struct Find<'a, P> {
    document: &'a Document,
    next: usize,
    predicate: P,
}

impl<'a, P: Predicate> Find<'a, P> {
    pub fn into_selection(self) -> Selection<'a> {
        Selection::new(self.document, self.map(|node| node.index()).collect())
    }
}

impl<'a, P: Predicate> Iterator for Find<'a, P> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Node<'a>> {
        while self.next < self.document.nodes.len() {
            let node = self.document.nth(self.next).unwrap();
            self.next += 1;
            if self.predicate.matches(&node) {
                return Some(node);
            }
        }
        None
    }
}
