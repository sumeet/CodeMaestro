use std::collections::BTreeMap;

use matches::matches;
use serde_json;
use itertools::Itertools;

#[derive(PartialEq, Clone, Eq, Hash)]
pub enum Structure {
    // scalars
    Bool,
    String,
    Number,
    Float,
    Null,
    //
    // TODO: the metadata here causes Eq and PartialEq to be false when they shouldn't. rework the
    // metadata later (i believe we can use the original JSON Value to capture this)
//    List { size: usize, of: Box<Structure> },
    List { of: Box<Structure> },
    // BTreeMaps are sorted and therefore comparable while HashMaps aren't
    Map(BTreeMap<String,Structure>),

    // these are /errors/ (when parsing Lists) but for simplicity we'll put these in the
    // actual structure enum instead of putting them in a separate one
    EmptyCantInfer,
    NonHomogeneousCantParse,
}

impl Structure {
    pub fn typ(&self) -> &str {
        match *self {
            Structure::Bool => "Bool",
            Structure::String => "String",
            Structure::Number => "Number",
            Structure::Float => "Float",
            Structure::Null => "Null",
            Structure::List {..} => "List",
            Structure::Map(_) => "Map",
            Structure::EmptyCantInfer => "EmptyCantInfer",
            Structure::NonHomogeneousCantParse => "NonHomogeneousCantParse",
        }
    }
}

pub fn infer_structure(j: &serde_json::Value) -> Structure {
    match j {
        serde_json::Value::Null => Structure::Null,
        serde_json::Value::Bool(_) => Structure::Bool,
        serde_json::Value::Number(_) => Structure::Number,
        serde_json::Value::String(_) => Structure::String,
        serde_json::Value::Array(vs) =>  {
            let mut types = vs.iter()
                .map(infer_structure)
                .filter(|v| !matches!(v, Structure::EmptyCantInfer))
                .unique()
                .collect_vec();
            if types.is_empty() {
                Structure::EmptyCantInfer
            } else if types.len() > 1 {
                Structure::NonHomogeneousCantParse
            } else {
//                Structure::List { size: vs.len(), of: Box::new(types.pop().unwrap()) }
                Structure::List { of: Box::new(types.pop().unwrap()) }
            }
        }
        serde_json::Value::Object(o) => {
            Structure::Map(o.iter().map(|(k, v)| (k.clone(), infer_structure(v))).collect())
        },
    }
}
