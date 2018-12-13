use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use pyo3::prelude::*;
use pyo3::types::{PyIterator,PyObjectRef,PyDict};

use super::lang;
use super::env;
use super::external_func;

thread_local! {
    pub static GILGUARD: Rc<GILGuard> = Rc::new(Python::acquire_gil());
    pub static NAMESPACE_BY_FUNC_ID : RefCell<HashMap<lang::ID, Rc<PyNamespace>>> =
        RefCell::new(HashMap::new());
}

fn getgil() -> Rc<GILGuard> {
    GILGUARD.with(|gg| Rc::clone(&gg))
}

fn get_namespace(id: lang::ID) -> Rc<PyNamespace> {
    NAMESPACE_BY_FUNC_ID.with(|namespace_by_func_id| {
        let mut namespace_by_func_id = namespace_by_func_id.borrow_mut();
        if !namespace_by_func_id.contains_key(&id) {
            let newnamespace = PyNamespace::new(&getgil());
            namespace_by_func_id.insert(id, Rc::new(newnamespace));
        }
        Rc::clone(namespace_by_func_id.get(&id).unwrap())
    })
}

fn newdict(py: Python) -> Py<PyDict> {
    PyDict::new(py).into()
}

pub struct PyNamespace {
    locals: Py<PyDict>,
}

impl PyNamespace {
    // TODO: this could also own a ref to the GILGuard, i think if we want to ever do multiple interpreters
    fn new(gilguard: &GILGuard) -> Self {
        Self {
            locals: newdict(gilguard.python()),
        }
    }

    fn locals(&self, py: Python) -> &PyDict {
        self.locals.as_ref(py)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PyFunc {
    pub prelude: String,
    // TODO: eval can only be a single expression. so there should probably be a `run` step that can execute
    // statements and stuff, and then a final line to be evaluated
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub id: lang::ID,
    pub args: Vec<lang::ArgumentDefinition>,
}

impl ToPyObject for lang::Value {
    fn to_object(&self, py: Python) -> PyObject {
        use super::lang::Value::*;
        match self {
            Null => ().to_object(py),
            String(s) => s.to_object(py),
            // not quite sure what to do with these...
            Error(e) => format!("{:?}", e).to_object(py),
            Number(i) => i.to_object(py),
            List(v) => v.to_object(py),
        }
    }
}

// TODO: i think a ref to the python interpreter could live inside of here
impl PyFunc {
    pub fn new() -> Self {
        Self {
            prelude: "".to_string(),
            eval: "".to_string(),
            return_type: lang::Type::from_spec(&lang::NULL_TYPESPEC),
            name: "New PyFunc".to_string(),
            id: lang::new_id(),
            args: vec![],
        }
    }
}

impl PyFunc {
    fn extract(&self, pyobjectref: &PyObjectRef) -> lang::Value {
        use self::lang::Function;
        self.ex(pyobjectref, &self.returns())
    }

    fn ex(&self, pyobjectref: &PyObjectRef, into_type: &lang::Type) -> lang::Value {
        let gil = getgil();
        let py = gil.python();

        // TODO: this is terrible. i should've used the FromPyObject trait instead of this
        // verbose bullshit
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
            // TODO: error handlign! just figure out what's neccessary by testing it out in the
            // GUI
            let iter = PyIterator::from_object(py, &pyobj).unwrap();
            let collected : Vec<lang::Value> = iter
                .map(|pyresult| {self.ex(pyresult.unwrap(), collection_type)})
                .collect();
            return lang::Value::List(collected)
        }
        lang::Value::Error(lang::Error::PythonDeserializationError)
    }

    fn py_exception_to_error(&self, pyerror: &PyErr) -> lang::Error {
        let gil = getgil();
        let py = gil.python();
        let error_obj = pyerror.into_object(py);
        let error_cls = error_obj.getattr(py, "__class__")
            .unwrap();
        let error_cls_name = error_cls.getattr(py, "__name__")
            .unwrap();
        lang::Error::PythonError(self.str(error_cls_name), self.str(error_obj))
    }

    fn str(&self, pyobj: PyObject) -> String {
        let gil = getgil();
        let py = gil.python();
        pyobj.getattr(py, "__str__").unwrap().call0(py).unwrap()
            .extract(py).unwrap()
    }
}

fn mix_args_with_locals<'a>(py: Python, args: &'a PyDict, from: PyObject) -> &'a PyDict {
    let mixed = args.copy().unwrap();
    let mixed_as_obj = mixed.to_object(py);
    mixed_as_obj.getattr(py, "update").unwrap().call1(py, (from,)).unwrap();
    mixed
}

impl lang::Function for PyFunc {
    fn call(&self, _env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let gil = getgil();
        let py = gil.python();

        let our_namespace = get_namespace(self.id);
        let locals = our_namespace.locals(py);

        // TODO: only run the prelude once, instead of every time the func is called
        let result = py.run(&self.prelude, None, Some(locals));

        let named_args = external_func::to_named_args(self, args);
        let pd = named_args.to_object(py);
        let locals_with_params = mix_args_with_locals(py, locals, pd);

        if let Err(e) = result {
            lang::Value::Error(self.py_exception_to_error(&e))
        } else {
            let eval_result = py.eval(self.eval.as_ref(), None, Some(locals_with_params));
            if let Err(pyerr) = eval_result {
                return lang::Value::Error(self.py_exception_to_error(&pyerr))
            }
            let eval_result = eval_result.unwrap();
            self.extract(eval_result)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    // XXX: this should really not copy, it should return a reference
    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        self.args.clone()
    }

    fn returns(&self) -> lang::Type {
        self.return_type.clone()
    }
}

impl external_func::ModifyableFunc for PyFunc {
    fn set_return_type(&mut self, return_type: lang::Type) {
        self.return_type = return_type
    }

    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>) {
        self.args = args
    }

    fn clone(&self) -> Self {
        PyFunc {
            prelude: self.prelude.clone(),
            eval: self.eval.clone(),
            return_type: self.return_type.clone(),
            name: self.name.clone(),
            id: self.id.clone(),
            args: self.args.clone(),
        }
    }
}
