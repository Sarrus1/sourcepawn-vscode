use std::sync::{Arc, RwLock};

use super::{Location, SPItem};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemTag, CompletionParams, DocumentSymbol,
    GotoDefinitionParams, Hover, HoverContents, HoverParams, LanguageString, LocationLink,
    MarkedString, Range, SymbolKind, SymbolTag, Url,
};

use crate::providers::hover::description::Description;

#[derive(Debug, Clone)]
/// SPItem representation of a SourcePawn typeset/funcenum, which can be converted to a
/// [CompletionItem](lsp_types::CompletionItem), [Location](lsp_types::Location), etc.
pub struct TypesetItem {
    /// Name of the typeset.
    pub name: String,

    /// Range of the name of the typeset.
    pub range: Range,

    /// Range of the whole typeset.
    pub full_range: Range,

    /// Description of the typeset.
    pub description: Description,

    /// Uri of the file where the typeset is declared.
    pub uri: Arc<Url>,

    /// References to this typeset.
    pub references: Vec<Location>,

    /// Parameters of the typeset.
    pub children: Vec<Arc<RwLock<SPItem>>>,
}

impl TypesetItem {
    fn is_deprecated(&self) -> bool {
        self.description.deprecated.is_some()
    }

    /// Return a [CompletionItem](lsp_types::CompletionItem) from a [TypesetItem].
    ///
    /// # Arguments
    ///
    /// * `_params` - [CompletionParams](lsp_types::CompletionParams) of the request.
    pub(crate) fn to_completion(&self, _params: &CompletionParams) -> Option<CompletionItem> {
        let mut tags = vec![];
        if self.is_deprecated() {
            tags.push(CompletionItemTag::DEPRECATED);
        }

        Some(CompletionItem {
            label: self.name.to_string(),
            kind: Some(CompletionItemKind::INTERFACE),
            tags: Some(tags),
            detail: None,
            deprecated: Some(self.is_deprecated()),
            ..Default::default()
        })
    }

    /// Return a [Hover] from a [TypesetItem].
    ///
    /// # Arguments
    ///
    /// * `_params` - [HoverParams] of the request.
    pub(crate) fn to_hover(&self, _params: &HoverParams) -> Option<Hover> {
        Some(Hover {
            contents: HoverContents::Array(vec![
                self.formatted_text(),
                MarkedString::String(self.description.to_md()),
            ]),
            range: None,
        })
    }

    /// Return a [LocationLink] from a [TypesetItem].
    ///
    /// # Arguments
    ///
    /// * `_params` - [GotoDefinitionParams] of the request.
    pub(crate) fn to_definition(&self, _params: &GotoDefinitionParams) -> Option<LocationLink> {
        Some(LocationLink {
            target_range: self.range,
            target_uri: self.uri.as_ref().clone(),
            target_selection_range: self.range,
            origin_selection_range: None,
        })
    }

    /// Return a [DocumentSymbol] from a [TypesetItem].
    pub(crate) fn to_document_symbol(&self) -> Option<DocumentSymbol> {
        let mut tags = vec![];
        if self.description.deprecated.is_some() {
            tags.push(SymbolTag::DEPRECATED);
        }
        #[allow(deprecated)]
        Some(DocumentSymbol {
            name: self.name.to_string(),
            detail: None,
            kind: SymbolKind::NAMESPACE,
            tags: Some(tags),
            range: self.full_range,
            deprecated: None,
            selection_range: self.range,
            children: Some(
                self.children
                    .iter()
                    .filter_map(|child| child.read().unwrap().to_document_symbol())
                    .collect(),
            ),
        })
    }

    /// Return a vector of [CompletionItem] of all the [TypedefItem](super::typedef_item::TypedefItem)
    /// of a [TypesetItem] for a callback completion.
    ///
    /// # Arguments
    ///
    /// * `range` - [Range] of the "$" that will be replaced.
    pub(crate) fn to_snippet_completion(&self, range: Range) -> Vec<CompletionItem> {
        let mut res = vec![];
        for child in self.children.iter() {
            if let SPItem::Typedef(typedef_item) = &*child.read().unwrap() {
                if let Some(completion) = typedef_item.to_snippet_completion(range) {
                    res.push(completion);
                }
            }
        }

        res
    }

    /// Formatted representation of a [TypesetItem].
    ///
    /// # Exemple
    ///
    /// `typeset EventHook`
    fn formatted_text(&self) -> MarkedString {
        MarkedString::LanguageString(LanguageString {
            language: "sourcepawn".to_string(),
            value: format!("typeset {}", self.name),
        })
    }
}
