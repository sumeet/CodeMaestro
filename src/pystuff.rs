use std::collections::HashMap;

use super::pyo3::prelude::*;
use super::lang;
use super::env;

static PRELUDE : &str = "import random";
static PYSTR: &str = "str(random.choice([1, 2, 3, 4, 5]))";

#[derive(Clone)]
pub struct PyChoice {}

impl lang::Function for PyChoice {
    fn call(&self, env: &mut env::ExecutionEnvironment,
            args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let gil = Python::acquire_gil();
        let py = gil.python();

        py.run(PRELUDE, None, None);
        let choice = py.eval(PYSTR, None, None);

        if let(Err(e)) = choice {
            println!("{:#?}", e);
            e.print(py);
            return lang::Value::String("error1".to_string())
        }

        let choice = choice.unwrap().extract();

        match choice {
            Ok(result) => {
                lang::Value::String(result)
            }
            Err(e) => {
                println!("{:#?}", e);
                e.print(py);
                lang::Value::String("error2".to_string())
            }
        }
    }

    fn name(&self) -> &str {
        "PyChoice"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("a3fc25c7-171a-43a2-97ce-1b25c593e67f").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![]
    }

    fn returns(&self) -> &lang::Type {
        &lang::STRING_TYPE
    }
}
