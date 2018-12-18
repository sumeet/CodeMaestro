use super::lang;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Struct {
    pub name: String,
    pub id: lang::ID,
    pub symbol: String,
    pub fields: Vec<StructField>,
}

impl Struct {
    pub fn new() -> Self {
        Self {
            name: "New Struct".to_string(),
            id: lang::new_id(),
            // lol
            symbol: "\u{f535}".to_string(),
            fields: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructField {
    pub id: lang::ID,
    pub name: String,
    pub field_type: lang::Type,
}

impl StructField {
    pub fn new(name: String, field_type: lang::Type) -> Self {
        Self { id: lang::new_id(), name, field_type }
    }
}

impl lang::TypeSpec for Struct {
    fn readable_name(&self) -> &str {
        &self.name
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