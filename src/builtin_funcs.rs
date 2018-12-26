use std::collections::HashMap;
use super::lang;
use super::env;

#[derive(Clone)]
pub struct Print {}

impl lang::Function for Print {
    fn call(&self, env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        match args.get(&self.takes_args()[0].id) {
            Some(lang::Value::String(ref string)) =>  {
                env.println(string);
                lang::Value::Null
            }
            _ => lang::Value::Error(lang::Error::ArgumentError)
        }
    }

    fn name(&self) -> &str {
        "Print"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("b5c18d63-f9a0-4f08-8ee7-e35b3db9122d").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("feff08f0-7319-4b47-964e-1f470eca81df").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "String to print".to_string()
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }
}

#[derive(Clone)]
pub struct Capitalize {}

impl lang::Function for Capitalize {
    fn call(&self, _env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        match args.get(&self.takes_args()[0].id) {
            Some(lang::Value::String(ref string)) =>  {
                lang::Value::String(string.to_uppercase())
            }
            _ => lang::Value::Error(lang::Error::ArgumentError)
        }
    }

    fn name(&self) -> &str {
        "Capitalize"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("86ae2a51-5538-436f-b48e-3aa6c873b189").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("94e81ddc-843b-426d-847e-a215125c9593").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "String to capitalize".to_string(),
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::STRING_TYPESPEC)
    }
}

