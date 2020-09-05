use std::collections::HashMap;

use super::json2;
use crate::json2::{ParsedDocument, Scalar};
use objekt::private::fmt::Formatter;
use std::fmt::Display;

pub const ALL_FIELD_TYPES: [&FieldType; 5] = [&FieldType::String,
                                              &FieldType::Number,
                                              &FieldType::Boolean,
                                              &FieldType::Null,
                                              &FieldType::Object];

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum FieldType {
    String,
    Number,
    Boolean,
    Null,
    Object,
}

// pub fn (ft: &FieldType) -> Self {
//     match ft {
//         FieldType::String => SchemaType::String { example: Default::default() },
//         FieldType::Number => SchemaType::Number { example: Default::default() },
//         FieldType::Boolean => SchemaType::Boolean { example: Default::default() },
//         FieldType::Null => SchemaType::Null,
//         FieldType::Object => SchemaType::Object { map: Default::default() },
//     }
// }

impl From<FieldType> for SchemaType {
    fn from(ft: FieldType) -> Self {
        match ft {
            FieldType::String => SchemaType::String { example: Default::default() },
            FieldType::Number => SchemaType::Number { example: Default::default() },
            FieldType::Boolean => SchemaType::Boolean { example: Default::default() },
            FieldType::Null => SchemaType::Null,
            FieldType::Object => SchemaType::Object { map: Default::default() },
        }
    }
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            FieldType::String => "String",
            FieldType::Number => "Number",
            FieldType::Boolean => "Boolean",
            FieldType::Null => "Null",
            FieldType::Object => "Object",
        };
        f.write_str(s)
    }
}

#[derive(Clone, Debug)]
pub enum FieldIdentifier {
    Root,
    Name(String),
}

#[derive(Clone)]
pub struct Schema {
    pub field_id: FieldIdentifier,
    pub typ: SchemaType,
    pub optional: bool,
    // indicator saying this came from the "pure" inference, not changed by user
    // pub inferred: bool,
}

#[derive(Clone)]
pub enum SchemaType {
    String { example: String },
    Number { example: i128 },
    Boolean { example: bool },
    Null,
    List { schema: Box<Schema> },
    Object { map: HashMap<String, Schema> },
    CameFromUnsupportedList,
}

pub type SchemaWithIndent<'a> = (&'a Schema, Indent);
pub type Indent = Vec<FieldIdentifier>;
pub type IndentRef<'a> = &'a [FieldIdentifier];

impl Schema {
    pub fn field_type(&self) -> FieldType {
        match self.typ {
            SchemaType::String { .. } => FieldType::String,
            SchemaType::Number { .. } => FieldType::Number,
            SchemaType::Boolean { .. } => FieldType::Boolean,
            SchemaType::Null => FieldType::Null,
            SchemaType::List { .. } => unimplemented!(),
            SchemaType::Object { .. } => FieldType::Object,
            SchemaType::CameFromUnsupportedList => unimplemented!(),
        }
    }
    pub fn get_mut(&mut self, indent: IndentRef) -> Result<&mut Self, Box<dyn std::error::Error>> {
        if indent.len() == 1 {
            return Ok(self);
        }

        let indent = &indent[1..];
        match &mut self.typ {
            SchemaType::String { .. }
            | SchemaType::Number { .. }
            | SchemaType::Boolean { .. }
            | SchemaType::Null
            | SchemaType::List { .. }
            | SchemaType::CameFromUnsupportedList => Err("bad indent".to_owned().into()),
            SchemaType::Object { map } => match &indent[0] {
                FieldIdentifier::Root => Err("bad indent".to_owned().into()),
                FieldIdentifier::Name(name) => {
                    let inner_schema: Result<_, Box<dyn std::error::Error>> =
                        map.get_mut(name.as_str()).ok_or("blah".to_string().into());
                    inner_schema?.get_mut(indent)
                }
            },
        }
    }

    pub fn iter_dfs_including_self(&self) -> impl Iterator<Item = SchemaWithIndent> {
        self.iter_dfs_including_self_rec(vec![])
    }

    pub fn iter_dfs_including_self_rec(&self,
                                       mut indent: Indent)
                                       -> impl Iterator<Item = SchemaWithIndent> {
        let indent2 = indent.clone();

        indent.push(self.field_id.clone());
        let schema_with_indent = (self, indent);
        let first: Box<dyn Iterator<Item = SchemaWithIndent>> =
            Box::new(std::iter::once(schema_with_indent));

        match &self.typ {
            SchemaType::String { .. }
            | SchemaType::Number { .. }
            | SchemaType::Boolean { .. }
            | SchemaType::Null
            | SchemaType::List { .. }
            | SchemaType::CameFromUnsupportedList => first,
            SchemaType::Object { map, .. } => {
                let rest = map.iter()
                              .map(move |(_, inner_schema)| {
                                  let mut indent = indent2.clone();
                                  indent.push(self.field_id.clone());
                                  inner_schema.iter_dfs_including_self_rec(indent)
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
                SchemaType::List { schema: Box::new(Self::from_parsed_doc(first, field_id.clone())) }
            }
            ParsedDocument::Map { value, .. } => {
                SchemaType::Object { map: value.iter()
                                               .map(|(key, value)| {
                                                   (key.to_owned(),
                                                    Self::from_parsed_doc(value, FieldIdentifier::Name(key.to_owned())))
                                               })
                                               .collect() }
            }
            ParsedDocument::EmptyCantInfer { .. } => SchemaType::CameFromUnsupportedList,
            ParsedDocument::NonHomogeneousCantParse { .. } => SchemaType::CameFromUnsupportedList,
        };
        Schema { field_id,
                 typ: schema_type,
                 optional: false }
    }
}
