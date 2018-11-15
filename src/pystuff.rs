use std::collections::HashMap;

use super::pyo3::prelude::*;
use super::lang;
use super::env;
use std::rc::Rc;

pub struct Py {
    gil: GILGuard,
}

impl Py {
    fn new() -> Self {
        Self { gil: Python::acquire_gil() }
    }

    fn py<'a>(&'a self) -> Python<'a> {
        self.gil.python()
    }
}

thread_local! {
    pub static PY: Py = Py::new();
}

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
        let gil = Python::acquire_gil();
        Self {
            prelude: "".to_string(),
            eval: "".to_string(),
            return_type: lang::Type::from_spec(&lang::NULL_TYPESPEC),
            name: "New PyFunc".to_string(),
            id: lang::new_id(),
        }
    }
}

impl PyFunc {
    fn extract(&self, pyobjectref: &PyObjectRef) -> Option<lang::Value> {
        use self::lang::Function;
        if self.returns().matches_spec(&lang::STRING_TYPESPEC) {
            if let(Ok(string)) = pyobjectref.extract() {
                return Some(lang::Value::String(string))
            }
        } else if self.returns().matches_spec(&lang::NUMBER_TYPESPEC) {
            if let(Ok(int)) = pyobjectref.extract() {
                return Some(lang::Value::Number(int))
            }
        } else if self.returns().matches_spec(&lang::NULL_TYPESPEC) {
            if pyobjectref.is_none() {
                return Some(lang::Value::Null)
            }
        }
        None
    }

    fn py_exception_to_error(&self, pyerror: &PyErr) -> lang::Error {
        let error_str = PY.with(|py| {
            let error_obj = pyerror.into_object(py.py());
            error_obj.getattr(py.py(), "__str__").unwrap().call0(py.py()).unwrap()
                .extract(py.py()).unwrap()
        });
        lang::Error::PythonError(error_str)
    }
}

impl lang::Function for PyFunc {
    fn call(&self, env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        PY.with(|py| {
            let result = py.py().run(&self.prelude, None, None);

            if let(Err(e)) = result {
                lang::Value::Result(Err(lang::Error::PythonError("error rnning the prelude".to_string())))
            } else {
                let eval_result = py.py().eval(self.eval.as_ref(), None, None);
                if let(Err(pyerr)) = eval_result {
                    return lang::Value::Result(Err(self.py_exception_to_error(&pyerr)))
                }
                let eval_result = eval_result.unwrap();

                if let(Some(value)) = self.extract(eval_result) {
                    return value
                }

                lang::Value::Result(Err(lang::Error::PythonError("couldn't deserialize type from python".to_string())))
            }
        })
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

    fn returns(&self) -> lang::Type {
        self.return_type.clone()
    }
}
