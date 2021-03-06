use std::sync::Arc;

use gqls_syntax::{Node, NodeExt, NodeKind, Tree};

use crate::*;

pub(crate) struct BodyCtxt<'db> {
    db: &'db dyn DefDatabase,
    text: Arc<str>,
    file: FileId,
    diagnostics: Vec<Diagnostic>,
}

impl<'db> BodyCtxt<'db> {
    pub(crate) fn new(db: &'db dyn DefDatabase, file: FileId) -> Self {
        let text = db.file_text(file);
        Self { db, text, file, diagnostics: Default::default() }
    }

    pub fn lower_typedef(mut self, node: Node<'_>) -> ItemBody {
        let kind = match node.kind() {
            NodeKind::OBJECT_TYPE_DEFINITION | NodeKind::OBJECT_TYPE_EXTENSION =>
                ItemBodyKind::Object(self.lower_object_typedef(node)),
            NodeKind::INTERFACE_TYPE_DEFINITION =>
                ItemBodyKind::Interface(self.lower_interface_typedef(node)),
            NodeKind::INPUT_OBJECT_TYPE_DEFINITION =>
                ItemBodyKind::InputObject(self.lower_input_object_typedef(node)),
            NodeKind::UNION_TYPE_DEFINITION => ItemBodyKind::Union(self.lower_union_typedef(node)),
            NodeKind::ENUM_TYPE_DEFINITION => ItemBodyKind::Enum(self.lower_enum_typedef(node)),
            // TODO extensions etc
            _ => ItemBodyKind::Todo,
        };
        ItemBody { diagnostics: self.diagnostics, kind }
    }

    fn lower_object_typedef(&mut self, node: Node<'_>) -> ObjectTypeDefinitionBody {
        assert!(
            [NodeKind::OBJECT_TYPE_DEFINITION, NodeKind::OBJECT_TYPE_EXTENSION]
                .contains(&node.kind())
        );
        ObjectTypeDefinitionBody { fields: self.lower_fields_of(node) }
    }

    fn lower_input_object_typedef(&mut self, node: Node<'_>) -> InputTypeDefinitionBody {
        assert_eq!(node.kind(), NodeKind::INPUT_OBJECT_TYPE_DEFINITION);
        let fields = node
            .child_of_kind(NodeKind::INPUT_FIELDS_DEFINITION)
            .map(|fields| self.lower_input_fields(fields))
            .unwrap_or_default();
        InputTypeDefinitionBody { fields }
    }

    fn lower_interface_typedef(&mut self, node: Node<'_>) -> InterfaceDefinitionBody {
        assert_eq!(node.kind(), NodeKind::INTERFACE_TYPE_DEFINITION);
        InterfaceDefinitionBody { fields: self.lower_fields_of(node) }
    }

    fn lower_enum_typedef(&mut self, node: Node<'_>) -> EnumDefinitionBody {
        assert_eq!(node.kind(), NodeKind::ENUM_TYPE_DEFINITION);
        let variants = node
            .child_of_kind(NodeKind::ENUM_VALUES_DEFINITION)
            .map(|variants| self.lower_enum_variants(variants))
            .unwrap_or_default();
        EnumDefinitionBody { variants }
    }

    fn lower_enum_variants(&mut self, node: Node<'_>) -> Variants {
        assert_eq!(node.kind(), NodeKind::ENUM_VALUES_DEFINITION);
        node.children_of_kind(&mut node.walk(), NodeKind::ENUM_VALUE_DEFINITION)
            .filter_map(|variant| self.lower_enum_variant(variant))
            .collect()
    }

    fn lower_enum_variant(&mut self, node: Node<'_>) -> Option<Variant> {
        assert_eq!(node.kind(), NodeKind::ENUM_VALUE_DEFINITION);
        Some(Variant { name: self.name_of(node.child_of_kind(NodeKind::ENUM_VALUE)?)? })
    }

    fn lower_union_typedef(&mut self, node: Node<'_>) -> UnionDefinitionBody {
        assert_eq!(node.kind(), NodeKind::UNION_TYPE_DEFINITION);
        let types = node
            .child_of_kind(NodeKind::UNION_MEMBER_TYPES)
            .map(|node| self.lower_union_member_types(node))
            .unwrap_or_default();
        UnionDefinitionBody { types }
    }

    fn lower_union_member_types(&mut self, node: Node<'_>) -> Vec<Ty> {
        assert_eq!(node.kind(), NodeKind::UNION_MEMBER_TYPES);
        node.children_of_kind(&mut node.walk(), NodeKind::NAMED_TYPE)
            .map(|node| self.lower_named_type(node))
            .collect()
    }

