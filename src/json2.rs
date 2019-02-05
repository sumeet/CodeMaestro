use super::lang;

use std::collections::HashMap;
use std::collections::BTreeMap;

use matches::matches;
use serde_json;
use itertools::Itertools;

pub enum ParsedDocument {
    // Scalars
    Null,
    Bool(bool),
    String(String),
    Number(i128),
    // Composites
    List(Vec<ParsedDocument>),
    Map(HashMap<String,ParsedDocument>),
    // these are /errors/ (when parsing Lists) but for simplicity we'll put these in the
    // actual structure enum instead of putting them in a separate one
    EmptyCantInfer(serde_json::Value),
    NonHomogeneousCantParse(serde_json::Value),
}

impl ParsedDocument {
    fn doc_type(&self) -> DocType {
        match self {
            ParsedDocument::Null => DocType::Null,
            ParsedDocument::Bool(_) => DocType::Bool,
            ParsedDocument::String(_) => DocType::String,
            ParsedDocument::Number(_) => DocType::Number,
            ParsedDocument::List(list) => {
                let mut list_type = list.iter()
                    .map(|d| d.doc_type())
                    .unique()
                    .collect_vec();
                if list_type.len() != 1 {
                    panic!("there is a bug here, parsed doc lists should only have a single type")
                }
                DocType::List(Box::new(list_type.pop().unwrap()))
            }
            ParsedDocument::Map(map) => {
                let doc_type_by_key = map.iter()
                    .map(|(key, doc)| (key.clone(), doc.doc_type()))
                    .collect();
                DocType::Map(doc_type_by_key)
            },
            ParsedDocument::EmptyCantInfer(_) => DocType::EmptyCantInfer,
            ParsedDocument::NonHomogeneousCantParse(_) => DocType::NonHomogeneousCantParse,
        }
    }
}

pub fn parse(j: serde_json::Value) -> ParsedDocument {
    match j {
        serde_json::Value::Null => ParsedDocument::Null,
        serde_json::Value::Bool(b) => ParsedDocument::Bool(b),
        serde_json::Value::String(s) => ParsedDocument::String(s),
        serde_json::Value::Number(number) => {
            if number.is_f64() {
                // yep we turn floats into strings
                ParsedDocument::String(number.to_string())
            } else {
                ParsedDocument::Number(number.as_i64().unwrap() as i128)
            }
        },
        serde_json::Value::Array(ref vs) => {
            let parsed_docs = vs.into_iter().cloned().map(parse).collect_vec();
            let doc_types = parsed_docs.iter().map(|d| d.doc_type())
                .filter(|t| !matches!(t, DocType::EmptyCantInfer))
                .unique()
                .collect_vec();
            if doc_types.is_empty() {
                ParsedDocument::EmptyCantInfer(j)
            } else if doc_types.len() > 1 {
                ParsedDocument::NonHomogeneousCantParse(j)
            } else {
                ParsedDocument::List(parsed_docs)
            }
        },
        serde_json::Value::Object(o) => {
            ParsedDocument::Map(o.into_iter().map(|(k, v)| (k, parse(v))).collect())
        }
    }
}

#[derive(PartialEq, Clone, Eq, Hash)]
enum DocType {
    Null,
    Bool,
    String,
    Number,
    // Composites
    List(Box<DocType>),
    // BTreeMaps are sorted and therefore comparable while HashMaps aren't
    Map(BTreeMap<String,DocType>),
    // these are /errors/ (when parsing Lists) but for simplicity we'll put these in the
    // actual structure enum instead of putting them in a separate one
    EmptyCantInfer,
    NonHomogeneousCantParse,
}

