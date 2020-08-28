use std::collections::HashMap;

use super::json2;
use crate::json2::{ParsedDocument, Scalar};

#[derive(Clone)]
pub enum FieldIdentifier {
    Root,
    Name(String),
}

#[derive(Clone)]
pub struct Schema {
    pub field_id: FieldIdentifier,
    pub typ: SchemaType,
    pub optional: bool,
}

#[derive(Clone)]
pub enum SchemaType {
    String { example: String },
    Number { example: i128 },
    Boolean { example: bool },
    Null,
    List { schema_type: Box<Schema> },
    Object { map: HashMap<String, Schema> },
    RemoveFromDocument,
}

pub type SchemaWithIndent<'a> = (&'a Schema, usize);

impl Schema {
    pub fn iter_dfs_including_self(&self) -> impl Iterator<Item = SchemaWithIndent> {
        self.iter_dfs_including_self_rec(0)
    }

    pub fn iter_dfs_including_self_rec(&self,
                                       indent: usize)
                                       -> impl Iterator<Item = SchemaWithIndent> {
        let idk = (self, indent);
        let first: Box<dyn Iterator<Item = SchemaWithIndent>> = Box::new(std::iter::once(idk));

        match &self.typ {
            SchemaType::String { .. }
            | SchemaType::Number { .. }
            | SchemaType::Boolean { .. }
            | SchemaType::Null
            | SchemaType::List { .. }
            | SchemaType::RemoveFromDocument => first,
            SchemaType::Object { map, .. } => {
                let rest = map.iter()
                              .map(move |(_, inner_schema)| {
                                  inner_schema.iter_dfs_including_self_rec(indent + 1)
                              })
                              .flatten();
                Box::new(first.chain(rest))
            }
        }
    }

    pub fn from_parsed_doc_root(doc: &json2::ParsedDocument) -> Schema {
        Self::from_parsed_doc(doc, FieldIdentifier::Root)
    }

    pub fn from_parsed_doc(doc: &json2::ParsedDocument, field_id: FieldIdentifier) -> Schema {
        let schema_type = match doc {
            ParsedDocument::Scalar(scalar) => match scalar {
                Scalar::Null { .. } => SchemaType::Null,
                Scalar::Bool { value, .. } => SchemaType::Boolean { example: *value },
                Scalar::String { value, .. } => SchemaType::String { example: value.to_owned() },
                Scalar::Number { value, .. } => SchemaType::Number { example: *value },
            },
            ParsedDocument::List { value, .. } => {
                if value.is_empty() {
                    panic!("this should never be empty");
                }
                let first = value.first().unwrap();
                SchemaType::List { schema_type: Box::new(Self::from_parsed_doc(first, field_id.clone())) }
            }
            ParsedDocument::Map { value, .. } => {
                SchemaType::Object { map: value.iter()
                                               .map(|(key, value)| {
                                                   (key.to_owned(),
                                                    Self::from_parsed_doc(value, FieldIdentifier::Name(key.to_owned())))
                                               })
                                               .collect() }
            }
            ParsedDocument::EmptyCantInfer { .. } => SchemaType::RemoveFromDocument,
            ParsedDocument::NonHomogeneousCantParse { .. } => SchemaType::RemoveFromDocument,
        };
        Schema { field_id,
                 typ: schema_type,
                 optional: false }
    }
}
