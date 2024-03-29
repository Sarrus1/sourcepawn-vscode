use anyhow::Context;
use base_db::FileRange;
use lsp_types::{
    SemanticTokensDeltaParams, SemanticTokensFullDeltaResult, SemanticTokensParams,
    SemanticTokensRangeParams, SemanticTokensRangeResult, SemanticTokensResult, Url,
};
use stdx::format_to;

use crate::{
    global_state::GlobalStateSnapshot,
    lsp::{from_proto, to_proto},
    lsp_ext::{
        AnalyzerStatusParams, ItemTreeParams, PreprocessedDocumentParams, ProjectMainPathParams,
        ProjectsGraphvizParams, SyntaxTreeParams,
    },
};

pub(crate) fn handle_goto_definition(
    snap: GlobalStateSnapshot,
    params: lsp_types::GotoDefinitionParams,
) -> anyhow::Result<Option<lsp_types::GotoDefinitionResponse>> {
    log::debug!("goto def: {:?}", params);
    let pos = from_proto::file_position(&snap, params.text_document_position_params.clone())?;

    let targets = match snap.analysis.goto_definition(pos)? {
        None => return Ok(None),
        Some(it) => it,
    };
    let src = FileRange {
        file_id: pos.file_id,
        range: targets.range,
    };

    Ok(Some(to_proto::goto_definition_response(
        &snap,
        Some(src),
        targets.info,
    )?))
}

pub(crate) fn handle_hover(
    snap: GlobalStateSnapshot,
    params: lsp_types::HoverParams,
) -> anyhow::Result<Option<lsp_types::Hover>> {
    let pos = from_proto::file_position(&snap, params.text_document_position_params.clone())?;

    let hover = match snap.analysis.hover(pos, &snap.config.hover())? {
        None => return Ok(None),
        Some(it) => it,
    };

    let res = lsp_types::Hover {
        contents: lsp_types::HoverContents::Markup(to_proto::markup_content(
            hover.info.markup,
            snap.config.hover().format,
        )),
        range: Some(hover.range),
    };

    Ok(res.into())
}

pub(crate) fn handle_semantic_tokens_full(
    snap: GlobalStateSnapshot,
    params: SemanticTokensParams,
) -> anyhow::Result<Option<SemanticTokensResult>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let text = snap.analysis.file_text(file_id)?;

    let highlights = snap.analysis.highlight(file_id)?;
    let semantic_tokens = to_proto::semantic_tokens(&text, highlights);

    // Unconditionally cache the tokens
    snap.semantic_tokens_cache
        .lock()
        .insert(params.text_document.uri, semantic_tokens.clone());

    Ok(Some(semantic_tokens.into()))
}

pub(crate) fn handle_semantic_tokens_full_delta(
    snap: GlobalStateSnapshot,
    params: SemanticTokensDeltaParams,
) -> anyhow::Result<Option<SemanticTokensFullDeltaResult>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let text = snap.analysis.file_text(file_id)?;

    let highlights = snap.analysis.highlight(file_id)?;

    let semantic_tokens = to_proto::semantic_tokens(&text, highlights);

    let cached_tokens = snap
        .semantic_tokens_cache
        .lock()
        .remove(&params.text_document.uri);

    if let Some(
        cached_tokens @ lsp_types::SemanticTokens {
            result_id: Some(prev_id),
            ..
        },
    ) = &cached_tokens
    {
        if *prev_id == params.previous_result_id {
            let delta = to_proto::semantic_token_delta(cached_tokens, &semantic_tokens);
            snap.semantic_tokens_cache
                .lock()
                .insert(params.text_document.uri, semantic_tokens);
            return Ok(Some(delta.into()));
        }
    }

    // Clone first to keep the lock short
    let semantic_tokens_clone = semantic_tokens.clone();
    snap.semantic_tokens_cache
        .lock()
        .insert(params.text_document.uri, semantic_tokens_clone);

    Ok(Some(semantic_tokens.into()))
}

pub(crate) fn handle_semantic_tokens_range(
    snap: GlobalStateSnapshot,
    params: SemanticTokensRangeParams,
) -> anyhow::Result<Option<SemanticTokensRangeResult>> {
    let frange = from_proto::file_range(&snap, &params.text_document, params.range)?;
    let text = snap.analysis.file_text(frange.file_id)?;

    let highlights = snap.analysis.highlight_range(frange)?;
    let semantic_tokens = to_proto::semantic_tokens(&text, highlights);

    Ok(Some(semantic_tokens.into()))
}

