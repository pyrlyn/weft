//! The layers list and a read of one element (T69 milestone 3). Paths are the diagnostic paths
//! of SPEC §6.1, the same names `element_paths` uses, so a patch can name what the list shows.
//! Nothing here writes the document.

use weft_core::{Child, Document, Node, Value, path_segment};

/// One row of the layers list, in document order: an element, then its children, then each slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub path: String,
    pub kind: String,
    pub id: Option<String>,
    pub depth: u32,
}

/// What the inspector shows for one element. `children` are its direct elements, including the
/// elements of its slots, in the same order as [`layers`].
#[derive(Debug, Clone, PartialEq)]
pub struct Inspection {
    pub path: String,
    pub kind: String,
    pub id: Option<String>,
    pub props: Vec<(String, Value)>,
    pub on: Vec<(String, String)>,
    pub children: Vec<String>,
}

struct Entry {
    path: String,
    kind: String,
    id: Option<String>,
    props: Vec<(String, Value)>,
    on: Vec<(String, String)>,
    children: Vec<String>,
}

/// Every element of `document`, parent before children.
pub fn layers(document: &Document) -> Vec<Layer> {
    walk(document)
        .into_iter()
        .map(|entry| Layer {
            depth: depth(&entry.path),
            path: entry.path,
            kind: entry.kind,
            id: entry.id,
        })
        .collect()
}

/// The element at `path`, or nothing when the path is not in the document.
pub fn inspect(document: &Document, path: &str) -> Option<Inspection> {
    walk(document).into_iter().find_map(|entry| {
        (entry.path == path).then_some(Inspection {
            path: entry.path,
            kind: entry.kind,
            id: entry.id,
            props: entry.props,
            on: entry.on,
            children: entry.children,
        })
    })
}

fn walk(document: &Document) -> Vec<Entry> {
    let root = &document.root;
    let path = format!("/{}", path_segment(&root.kind, root.id.as_deref(), None));
    let mut out = Vec::new();
    descend(root, path, &mut out);
    out
}

fn descend(node: &Node, path: String, out: &mut Vec<Entry>) {
    let index = out.len();
    out.push(Entry {
        path: path.clone(),
        kind: node.kind.clone(),
        id: node.id.clone(),
        props: node
            .props
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
        on: node
            .on
            .iter()
            .map(|(name, action)| (name.clone(), action.clone()))
            .collect(),
        children: Vec::new(),
    });
    let mut children = elements(&node.children, &path, None, out);
    for (name, list) in &node.slots {
        children.extend(elements(list, &path, Some(name), out));
    }
    out[index].children = children;
}

fn elements(list: &[Child], parent: &str, slot: Option<&str>, out: &mut Vec<Entry>) -> Vec<String> {
    let mut paths = Vec::new();
    for (index, child) in list.iter().enumerate() {
        let Child::Node(node) = child else {
            continue;
        };
        let segment = path_segment(&node.kind, node.id.as_deref(), Some(index));
        let path = match slot {
            Some(name) => format!("{parent}/slot[{name}]/{segment}"),
            None => format!("{parent}/{segment}"),
        };
        paths.push(path.clone());
        descend(node, path, out);
    }
    paths
}

fn depth(path: &str) -> u32 {
    u32::try_from(path.matches('/').count())
        .unwrap_or(u32::MAX)
        .saturating_sub(1)
}
