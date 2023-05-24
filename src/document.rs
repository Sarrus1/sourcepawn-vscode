use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use fxhash::{FxHashMap, FxHashSet};
use lazy_static::lazy_static;
use lsp_types::Range;
use lsp_types::Url;
use tree_sitter::{Node, Query, QueryCursor};

use crate::{
    linter::document_diagnostics::DocumentDiagnostics,
    parser::comment_parser::{Comment, Deprecated},
    sourcepawn_preprocessor::preprocessor::{Macro, Offset},
    spitem::SPItem,
    utils::ts_range_to_lsp_range,
};

lazy_static! {
    static ref SYMBOL_QUERY: Query = {
        Query::new(
            tree_sitter_sourcepawn::language(),
            "[(symbol) @symbol (this) @symbol]",
        )
        .unwrap()
    };
}

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub range: Range,
}

impl Token {
    pub fn new(node: Node, source: &String) -> Self {
        Self {
            text: node.utf8_text(source.as_bytes()).unwrap().to_string(),
            range: ts_range_to_lsp_range(&node.range()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Document {
    pub uri: Arc<Url>,
    pub text: String,
    pub preprocessed_text: String,
    pub sp_items: Vec<Arc<RwLock<SPItem>>>,
    pub includes: FxHashMap<Url, Token>,
    pub parsed: bool,
    pub tokens: Vec<Arc<Token>>,
    pub missing_includes: FxHashMap<String, Range>,
    pub unresolved_tokens: FxHashSet<String>,
    pub declarations: FxHashMap<String, Arc<RwLock<SPItem>>>,
    pub diagnostics: DocumentDiagnostics,
    pub(crate) macros: FxHashMap<String, Macro>,
    pub(crate) macro_symbols: Vec<Arc<Token>>,
    pub(crate) offsets: FxHashMap<u32, Vec<Offset>>,
}

pub struct Walker {
    pub comments: Vec<Comment>,
    pub deprecated: Vec<Deprecated>,
    pub anon_enum_counter: u32,
}

impl Document {
    pub fn new(uri: Arc<Url>, text: String) -> Self {
        Self {
            uri,
            preprocessed_text: String::new(),
            text,
            sp_items: vec![],
            includes: FxHashMap::default(),
            parsed: false,
            tokens: vec![],
            missing_includes: FxHashMap::default(),
            unresolved_tokens: FxHashSet::default(),
            declarations: FxHashMap::default(),
            diagnostics: DocumentDiagnostics::default(),
            macros: FxHashMap::default(),
            macro_symbols: vec![],
            offsets: FxHashMap::default(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn extension(&self) -> anyhow::Result<String> {
        let extension = self
            .path()?
            .extension()
            .ok_or_else(|| anyhow!("Failed to get file extension."))?
            .to_str()
            .ok_or_else(|| anyhow!("Failed to convert extension to string."))?
            .to_string();

        Ok(extension)
    }

    pub(crate) fn path(&self) -> anyhow::Result<PathBuf> {
        let path = self
            .uri
            .to_file_path()
            .map_err(|e| anyhow!("Failed to convert URI to file path: {:?}.", e))?;

        Ok(path)
    }

    pub(crate) fn uri(&self) -> Url {
        (*self.uri.as_ref()).clone()
    }

    pub fn extract_tokens(&mut self, root_node: Node) {
        let mut cursor = QueryCursor::new();
        let matches = cursor.captures(&SYMBOL_QUERY, root_node, self.preprocessed_text.as_bytes());
        for (match_, _) in matches {
            for capture in match_.captures.iter() {
                self.tokens
                    .push(Arc::new(Token::new(capture.node, &self.preprocessed_text)));
            }
        }
    }

    pub fn add_macro_symbols(&mut self) {
        self.tokens.extend(self.macro_symbols.clone())
    }

    pub fn line(&self, line_nb: u32) -> Option<&str> {
        for (i, line) in self.preprocessed_text.lines().enumerate() {
            if i == line_nb as usize {
                return Some(line);
            }
        }

        None
    }

    pub fn sp_items(&self) -> Vec<Arc<RwLock<SPItem>>> {
        let mut sp_items = vec![];
        for item in self.sp_items.iter() {
            sp_items.push(item.clone());
        }

        sp_items
    }

    pub fn sp_items_flat(&self) -> Vec<Arc<RwLock<SPItem>>> {
        let mut sp_items = vec![];
        for item in self.sp_items.iter() {
            sp_items.push(item.clone());
            match &*item.read().unwrap() {
                SPItem::Function(function_item) => {
                    for child_item in function_item.children.iter() {
                        sp_items.push(child_item.clone())
                    }
                }
                SPItem::Enum(enum_item) => {
                    for child_item in enum_item.children.iter() {
                        sp_items.push(child_item.clone())
                    }
                }
                SPItem::EnumStruct(es_item) => {
                    for child_item in es_item.children.iter() {
                        sp_items.push(child_item.clone());
                        match &*child_item.read().unwrap() {
                            SPItem::Function(method_item) => {
                                for sub_child_item in method_item.children.iter() {
                                    sp_items.push(sub_child_item.clone());
                                }
                            }
                            SPItem::EnumMember(_)
                            | SPItem::Typedef(_)
                            | SPItem::Typeset(_)
                            | SPItem::Variable(_)
                            | SPItem::Property(_)
                            | SPItem::Include(_)
                            | SPItem::Methodmap(_)
                            | SPItem::Enum(_)
                            | SPItem::EnumStruct(_)
                            | SPItem::Define(_) => {}
                        }
                    }
                }
                SPItem::Methodmap(mm_item) => {
                    for child_item in mm_item.children.iter() {
                        sp_items.push(child_item.clone());
                        match &*child_item.read().unwrap() {
                            SPItem::Function(method_item) => {
                                for sub_child_item in method_item.children.iter() {
                                    sp_items.push(sub_child_item.clone());
                                }
                            }
                            SPItem::EnumMember(_)
                            | SPItem::Typedef(_)
                            | SPItem::Typeset(_)
                            | SPItem::Property(_)
                            | SPItem::Variable(_)
                            | SPItem::Include(_)
                            | SPItem::Methodmap(_)
                            | SPItem::Enum(_)
                            | SPItem::EnumStruct(_)
                            | SPItem::Define(_) => {}
                        }
                    }
                }
                SPItem::Typeset(ts_item) => {
                    for child_item in ts_item.children.iter() {
                        sp_items.push(child_item.clone())
                    }
                }
                SPItem::Variable(_)
                | SPItem::Typedef(_)
                | SPItem::EnumMember(_)
                | SPItem::Property(_)
                | SPItem::Include(_)
                | SPItem::Define(_) => {}
            }
        }

        sp_items
    }

    pub fn build_v_range(&self, range: &Range) -> Range {
        let mut start = range.start;
        let mut end = range.end;

        if let Some(start_offsets) = self.offsets.get(&start.line) {
            for offset in start_offsets.iter() {
                if offset.col < start.character {
                    start.character = start
                        .character
                        .checked_add_signed(-offset.diff)
                        .unwrap_or(0);
                }
            }
        }

        if let Some(end_offsets) = self.offsets.get(&end.line) {
            for offset in end_offsets.iter() {
                if offset.col < end.character {
                    end.character = end.character.checked_add_signed(-offset.diff).unwrap_or(0);
                }
            }
        }

        Range { start, end }
    }
}
