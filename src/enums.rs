use super::lang;

pub struct Enum {
    pub name: String,
    pub id: lang::ID,
    pub symbol: char,
    pub variants: Vec<EnumVariant>
}

impl Enum {
    pub fn new() -> Self {
        Self {
            name: "New Enum".to_string(),
            id: lang::new_id(),
            symbol: '\u{f535}',
            variants: vec![],
        }
    }
}

pub struct EnumVariant {
    pub name: String,
    pub id: lang::ID,
    pub field_type: lang::Type,
}