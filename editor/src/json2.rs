use std::collections::BTreeMap;
use std::iter;

use itertools::Itertools;
use matches::matches;
use serde_json;

#[derive(Clone, Debug, PartialEq)]
pub enum Nest {
    ListElement(usize),
    MapKey(String),
}

pub type Nesting = Vec<Nest>;

#[derive(Clone)]
pub enum Scalar {
    // Scalars
    Null { nesting: Nesting },
    Bool { value: bool, nesting: Nesting },
    String { value: String, nesting: Nesting },
    Number { value: i128, nesting: Nesting },
}

impl Scalar {
    #[allow(unused)]
    pub(crate) fn nesting(&self) -> &Nesting {
        use Scalar::*;
        match self {
            Null { nesting, .. }
            | Bool { nesting, .. }
            | String { nesting, .. }
            | Number { nesting, .. } => nesting,
        }
    }
}

#[derive(Clone)]
pub enum ParsedDocument {
    Scalar(Scalar),
    // Composites
    List {
        value: Vec<ParsedDocument>,
        nesting: Nesting,
    },
    // BTreeMaps have stable key order, which is a must when rendering otherwise the screen flickers
    // like crazy lol
    Map {
        value: BTreeMap<String, ParsedDocument>,
        nesting: Nesting,
    },
    // these are /errors/ (when parsing Lists) but for simplicity we'll put these in the
    // actual structure enum instead of putting them in a separate one
    EmptyCantInfer {
        value: serde_json::Value,
        nesting: Nesting,
    },
    NonHomogeneousCantParse {
        value: serde_json::Value,
        nesting: Nesting,
    },
}

impl ParsedDocument {
    // TODO: is there a way i can make this lazy?
    fn doc_type(&self) -> DocType {
        match self {
            ParsedDocument::Scalar(Scalar::Null { .. }) => DocType::Null,
            ParsedDocument::Scalar(Scalar::Bool { .. }) => DocType::Bool,
            ParsedDocument::Scalar(Scalar::String { .. }) => DocType::String,
            ParsedDocument::Scalar(Scalar::Number { .. }) => DocType::Number,
            ParsedDocument::List { value, .. } => {
                let mut list_type = value.iter().map(|d| d.doc_type()).unique().collect_vec();
                if list_type.len() != 1 {
                    panic!("there is a bug here, parsed doc lists should only have a single type")
                }
                DocType::List(Box::new(list_type.pop().unwrap()))
            }
            ParsedDocument::Map { value, .. } => {
                let doc_type_by_key = value.iter()
                                           .map(|(key, doc)| (key.clone(), doc.doc_type()))
                                           .collect();
                DocType::Map(doc_type_by_key)
            }
            ParsedDocument::EmptyCantInfer { .. } => DocType::EmptyCantInfer,
            ParsedDocument::NonHomogeneousCantParse { .. } => DocType::NonHomogeneousCantParse,
        }
    }

    pub fn nesting(&self) -> &Nesting {
        match self {
            ParsedDocument::Scalar(Scalar::Null { nesting, .. })
            | ParsedDocument::NonHomogeneousCantParse { nesting, .. }
            | ParsedDocument::EmptyCantInfer { nesting, .. }
            | ParsedDocument::Scalar(Scalar::Bool { nesting, .. })
            | ParsedDocument::Map { nesting, .. }
            | ParsedDocument::List { nesting, .. }
            | ParsedDocument::Scalar(Scalar::String { nesting, .. })
            | ParsedDocument::Scalar(Scalar::Number { nesting, .. }) => nesting,
        }
    }

    #[allow(unused)]
    pub fn find(&self, nesting: &Nesting) -> Option<&Self> {
        if self.nesting() == nesting {
            return Some(self);
        }
        self.all_children_dfs()
            .flat_map(|child| child.find(nesting))
            .next()
    }

    #[allow(unused)]
    pub fn all_children_including_self(&self) -> impl Iterator<Item = &Self> + '_ {
        return std::iter::once(self).chain(self.all_children_dfs());
    }

    #[allow(unused)]
    fn all_children_dfs<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self> + 'a> {
        Box::new(self.iterate_children()
                     .flat_map(|child| iter::once(child).chain(child.all_children_dfs())))
    }

    fn iterate_children<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self> + 'a> {
        match self {
            ParsedDocument::List { value, .. } => Box::new(value.iter()),
            ParsedDocument::Map { value, .. } => Box::new(value.values()),
            // scalars don't have children
            ParsedDocument::NonHomogeneousCantParse { .. }
            | ParsedDocument::EmptyCantInfer { .. }
            | ParsedDocument::Scalar(_) => Box::new(iter::empty()),
        }
    }
}

pub fn parse(j: serde_json::Value) -> ParsedDocument {
    parse_nesting(j, vec![])
}

fn parse_nesting(j: serde_json::Value, nesting: Nesting) -> ParsedDocument {
    match j {
        serde_json::Value::Null => ParsedDocument::Scalar(Scalar::Null { nesting }),
        serde_json::Value::Bool(value) => ParsedDocument::Scalar(Scalar::Bool { value, nesting }),
        serde_json::Value::String(value) => {
            ParsedDocument::Scalar(Scalar::String { value, nesting })
        }
        serde_json::Value::Number(number) => {
            if number.is_f64() {
                // yep we turn floats into strings
                ParsedDocument::Scalar(Scalar::String { value: number.to_string(),
                                                        nesting })
            } else {
                ParsedDocument::Scalar(Scalar::Number { value: number.as_i64().unwrap() as i128,
                                                        nesting })
            }
        }
        serde_json::Value::Array(ref vs) => {
            let parsed_docs = vs.into_iter()
                                .cloned()
                                .enumerate()
                                .map(|(index, value)| {
                                    let mut nesting = nesting.clone();
                                    nesting.push(Nest::ListElement(index));
                                    parse_nesting(value, nesting)
                                })
                                .collect_vec();
            let doc_types = parsed_docs.iter()
                                       .map(|d| d.doc_type())
                                       .filter(|t| !matches!(t, DocType::EmptyCantInfer))
                                       .unique()
                                       .collect_vec();
            if doc_types.is_empty() {
                ParsedDocument::EmptyCantInfer { value: j, nesting }
            } else if doc_types.len() > 1 {
                ParsedDocument::NonHomogeneousCantParse { value: j, nesting }
            } else {
                ParsedDocument::List { value: parsed_docs,
                                       nesting }
            }
        }
        serde_json::Value::Object(o) => {
            ParsedDocument::Map { value: o.into_iter()
                                          .map(|(k, v)| {
                                              let mut nesting = nesting.clone();
                                              nesting.push(Nest::MapKey(k.clone()));
                                              (k, parse_nesting(v, nesting))
                                          })
                                          .collect(),
                                  nesting }
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
    Map(BTreeMap<String, DocType>),
    // these are /errors/ (when parsing Lists) but for simplicity we'll put these in the
    // actual structure enum instead of putting them in a separate one
    EmptyCantInfer,
    NonHomogeneousCantParse,
}
