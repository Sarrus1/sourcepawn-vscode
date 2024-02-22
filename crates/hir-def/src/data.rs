use std::sync::Arc;

use fxhash::FxHashMap;
use la_arena::{Arena, ArenaMap, Idx};
use smol_str::ToSmolStr;
use syntax::TSKind;

use crate::{
    hir::type_ref::TypeRef,
    item_tree::{EnumStructItemId, MethodmapItemId, Name},
    src::{HasChildSource, HasSource},
    DefDatabase, EnumStructId, FunctionId, FunctionLoc, InFile, Intern, ItemTreeId, LocalFieldId,
    LocalPropertyId, Lookup, MacroId, MethodmapId, NodePtr, PropertyId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionData {
    pub name: Name,
    pub type_ref: Option<TypeRef>,
}

impl FunctionData {
    pub(crate) fn function_data_query(db: &dyn DefDatabase, id: FunctionId) -> Arc<FunctionData> {
        let loc = id.lookup(db).id;
        let item_tree = loc.tree_id().item_tree(db);
        let function = &item_tree[loc.value];
        let function_data = FunctionData {
            name: function.name.clone(),
            type_ref: function.ret_type.clone(),
        };

        Arc::new(function_data)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroData {
    pub name: Name,
}

impl MacroData {
    pub(crate) fn macro_data_query(db: &dyn DefDatabase, id: MacroId) -> Arc<MacroData> {
        let loc = id.lookup(db).id;
        let item_tree = loc.tree_id().item_tree(db);
        let macro_ = &item_tree[loc.value];
        let macro_data = MacroData {
            name: macro_.name.clone(),
        };

        Arc::new(macro_data)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodmapData {
    pub name: Name,
    pub items: Arc<Arena<MethodmapItemData>>,
    pub items_map: Arc<FxHashMap<Name, Idx<MethodmapItemData>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodmapItemData {
    Property(PropertyData),
    Method(FunctionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyItem {
    Getter(FunctionId),
    Setter(FunctionId),
}

impl PropertyItem {
    pub fn function_id(&self) -> FunctionId {
        match self {
            PropertyItem::Getter(id) | PropertyItem::Setter(id) => *id,
        }
    }
}

/// A single property of a methodmap
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyData {
    pub name: Name,
    pub type_ref: TypeRef,
    pub getters_setters: Vec<PropertyItem>,
}

impl MethodmapData {
    pub(crate) fn methodmap_data_query(
        db: &dyn DefDatabase,
        id: MethodmapId,
    ) -> Arc<MethodmapData> {
        let loc = id.lookup(db).id;
        let item_tree = loc.tree_id().item_tree(db);
        let methodmap = &item_tree[loc.value];
        let mut items = Arena::new();
        let mut items_map = FxHashMap::default();
        methodmap.items.iter().for_each(|e| match *e {
            MethodmapItemId::Property(property_idx) => {
                let property = &item_tree[property_idx];
                let property_data = MethodmapItemData::Property(PropertyData {
                    name: property.name.clone(),
                    type_ref: property.type_ref.clone(),
                    getters_setters: property
                        .getters_setters
                        .clone()
                        .map(|fn_id| {
                            let id = FunctionLoc {
                                container: id.into(),
                                id: ItemTreeId {
                                    tree: loc.tree_id(),
                                    value: fn_id,
                                },
                            }
                            .intern(db);
                            let data = db.function_data(id);
                            match data.name.to_smolstr().as_str() {
                                "get" => PropertyItem::Getter(id),
                                "set" => PropertyItem::Setter(id),
                                _ => unreachable!("Invalid getter/setter function"),
                            }
                        })
                        .collect(),
                });
                let property_id = items.alloc(property_data);
                items_map.insert(property.name.clone(), property_id);
            }
            MethodmapItemId::Method(method_idx) => {
                let method = &item_tree[method_idx];
                let fn_id = FunctionLoc {
                    container: id.into(),
                    id: ItemTreeId {
                        tree: loc.tree_id(),
                        value: method_idx,
                    },
                }
                .intern(db);
                let method_id = items.alloc(MethodmapItemData::Method(fn_id));
                // FIXME: Not sure if we should intern like this...
                items_map.insert(method.name.clone(), method_id);
            } // TODO: Add diagnostic for duplicate methodmap items
        });
        // FIXME: Should we look up the inherited methodmap and add its items to the items_map?
        let methodmap_data = MethodmapData {
            name: methodmap.name.clone(),
            items: Arc::new(items),
            items_map: Arc::new(items_map),
        };

        Arc::new(methodmap_data)
    }

    pub fn item(&self, item: Idx<MethodmapItemData>) -> &MethodmapItemData {
        &self.items[item]
    }

    pub fn method(&self, item: Idx<MethodmapItemData>) -> Option<&FunctionId> {
        match &self.items[item] {
            MethodmapItemData::Property(_) => None,
            MethodmapItemData::Method(function_id) => Some(function_id),
        }
    }

    pub fn property(&self, item: Idx<MethodmapItemData>) -> Option<&PropertyData> {
        match &self.items[item] {
            MethodmapItemData::Property(property_data) => Some(property_data),
            MethodmapItemData::Method(_) => None,
        }
    }

    pub fn items(&self, name: &Name) -> Option<Idx<MethodmapItemData>> {
        self.items_map.get(name).cloned()
    }

    pub fn property_type(&self, property: Idx<MethodmapItemData>) -> Option<&TypeRef> {
        match &self.items[property] {
            MethodmapItemData::Property(property_data) => Some(&property_data.type_ref),
            MethodmapItemData::Method(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumStructData {
    pub name: Name,
    pub items: Arc<Arena<EnumStructItemData>>,
    pub items_map: Arc<FxHashMap<Name, Idx<EnumStructItemData>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumStructItemData {
    Field(FieldData),
    Method(FunctionId),
}

/// A single field of a struct
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldData {
    pub name: Name,
    pub type_ref: TypeRef,
}

impl EnumStructData {
    pub(crate) fn enum_struct_data_query(
        db: &dyn DefDatabase,
        id: EnumStructId,
    ) -> Arc<EnumStructData> {
        let loc = id.lookup(db).id;
        let item_tree = loc.tree_id().item_tree(db);
        let enum_struct = &item_tree[loc.value];
        let mut items = Arena::new();
        let mut items_map = FxHashMap::default();
        enum_struct.items.iter().for_each(|e| match e {
            EnumStructItemId::Field(field_idx) => {
                let field = &item_tree[*field_idx];
                let field_data = EnumStructItemData::Field(FieldData {
                    name: field.name.clone(),
                    type_ref: field.type_ref.clone(),
                });
                let field_id = items.alloc(field_data);
                items_map.insert(field.name.clone(), field_id);
            }
            EnumStructItemId::Method(method_idx) => {
                let method = &item_tree[*method_idx];
                let fn_id = FunctionLoc {
                    container: id.into(),
                    id: ItemTreeId {
                        tree: loc.tree_id(),
                        value: *method_idx,
                    },
                }
                .intern(db);
                let method_id = items.alloc(EnumStructItemData::Method(fn_id));
                // FIXME: Not sure if we should intern like this...
                items_map.insert(method.name.clone(), method_id);
            } // TODO: Add diagnostic for duplicate enum struct items
        });
        let enum_struct_data = EnumStructData {
            name: enum_struct.name.clone(),
            items: Arc::new(items),
            items_map: Arc::new(items_map),
        };

        Arc::new(enum_struct_data)
    }

    pub fn item(&self, item: Idx<EnumStructItemData>) -> &EnumStructItemData {
        &self.items[item]
    }

    pub fn method(&self, item: Idx<EnumStructItemData>) -> Option<&FunctionId> {
        match &self.items[item] {
            EnumStructItemData::Field(_) => None,
            EnumStructItemData::Method(function_id) => Some(function_id),
        }
    }

    pub fn items(&self, name: &Name) -> Option<Idx<EnumStructItemData>> {
        self.items_map.get(name).cloned()
    }

    pub fn field_type(&self, field: Idx<EnumStructItemData>) -> Option<&TypeRef> {
        match &self.items[field] {
            EnumStructItemData::Field(field_data) => Some(&field_data.type_ref),
            EnumStructItemData::Method(_) => None,
        }
    }
}

impl HasChildSource<LocalFieldId> for EnumStructId {
    type Value = NodePtr;

    fn child_source(&self, db: &dyn DefDatabase) -> InFile<ArenaMap<LocalFieldId, Self::Value>> {
        let loc = self.lookup(db).id;
        let mut map = ArenaMap::default();
        let tree = db.parse(loc.file_id());
        // We use fields to get the Idx of the field, even if they are dropped at the end of the call.
        // The Idx will be the same when we rebuild the EnumStructData.
        // It feels like we could just be inserting empty data...
        // Why can't we just treat fields as item_tree members instead of making them local to the enum_struct?
        // TODO: Is there a better way to do this?
        // FIXME: Why does it feel like we are doing this twice?
        let mut items = Arena::new();
        let enum_struct_node = loc.source(db, &tree).value;
        for child in enum_struct_node
            .children(&mut enum_struct_node.walk())
            .filter(|c| TSKind::from(c) == TSKind::enum_struct_field)
        {
            let name_node = child.child_by_field_name("name").unwrap();
            let name = Name::from_node(&name_node, &db.preprocessed_text(loc.file_id()));
            let type_ref_node = child.child_by_field_name("type").unwrap();
            let type_ref =
                TypeRef::from_node(&type_ref_node, &db.preprocessed_text(loc.file_id())).unwrap();
            let field = EnumStructItemData::Field(FieldData { name, type_ref });
            map.insert(items.alloc(field), NodePtr::from(&child));
        }
        InFile::new(loc.file_id(), map)
    }
}

impl HasChildSource<LocalPropertyId> for MethodmapId {
    type Value = NodePtr;

    fn child_source(&self, db: &dyn DefDatabase) -> InFile<ArenaMap<LocalPropertyId, Self::Value>> {
        let loc = self.lookup(db).id;
        let mut map = ArenaMap::default();
        let tree = db.parse(loc.file_id());
        let mut items: Arena<MethodmapItemData> = Arena::new();
        let methodmap_node = loc.source(db, &tree).value;
        for child in methodmap_node
            .children(&mut methodmap_node.walk())
            .filter(|c| TSKind::from(c) == TSKind::methodmap_property)
        {
            let name_node = child.child_by_field_name("name").unwrap();
            let name = Name::from_node(&name_node, &db.preprocessed_text(loc.file_id()));
            let type_ref_node = child.child_by_field_name("type").unwrap();
            let type_ref =
                TypeRef::from_node(&type_ref_node, &db.preprocessed_text(loc.file_id())).unwrap();
            let getters_setters = Vec::new();
            let property = MethodmapItemData::Property(PropertyData {
                name,
                type_ref,
                getters_setters,
            });
            map.insert(items.alloc(property), NodePtr::from(&child));
        }
        InFile::new(loc.file_id(), map)
    }
}