pub(crate) fn handle_syntax_tree(
    snap: GlobalStateSnapshot,
    params: SyntaxTreeParams,
) -> anyhow::Result<String> {
    let _tree = snap.analysis.parse(from_proto::file_id(
        &snap,
        &params
            .text_document
            .context("No text_document parameter passed.")?
            .uri,
    )?)?;

    // Ok(prettify_s_expression(&tree.root_node().to_sexp()))
    Ok("".to_string())
}

pub(crate) fn handle_projects_graphviz(
    snap: GlobalStateSnapshot,
    _params: ProjectsGraphvizParams,
) -> anyhow::Result<String> {
    let graph = snap.analysis.graph()?;

    graph
        .to_graphviz(|id| {
            let path = snap.vfs_read().file_path(id);
            path.name_and_extension()
                .map(|(name, ext)| format!("{}.{}", name, ext.unwrap_or_default()))
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to generate graphviz"))
}

pub(crate) fn handle_preprocessed_document(
    snap: GlobalStateSnapshot,
    params: PreprocessedDocumentParams,
) -> anyhow::Result<String> {
    let uri = params
        .text_document
        .ok_or_else(|| anyhow::anyhow!("No uri received in request"))?
        .uri;
    let file_id = from_proto::file_id(&snap, &uri)?;

    snap.analysis
        .preprocessed_text(file_id)
        .context("Failed to preprocess document")
        .map(|it| it.to_string())
}

pub(crate) fn handle_item_tree(
    snap: GlobalStateSnapshot,
    params: ItemTreeParams,
) -> anyhow::Result<String> {
    let uri = params
        .text_document
        .ok_or_else(|| anyhow::anyhow!("No uri received in request"))?
        .uri;
    let file_id = from_proto::file_id(&snap, &uri)?;

    snap.analysis
        .pretty_item_tree(file_id)
        .context("Failed to get the item tree")
}

pub(crate) fn handle_analyzer_status(
    snap: GlobalStateSnapshot,
    params: AnalyzerStatusParams,
) -> anyhow::Result<String> {
    let mut buf = String::new();

    let mut file_id = None;
    if let Some(tdi) = params.text_document {
        match from_proto::file_id(&snap, &tdi.uri) {
            Ok(it) => file_id = Some(it),
            Err(_) => format_to!(buf, "file {} not found in vfs", tdi.uri),
        }
    }

    // if snap.workspaces.is_empty() {
    //     buf.push_str("No workspaces\n")
    // } else {
    //     buf.push_str("Workspaces:\n");
    //     format_to!(
    //         buf,
    //         "Loaded {:?} packages across {} workspace{}.\n",
    //         snap.workspaces
    //             .iter()
    //             .map(|w| w.n_packages())
    //             .sum::<usize>(),
    //         snap.workspaces.len(),
    //         if snap.workspaces.len() == 1 { "" } else { "s" }
    //     );

    //     format_to!(
    //         buf,
    //         "Workspace root folders: {:?}",
    //         snap.workspaces
    //             .iter()
    //             .flat_map(|ws| ws.workspace_definition_path())
    //             .collect::<Vec<&AbsPath>>()
    //     );
    // }
    format_to!(
        buf,
        "\nVfs memory usage: {}\n",
        profile::Bytes::new(snap.vfs_memory_usage() as _)
    );
    buf.push_str("\nAnalysis:\n");
    buf.push_str(
        &snap
            .analysis
            .status(file_id)
            .unwrap_or_else(|_| "Analysis retrieval was cancelled".to_owned()),
    );
    Ok(buf)
}

pub(crate) fn handle_project_main_path(
    snap: GlobalStateSnapshot,
    params: ProjectMainPathParams,
) -> anyhow::Result<Url> {
    let uri = params
        .uri
        .ok_or_else(|| anyhow::anyhow!("No uri received in request"))?;
    let file_id = from_proto::file_id(&snap, &uri)?;

    snap.analysis
        .projects_for_file(file_id)
        .context("Failed to get project for file")?
        .first()
        .map(|it| to_proto::url(&snap, *it))
        .ok_or_else(|| anyhow::anyhow!("No project found for file"))
}
