use std::collections::HashMap;

use super::json2;
use crate::json2::{ParsedDocument, Scalar};

enum FieldIdentifier {
    Root,
    Name(String),
}

struct Schema {
    field_id: FieldIdentifier,
    typ: SchemaType,
    optional: bool,
}

#[derive(Clone)]
pub enum SchemaType {
    String { example: String },
    Number { example: i128 },
    Boolean { example: bool },
    Null,
    List { schema_type: Box<SchemaType> },
    Object { map: HashMap<String, SchemaType> },
    RemoveFromDocument,
}

impl SchemaType {
    pub fn from_parsed_doc(doc: &json2::ParsedDocument) -> Self {
        match doc {
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
                SchemaType::List { schema_type: Box::new(Self::from_parsed_doc(first)) }
            }
            ParsedDocument::Map { value, .. } => {
                SchemaType::Object { map: value.iter()
                                               .map(|(key, value)| {
                                                   (key.to_owned(), Self::from_parsed_doc(value))
                                               })
                                               .collect() }
            }
            ParsedDocument::EmptyCantInfer { .. } => SchemaType::RemoveFromDocument,
            ParsedDocument::NonHomogeneousCantParse { .. } => SchemaType::RemoveFromDocument,
        }
    }
}

// impl Schema {
//
//     fn from_parsed_doc(doc: &json2::ParsedDocument) -> Self {
//         let schema_type = ;
//         Self { typ: schema_type,
//                optional: false }
//     }
// }
