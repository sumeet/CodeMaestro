use std::collections::BTreeMap;

use matches::matches;
use serde_json;
use itertools::Itertools;

#[derive(PartialEq, Clone, Eq, Hash)]
enum Structure {
    // scalars
    Bool,
    String,
    Number,
    Float,
    Null,
    List { size: usize, of: Box<Structure> },
    // BTreeMaps are sorted and therefore comparable while HashMaps aren't
    Map(BTreeMap<String,Structure>),

    // these are /errors/ (when parsing Lists) but for simplicity we'll put these in the
    // actual structure enum instead of putting them in a separate one
    EmptyCantInfer,
    NonHomogeneousCantParse,
}

fn guess_structure(j: &serde_json::Value) -> Structure {
    match j {
        serde_json::Value::Null => Structure::Null,
        serde_json::Value::Bool(_) => Structure::Bool,
        serde_json::Value::Number(_) => Structure::Number,
        serde_json::Value::String(_) => Structure::String,
        serde_json::Value::Array(vs) =>  {
            let mut types = vs.iter()
                .map(guess_structure)
                .filter(|v| !matches!(v, Structure::EmptyCantInfer))
                .unique()
                .collect_vec();
            if types.is_empty() {
                Structure::EmptyCantInfer
            } else if types.len() > 1 {
                Structure::NonHomogeneousCantParse
            } else {
                Structure::List { size: vs.len(), of: Box::new(types.pop().unwrap()) }
            }
        }
        serde_json::Value::Object(o) => {
            Structure::Map(o.iter().map(|(k, v)| (k.clone(), guess_structure(v))).collect())
        },
    }
}
