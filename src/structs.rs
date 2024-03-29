use serde_derive::{Deserialize, Serialize};

use std::collections::HashMap;

use super::lang;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Struct {
    pub name: String,
    pub description: String,
    pub id: lang::ID,
    pub symbol: String,
    pub fields: Vec<StructField>,
}

impl Struct {
    pub fn new() -> Self {
        Self { name: "New Struct".to_string(),
               description: "".to_string(),
               id: lang::new_id(),
               // lol
               symbol: "\u{f1b2}".to_string(),
               fields: vec![] }
    }

    // TODO: don't compute this every time... replace the fields
    // vector with this
    pub fn field_by_id(&self) -> HashMap<lang::ID, &StructField> {
        self.fields.iter().map(|field| (field.id, field)).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructField {
    pub id: lang::ID,
    pub name: String,
    pub description: String,
    pub field_type: lang::Type,
}

impl StructField {
    pub fn new(name: String, description: String, field_type: lang::Type) -> Self {
        Self { id: lang::new_id(),
               description,
               name,
               field_type }
    }
}

#[typetag::serde]
impl lang::TypeSpec for Struct {
    fn readable_name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }

    // we may have parameterized structs at some point...
    fn num_params(&self) -> usize {
        0
    }
}
