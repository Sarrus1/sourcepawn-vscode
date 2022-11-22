use std::str::Utf8Error;

use tree_sitter::Node;

use crate::{
    fileitem::FileItem,
    spitem::{
        function_item::{FunctionItem, FunctionVisibility},
        SPItem,
    },
    utils::ts_range_to_lsp_range,
};

pub(crate) fn parse_function(file_item: &mut FileItem, node: &mut Node) -> Result<(), Utf8Error> {
    // Name of the function
    let name_node = node.child_by_field_name("name");
    // Return type of the function
    let type_node = node.child_by_field_name("returnType");
    // Visibility of the function (public, static, stock)
    let mut visibility_node: Option<Node> = None;
    // Arguments of the declaration
    let mut args_node: Option<Node> = None;
    // Type of function declaration ("native" or "forward")
    let mut function_type_node: Option<Node> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "function_visibility" => {
                visibility_node = Some(child);
            }
            "argument_declarations" => {
                args_node = Some(child);
            }
            "function_definition_type" => {
                function_type_node = Some(child);
            }
            _ => {
                continue;
            }
        }
    }

    if name_node.is_none() {
        // A function always has a name.
        return Ok(());
    }
    let name_node = name_node.unwrap();
    let name = name_node.utf8_text(&file_item.text.as_bytes());

    let mut type_ = Ok("");
    if type_node.is_some() {
        type_ = type_node.unwrap().utf8_text(&file_item.text.as_bytes());
    }

    let mut visibility = vec![];
    if visibility_node.is_some() {
        let visibility_text = visibility_node
            .unwrap()
            .utf8_text(&file_item.text.as_bytes())?;
        if visibility_text.contains("stock") {
            visibility.push(FunctionVisibility::Stock);
        }
        if visibility_text.contains("public") {
            visibility.push(FunctionVisibility::Public);
        }
        if visibility_text.contains("static") {
            visibility.push(FunctionVisibility::Static);
        }

        eprintln!("visibility {:?}", visibility);
    }

    let function_item = FunctionItem {
        name: name?.to_string(),
        type_: type_?.to_string(),
        range: ts_range_to_lsp_range(&name_node.range()),
        full_range: ts_range_to_lsp_range(&node.range()),
        description: "".to_string(),
        uri_string: file_item.uri.to_string(),
        deprecated: false,
        detail: "".to_string(),
        visibility,
    };

    file_item.sp_items.push(SPItem::Function(function_item));

    Ok(())
}