    fn lower_fields_of(&mut self, node: Node<'_>) -> Fields {
        node.child_of_kind(NodeKind::FIELDS_DEFINITION)
            .map(|fields| self.lower_fields(fields))
            .unwrap_or_default()
    }

    fn lower_input_fields(&mut self, node: Node<'_>) -> Fields {
        assert_eq!(node.kind(), NodeKind::INPUT_FIELDS_DEFINITION);
        let cursor = &mut node.walk();
        let args = node
            .children_of_kind(cursor, NodeKind::INPUT_VALUE_DEFINITION)
            .filter_map(|field| self.lower_input_field(field));
        Fields::new(args)
    }

    fn lower_input_field(&mut self, node: Node<'_>) -> Option<Field> {
        assert_eq!(node.kind(), NodeKind::INPUT_VALUE_DEFINITION);
        let name = self.name_of(node)?;
        let ty = self.lower_type(node.child_of_kind(NodeKind::TYPE)?)?;
        let default_value = self.lower_default_value_of(node);
        let directives = self.lower_directives_of(node);
        Some(Field {
            range: node.range(),
            name,
            ty,
            directives,
            default_value,
            args: Default::default(),
        })
    }

    fn lower_arg(&mut self, node: Node<'_>) -> Option<Arg> {
        assert_eq!(node.kind(), NodeKind::INPUT_VALUE_DEFINITION);
        let name = self.name_of(node)?;
        let ty = self.lower_type(node.child_of_kind(NodeKind::TYPE)?)?;
        let default_value = self.lower_default_value_of(node);
        let directives = self.lower_directives_of(node);
        Some(Arg { range: node.range(), name, ty, default_value, directives })
    }

    fn lower_default_value_of(&mut self, node: Node<'_>) -> Option<Value> {
        node.child_of_kind(NodeKind::DEFAULT_VALUE)
            .and_then(NodeExt::sole_named_child)
            .and_then(|value| self.lower_value(value))
    }

    fn lower_value(&mut self, node: Node<'_>) -> Option<Value> {
        assert_eq!(node.kind(), NodeKind::VALUE);
        let value = node.sole_named_child()?;
        let t = self.text_of(value);
        let value = match value.kind() {
            NodeKind::STRING_VALUE => Value::String(Arc::from(t.trim_matches('"'))),
            NodeKind::INT_VALUE => Value::Int(t.parse().unwrap()),
            NodeKind::FLOAT_VALUE => Value::Float(t.parse().unwrap()),
            NodeKind::BOOLEAN_VALUE => match t {
                "true" => Value::Boolean(true),
                "false" => Value::Boolean(false),
                _ => unreachable!(),
            },
            NodeKind::NULL_VALUE => Value::Null,
            NodeKind::ENUM_VALUE => Value::Enum(Arc::from(t)),
            NodeKind::LIST_VALUE => Value::List(
                value
                    .children_of_kind(&mut value.walk(), NodeKind::VALUE)
                    .filter_map(|value| self.lower_value(value))
                    .collect(),
            ),
            NodeKind::OBJECT_VALUE => Value::Object(Arc::new(
                value
                    .children_of_kind(&mut value.walk(), NodeKind::OBJECT_FIELD)
                    .filter_map(|field| self.lower_object_field(field))
                    .collect(),
            )),
            _ => unreachable!(),
        };
        Some(value)
    }

    fn lower_object_field(&mut self, node: Node<'_>) -> Option<(Name, Value)> {
        assert_eq!(node.kind(), NodeKind::OBJECT_FIELD);
        Some((self.name_of(node)?, self.lower_value(node.child_of_kind(NodeKind::VALUE)?)?))
    }

    fn lower_fields(&mut self, node: Node<'_>) -> Fields {
        assert_eq!(node.kind(), NodeKind::FIELDS_DEFINITION);
        Fields::new(
            node.children_of_kind(&mut node.walk(), NodeKind::FIELD_DEFINITION)
                .filter_map(|field| self.lower_field(field)),
        )
    }

    fn lower_field(&mut self, node: Node<'_>) -> Option<Field> {
        assert_eq!(node.kind(), NodeKind::FIELD_DEFINITION);
        let ty = self.lower_type(node.child_of_kind(NodeKind::TYPE)?)?;
        let name = self.name_of(node)?;
        let directives = self.lower_directives_of(node);
        let args = self.lower_args_of(node);
        Some(Field { range: node.range(), name, ty, directives, args, default_value: None })
    }

