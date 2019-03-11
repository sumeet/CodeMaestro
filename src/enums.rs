use serde_derive::{Serialize,Deserialize};

use super::lang::TypeSpec;
use super::lang;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Enum {
    pub name: String,
    pub id: lang::ID,
    pub symbol: String,
    pub variants: Vec<EnumVariant>
}

impl Enum {
    pub fn new() -> Self {
        Self {
            name: "New Enum".to_string(),
            id: lang::new_id(),
            symbol: "\u{f103}".to_string(),
            variants: vec![],
        }
    }

    pub fn variant_types<'a>(&'a self, params: &'a [lang::Type]) -> Vec<(&'a EnumVariant, &'a lang::Type)> {
        if params.len() != self.num_params() {
            panic!("# of variant types doesn't match")
        }
        let mut params = params.iter();
        self.variants.iter().map(|variant| {
            let typ = variant.variant_type.as_ref()
                .unwrap_or_else(|| params.next().unwrap());
            (variant, typ)
        }).collect()
    }
}

#[typetag::serde]
impl lang::TypeSpec for Enum {
    fn readable_name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn num_params(&self) -> usize {
        self.variants.iter().filter(|v| v.is_parameterized()).count()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub id: lang::ID,
    // if it's not a type, then it's parameterized
    pub variant_type: Option<lang::Type>,
}

impl EnumVariant {
    pub fn new(name: String, variant_type: Option<lang::Type>) -> Self {
        Self { id: lang::new_id(), name, variant_type }
    }

    pub fn is_parameterized(&self) -> bool {
        self.variant_type.is_none()
    }
}
