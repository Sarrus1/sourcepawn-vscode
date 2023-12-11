use std::ops::Index;
use std::sync::Arc;

use base_db::Tree;
use fxhash::FxHashMap;
use la_arena::{Arena, Idx};
use syntax::TSKind;
use vfs::FileId;

use crate::DefDatabase;

/// Not a _pointer_ in the memory sense, rather a location in a [Tree-Sitter](tree_sitter) syntax tree.
/// [NodePtr] is used by the [AstIdMap] to provide an incremental computation barrier for Salsa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodePtr {
    /// Kind of the [node](tree_sitter::Node).
    ///
    /// Sometimes two nodes are exactly overlapping (same start and end), for instance a single function
    /// in a source file will have the same start and end positions as the `source_file` node.
    /// `kind` allows to disambiguate between two overlapping nodes.
    kind: TSKind,

    /// Start byte of the [node](tree_sitter::Node).
    start_byte: usize,

    /// End byte of the [node](tree_sitter::Node).
    end_byte: usize,
}

impl From<&tree_sitter::Node<'_>> for NodePtr {
    fn from(node: &tree_sitter::Node<'_>) -> Self {
        NodePtr {
            kind: TSKind::from(*node),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        }
    }
}

impl NodePtr {
    pub fn to_node(self, tree: &'_ Tree) -> tree_sitter::Node<'_> {
        let mut node = tree.root_node();
        loop {
            if node.start_byte() == self.start_byte
                && node.end_byte() == self.end_byte
                && TSKind::from(node) == self.kind
            {
                return node;
            }
            let mut found = false;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.start_byte() <= self.start_byte && child.end_byte() >= self.end_byte {
                    node = child;
                    found = true;
                    break;
                }
            }
            if !found {
                panic!("failed to find node")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AstId {
    raw: Idx<NodePtr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstIdMap {
    arena: Arena<NodePtr>,
    map: FxHashMap<NodePtr, AstId>,
}

impl AstIdMap {
    pub fn from_tree(db: &dyn DefDatabase, file_id: FileId) -> Arc<Self> {
        let tree = db.parse(file_id);
        Arc::new(AstIdMap::from_source(&tree.root_node()))
    }

    fn from_source(root_node: &tree_sitter::Node) -> Self {
        assert!(root_node.parent().is_none());
        let mut arena = Arena::default();
        let mut map = FxHashMap::default();
        bdfs(root_node, &mut |node: tree_sitter::Node<'_>| {
            match TSKind::from(node) {
                TSKind::sym_source_file => (),
                TSKind::sym_global_variable_declaration
                | TSKind::sym_variable_declaration_statement => {
                    for child in node.children(&mut node.walk()) {
                        if TSKind::from(child) == TSKind::sym_variable_declaration {
                            let node_ptr = NodePtr::from(&child);
                            let ast_id = arena.alloc(node_ptr);
                            map.insert(node_ptr, AstId { raw: ast_id });
                        }
                    }
                }
                _ => {
                    let node_ptr = NodePtr::from(&node);
                    let ast_id = arena.alloc(node_ptr);
                    map.insert(node_ptr, AstId { raw: ast_id });
                }
            }
            matches!(
                TSKind::from(node),
                TSKind::sym_function_declaration
                    | TSKind::sym_block
                    | TSKind::sym_for_statement
                    | TSKind::sym_while_statement
            )
        });
        AstIdMap { arena, map }
    }

    pub fn ast_id_of(&self, node: &tree_sitter::Node) -> AstId {
        match self.map.get(&NodePtr::from(node)) {
            Some(id) => *id,
            None => {
                for (k, v) in self.map.iter() {
                    log::info!("k: {:?}, v: {:?}", k, v)
                }
                panic!("Failed to find: {:?}", NodePtr::from(node))
            }
        }
    }

    pub(crate) fn get_raw(&self, id: AstId) -> NodePtr {
        self.arena[id.raw]
    }
}

fn bdfs(node: &tree_sitter::Node, f: &mut impl FnMut(tree_sitter::Node) -> bool) {
    let cursor = &mut node.walk();
    let mut nodes = vec![];
    for child in node.children(cursor) {
        if f(child) {
            nodes.push(child);
        }
    }
    for child in nodes {
        bdfs(&child, f);
    }
}

impl Index<AstId> for AstIdMap {
    type Output = NodePtr;
    fn index(&self, index: AstId) -> &Self::Output {
        &self.arena[index.raw]
    }
}