    fn lower_args_of(&mut self, node: Node<'_>) -> Args {
        node.child_of_kind(NodeKind::ARGUMENTS_DEFINITION)
            .map(|args| self.lower_args(args))
            .unwrap_or_default()
    }

    fn lower_args(&mut self, node: Node<'_>) -> Args {
        assert_eq!(node.kind(), NodeKind::ARGUMENTS_DEFINITION);
        node.children_of_kind(&mut node.walk(), NodeKind::INPUT_VALUE_DEFINITION)
            .filter_map(|arg| self.lower_arg(arg))
            .collect()
    }

    pub(crate) fn lower_type(&mut self, node: Node<'_>) -> Option<Ty> {
        assert!(matches!(
            node.kind(),
            NodeKind::TYPE | NodeKind::NAMED_TYPE | NodeKind::LIST_TYPE | NodeKind::NON_NULL_TYPE
        ));
        let ty =
            if matches!(node.kind(), NodeKind::TYPE) { node.sole_named_child()? } else { node };
        let kind = match ty.kind() {
            NodeKind::NAMED_TYPE => return Some(self.lower_named_type(ty)),
            NodeKind::LIST_TYPE => TyKind::List(self.lower_type(ty.sole_named_child()?)?),
            NodeKind::NON_NULL_TYPE => {
                let inner = ty.sole_named_child()?;
                match inner.kind() {
                    NodeKind::NAMED_TYPE => TyKind::NonNull(self.lower_named_type(inner)),
                    NodeKind::LIST_TYPE => TyKind::NonNull(self.lower_list_type(inner)?),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        };
        Some(Arc::new(Type { range: ty.range(), kind }))
    }

    fn lower_list_type(&mut self, node: Node<'_>) -> Option<Ty> {
        assert_eq!(node.kind(), NodeKind::LIST_TYPE);
        let kind = TyKind::List(self.lower_type(node.sole_named_child()?)?);
        Some(Arc::new(Type { range: node.range(), kind }))
    }

    fn lower_named_type(&mut self, node: Node<'_>) -> Ty {
        assert_eq!(node.kind(), NodeKind::NAMED_TYPE);
        let name = Name::new(self, node);
        let range = name.range;
        let res = self.db.resolve_item(InProject::new(self.file, name.clone()));
        let kind = match res {
            Res::Err => {
                self.diagnostics
                    .push(Diagnostic::new(range, DiagnosticKind::UnresolvedType(name.clone())));
                TyKind::Err(name)
            }
            _ => TyKind::Named(name, res),
        };
        Arc::new(Type { range, kind })
    }
}

pub(crate) struct ItemCtxt {
    text: Arc<str>,
    typedefs: Arena<TypeDefinition>,
    directives: Arena<DirectiveDefinition>,
}

impl ItemCtxt {
    pub(crate) fn new(text: Arc<str>) -> Self {
        Self { text, typedefs: Default::default(), directives: Default::default() }
    }

    pub fn lower(mut self, tree: Tree) -> Arc<Items> {
        let node = tree.root_node();
        let items = node
            .relevant_children(&mut node.walk())
            .filter_map(|node| self.lower_item(node))
            .collect();

        Arc::new(Items { items, typedefs: self.typedefs, directives: self.directives })
    }

    fn lower_item(&mut self, node: Node<'_>) -> Option<Item> {
        assert_eq!(node.kind(), NodeKind::ITEM);
        let def = node.sole_named_child()?;
        let (name, kind) = match def.kind() {
            NodeKind::TYPE_DEFINITION => {
                let typedef = def.sole_named_child()?;
                let kind = match typedef.kind() {
                    NodeKind::OBJECT_TYPE_DEFINITION => TypeDefinitionKind::Object,
                    NodeKind::INTERFACE_TYPE_DEFINITION => TypeDefinitionKind::Interface,
                    NodeKind::SCALAR_TYPE_DEFINITION => TypeDefinitionKind::Scalar,
                    NodeKind::ENUM_TYPE_DEFINITION => TypeDefinitionKind::Enum,
                    NodeKind::UNION_TYPE_DEFINITION => TypeDefinitionKind::Union,
                    NodeKind::INPUT_OBJECT_TYPE_DEFINITION => TypeDefinitionKind::Input,
                    _ => {
                        unreachable!("invalid node kind for type definition: {:?}", typedef.kind())
                    }
                };
                let name_node = typedef.name_node()?;
                let name = Name::new(self, name_node);
                let directives = self.lower_directives_of(typedef);
                let implementations = self.try_lower_implementations_of(typedef);
                (
                    name,
                    ItemKind::TypeDefinition(self.typedefs.alloc(TypeDefinition {
                        is_ext: false,
                        kind,
                        directives,
                        implementations,
                    })),
                )
            }
            NodeKind::TYPE_EXTENSION => {
                let type_ext = def.sole_named_child()?;
                let kind = match type_ext.kind() {
                    NodeKind::OBJECT_TYPE_EXTENSION => TypeDefinitionKind::Object,
                    // TODO
                    _ => return None,
                };
                let name_node = type_ext.name_node()?;
                let name = Name::new(self, name_node);
                let directives = self.lower_directives_of(type_ext);
                let implementations = self.try_lower_implementations_of(type_ext);
                (
                    name,
                    ItemKind::TypeDefinition(self.typedefs.alloc(TypeDefinition {
                        is_ext: true,
                        kind,
                        directives,
                        implementations,
                    })),
                )
            }
            NodeKind::DIRECTIVE_DEFINITION => {
                let name = Name::new(self, def.name_node()?);
                let locations_node = def.child_of_kind(NodeKind::DIRECTIVE_LOCATIONS)?;
                let locations = locations_node
                    .children_of_kind(&mut locations_node.walk(), NodeKind::DIRECTIVE_LOCATION)
                    .filter_map(|location| {
                        Some(match location.child_by_field_name("location")?.kind() {
                            "ARGUMENT_DEFINITION" => DirectiveLocations::ARGUMENT_DEFINITION,
                            "ENUM" => DirectiveLocations::ENUM,
                            "ENUM_VALUE" => DirectiveLocations::ENUM_VALUE,
                            "FIELD_DEFINITION" => DirectiveLocations::FIELD_DEFINITION,
                            "INPUT_FIELD_DEFINITION" => DirectiveLocations::INPUT_FIELD_DEFINITION,
                            "INPUT_OBJECT" => DirectiveLocations::INPUT_OBJECT,
                            "INTERFACE" => DirectiveLocations::INTERFACE,
                            "OBJECT" => DirectiveLocations::OBJECT,
                            "SCALAR" => DirectiveLocations::SCALAR,
                            "SCHEMA" => DirectiveLocations::SCHEMA,
                            "UNION" => DirectiveLocations::UNION,
                            location => {
                                unreachable!("found invalid directive location: `{location}`",)
                            }
                        })
                    })
                    .fold(DirectiveLocations::default(), |acc, location| acc | location);
                (
                    name,
                    ItemKind::DirectiveDefinition(
                        self.directives.alloc(DirectiveDefinition { locations }),
                    ),
                )
            }
            // TODO
            _ => return None,
        };
        Some(Item { range: def.range(), name, kind })
    }

    fn try_lower_implementations_of(&mut self, node: Node<'_>) -> Option<Implementations> {
        let implementations = node.child_of_kind(NodeKind::IMPLEMENTS_INTERFACES)?;
        let cursor = &mut implementations.walk();
        Some(
            implementations
                .children_of_kind(cursor, NodeKind::NAMED_TYPE)
                .map(|node| Name::new(self, node))
                .collect(),
        )
    }
}

pub(crate) trait LowerCtxt: HasText {
    fn name_of(&mut self, node: Node<'_>) -> Option<Name> {
        node.name_node().map(|node| Name::new(self, node))
    }

    fn lower_directives_of(&mut self, node: Node<'_>) -> Directives {
        node.child_of_kind(NodeKind::DIRECTIVES)
            .map(|node| self.lower_directives(node))
            .unwrap_or_default()
    }

    fn lower_directives(&mut self, node: Node<'_>) -> Directives {
        assert_eq!(node.kind(), NodeKind::DIRECTIVES);
        node.children_of_kind(&mut node.walk(), NodeKind::DIRECTIVE)
            .filter_map(|node| self.lower_directive(node))
            .collect()
    }

    fn lower_directive(&mut self, node: Node<'_>) -> Option<Directive> {
        assert_eq!(node.kind(), NodeKind::DIRECTIVE);
        // TODO arguments
        let name = Name::new(self, node.name_node()?);
        Some(Directive { range: node.range(), name })
    }
}

impl<C: HasText> LowerCtxt for C {
}

impl HasText for ItemCtxt {
    fn text(&self) -> &str {
        &self.text
    }
}

impl HasText for BodyCtxt<'_> {
    fn text(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests;
