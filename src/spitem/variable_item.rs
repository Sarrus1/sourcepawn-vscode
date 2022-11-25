use std::sync::Arc;

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemTag, CompletionParams, Position, Range, Url,
};

use super::SPItem;

#[derive(Debug, Clone)]
/// SPItem representation of a SourcePawn variable.
pub struct VariableItem {
    /// Name of the variable.
    pub name: String,

    /// Type of the variable.
    pub type_: String,

    /// Range of the name of the variable.
    pub range: Range,

    /// Description of the variable.
    pub description: String,

    /// Uri of the file where the variable is declared.
    pub uri: Arc<Url>,

    /// Whether the variable is deprecated.
    pub deprecated: bool,

    /// Full variable signature.
    pub detail: String,

    /// Visibility of the variable.
    pub visibility: Vec<VariableVisibility>,
    // references: Location[];
    pub parent: Option<Arc<SPItem>>,
}

impl VariableItem {
    /// Return a [CompletionItem](lsp_types::CompletionItem) from a [VariableItem].
    ///
    /// If the conditions are not appropriate (ex: asking for a static outside of its scope), return None.
    ///
    /// # Arguments
    ///
    /// * `variable_item` - [VariableItem] to convert.
    /// * `params` - [CompletionParams](lsp_types::CompletionParams) of the request.
    pub(crate) fn to_completion(&self, params: &CompletionParams) -> Option<CompletionItem> {
        let mut tags = vec![];
        if self.deprecated {
            tags.push(CompletionItemTag::DEPRECATED);
        }
        if self.parent.is_some() {
            if self.uri.to_string() != params.text_document_position.text_document.uri.to_string() {
                return None;
            }
            let parent = Arc::clone(&self.parent.as_ref().unwrap());
            let parent_range = match &*parent {
                SPItem::Function(parent) => parent.full_range,
                _ => todo!(),
            };
            eprintln!(
                "{:?} {:?}",
                parent_range, params.text_document_position.position
            );
            if !range_contains_pos(parent_range, params.text_document_position.position) {
                return None;
            }
        }

        Some(CompletionItem {
            label: self.name.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            tags: Some(tags),
            ..Default::default()
        })
    }
}

/// Visibility of a SourcePawn variable.
#[derive(Debug, PartialEq, Clone)]
pub enum VariableVisibility {
    Public,
    Static,
    Stock,
}

fn range_contains_pos(range: Range, position: Position) -> bool {
    if range.start.line < position.line && range.end.line > position.line {
        return true;
    }
    if range.start.character <= position.character && range.end.character >= position.character {
        return false;
    }
    if range.start.line == position.line || range.end.line == position.line {
        return true;
    }
    return false;
}
