use std::collections::HashMap;

use super::pyo3::prelude::*;
use super::pyo3::types::{PyIterator,PyObjectRef};
use super::lang;
use super::env;
use super::external_func;

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
    fn extract(&self, pyobjectref: &PyObjectRef) -> lang::Value {
        use self::lang::Function;
        self.ex(pyobjectref, &self.returns())
    }

    fn ex(&self, pyobjectref: &PyObjectRef, into_type: &lang::Type) -> lang::Value {
        if into_type.matches_spec(&lang::STRING_TYPESPEC) {
            if let Ok(string) = pyobjectref.extract() {
                return lang::Value::String(string)
            }
        } else if into_type.matches_spec(&lang::NUMBER_TYPESPEC) {
            if let Ok(int) = pyobjectref.extract() {
                return lang::Value::Number(int)
            }
        } else if into_type.matches_spec(&lang::NULL_TYPESPEC) {
            if pyobjectref.is_none() {
                return lang::Value::Null
            }
        } else if into_type.matches_spec(&lang::LIST_TYPESPEC) {
            let pyobj : PyObject = pyobjectref.extract().unwrap();
            let collection_type = into_type.params.first().unwrap();
            return PY.with(|py| {
                // TODO: error handlign! just figure out what's neccessary by testing it out in the
                // GUI
                let iter = PyIterator::from_object(py.py(), &pyobj).unwrap();
                let collected : Vec<lang::Value> = iter
                    .map(|pyresult| {
                        self.ex(pyresult.unwrap(), collection_type)
                    })
                    .collect();
                lang::Value::List(collected)
            });
        }
        lang::Value::Error(lang::Error::PythonDeserializationError)
    }

    fn py_exception_to_error(&self, pyerror: &PyErr) -> lang::Error {
        PY.with(|py| {
            let error_obj = pyerror.into_object(py.py());
            let error_cls = error_obj.getattr(py.py(), "__class__")
                .unwrap();
            let error_cls_name = error_cls.getattr(py.py(), "__name__")
                .unwrap();
            lang::Error::PythonError(self.str(error_cls_name), self.str(error_obj))
        })
    }

    fn str(&self, pyobj: PyObject) -> String {
        PY.with(|py| {
            pyobj.getattr(py.py(), "__str__").unwrap().call0(py.py()).unwrap()
                .extract(py.py()).unwrap()
        })
    }
}

impl lang::Function for PyFunc {
    fn call(&self, _env: &mut env::ExecutionEnvironment, _args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        PY.with(|py| {
            let result = py.py().run(&self.prelude, None, None);

            if let Err(e) = result {
                lang::Value::Error(self.py_exception_to_error(&e))
            } else {
                let eval_result = py.py().eval(self.eval.as_ref(), None, None);
                if let Err(pyerr) = eval_result {
                    return lang::Value::Error(self.py_exception_to_error(&pyerr))
                }
                let eval_result = eval_result.unwrap();
                self.extract(eval_result)
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

impl external_func::ModifyableFunc for PyFunc {
    fn set_return_type(&mut self, return_type: lang::Type) {
        self.return_type = return_type
    }

    fn clone(&self) -> Self {
        PyFunc {
            prelude: self.prelude.clone(),
            eval: self.eval.clone(),
            return_type: self.return_type.clone(),
            name: self.name.clone(),
            id: self.id.clone(),
        }
    }
}
