use std::collections::HashMap;

use super::pyo3::prelude::*;
use super::lang;
use super::env;

static PRELUDE : &str = "import random";
static PYSTR: &str = "str(random.choice([1, 2, 3, 4, 5]))";

#[derive(Clone)]
pub struct PyFunc {
    pub prelude: String,
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub id: lang::ID,
}

impl PyFunc {
    pub fn new() -> Self {
        Self {
            prelude: "".to_string(),
            eval: "".to_string(),
            return_type: lang::NULL_TYPE.clone(),
            name: "New PyFunc".to_string(),
            id: lang::new_id(),
        }
    }
}

impl lang::Function for PyFunc {
    fn call(&self, env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let gil = Python::acquire_gil();
        let gil2 = Python::acquire_gil();
        let py = gil.python();
        let py2 = gil.python();
        let result = py.run(&self.prelude, None, None);
        if let(Err(e)) = result {
            lang::Value::Result(Err(lang::Error::PythonError))
        } else {
            let eval_result = py.eval(PYSTR, None, None);
            let result = eval_result.unwrap();
            if let(Ok(string)) = result.extract() {
                return lang::Value::String(string)
            }
            lang::Value::Result(Err(lang::Error::PythonError))
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![]
    }

    fn returns(&self) -> &lang::Type {
        &self.return_type
    }
}

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
