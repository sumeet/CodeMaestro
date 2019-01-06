use std::cell::RefCell;
use std::cmp;
use std::collections::HashMap;
use std::hash;
use std::rc::Rc;

use itertools::Itertools;
use pyo3::prelude::*;
use pyo3::types::{PyIterator,PyObjectRef,PyDict};
use serde_derive::{Serialize,Deserialize};

use super::env;
use super::external_func;
use super::external_func::{ValueWithEnv};
use super::lang;
use super::structs;

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

impl<'a> IntoPyObject for ValueWithEnv<'a> {
    fn into_object(self, py: Python) -> PyObject {
        use super::lang::Value::*;
        match (self.env, self.value) {
            (_, Null) => ().into_object(py),
            (_, String(s)) => s.into_object(py),
            // not quite sure what to do with these...
            (_, Error(e)) => format!("{:?}", e).into_object(py),
            (_, Number(i)) => i.into_object(py),
            (env, List(v)) => {
                v.into_iter().map(|item| Self { value: item, env: env }.into_object(py))
                    .collect_vec().into_object(py)
            },
            (env, Struct { struct_id, values }) => {
                let strukt = env.find_struct(struct_id).unwrap();
                let ms = py.eval("MaestroStruct", None, None).unwrap();
                let field_by_id = strukt.field_by_id();
                let value_by_name : HashMap<&str, ValueWithEnv> = values.into_iter()
                    .map(|(id, value)| {
                        let val_with_env = Self { value, env };
                        let name = &field_by_id.get(&id).unwrap().name;
                        (name.as_str(), val_with_env)
                    }).collect();

                ms.call1((&strukt.name, into_pyobject(value_by_name, py))).unwrap()
                    .to_object(py)
            },
            // TODO: unless we share the event loop with python, we should pry wait on the fut
            // before passing it along to python. i suppose that would require threading the
            // async executor through (which should be pretty doable)
            // some info on it here https://stackoverflow.com/questions/40329421/)
            //
            // but if we did that, then you'd need to know on the python side that it's a future
            //
            // i think instead, what we can do, is write a function from lang::Value => lang::Value
            // that recurses through the datastructure and if anything in there is a future, it'll
            // get resolved. then we just tell the interpreter resolve any futures before passing
            // them along to either a python function or a javascript function. i mean we're already
            // doing said recursion in here, so it should be pretty easy to implement!
            (_env, Future(_value_fut)) => unimplemented!(),
        }
    }
}

// TODO: i think a ref to the python interpreter could live inside of here
impl PyFunc {
    pub fn new() -> Self {
        Self {
            prelude: "".to_string(),
            eval: "".to_string(),
            return_type: lang::Type::from_spec(&*lang::NULL_TYPESPEC),
            name: "New PyFunc".to_string(),
            id: lang::new_id(),
            args: vec![],
        }
    }
}

impl PyFunc {
    fn extract(&self, pyobjectref: &PyObjectRef, env: &env::ExecutionEnvironment) -> lang::Value {
        use self::lang::Function;
        self.ex(pyobjectref, &self.returns(), env)
    }

    fn ex(&self, pyobjectref: &PyObjectRef, into_type: &lang::Type, env: &env::ExecutionEnvironment) -> lang::Value {
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
                .map(|pyresult| {
                    self.ex(pyresult.unwrap(), collection_type, env)
                })
                .collect();
            return lang::Value::List(collected)
        } else if let Some(strukt) = env.find_struct(into_type.typespec_id) {
            if let Ok(value) = self.pyobject_into_struct(pyobjectref, strukt, env) {
                return value
            }
        }
        lang::Value::Error(lang::Error::PythonDeserializationError)
    }

    fn pyobject_into_struct(&self, pyobjectref: &PyObjectRef, strukt: &structs::Struct,
                            env: &env::ExecutionEnvironment) -> PyResult<lang::Value> {
        let values : PyResult<HashMap<lang::ID, lang::Value>> = strukt.fields.iter()
            .map(|strukt_field| {
                let field_ref = try_get_name(pyobjectref, &strukt_field.name)?;
                Ok((strukt_field.id,
                    self.ex(&field_ref, &strukt_field.field_type, env)))
        }).collect();
        Ok(lang::Value::Struct { struct_id: strukt.id, values: values? })
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
    fn call(&self, env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let gil = getgil();
        let py = gil.python();

        let our_namespace = get_namespace(self.id);
        let locals = our_namespace.locals(py);

        // not passing in any locals so that the struct gets loaded into the module namespace
        let result = py.run(include_str!("maestro_struct.py"), None, None);
        if let Err(e) = result {
            return lang::Value::Error(self.py_exception_to_error(&e))
        }

        // TODO: only run the prelude once, instead of every time the func is called
        let result = py.run(&self.prelude, None, Some(locals));
        if let Err(e) = result {
            return lang::Value::Error(self.py_exception_to_error(&e))
        }

        let named_args : HashMap<String, ValueWithEnv> = external_func::to_named_args(self, args)
            .map(|(name, value)| (name, ValueWithEnv { env, value: value })).collect();

        let args_dict = into_pyobject(named_args, py);
        let locals_with_params = mix_args_with_locals(py, locals, args_dict);
        let eval_result = py.eval(self.eval.as_ref(), None, Some(locals_with_params));
        if let Err(pyerr) = eval_result {
            return lang::Value::Error(self.py_exception_to_error(&pyerr))
        }

        let eval_result = eval_result.unwrap();
        self.extract(eval_result, env)
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

fn into_pyobject<K, V, H>(from: HashMap<K, V, H>, py: Python) -> PyObject
    where
        K: hash::Hash + cmp::Eq + IntoPyObject,
        V: IntoPyObject,
        H: hash::BuildHasher,
{
    let dict = PyDict::new(py);
    for (key, value) in from {
        dict.set_item(key.into_object(py), value.into_object(py))
            .expect("Failed to set_item on dict");
    }
    dict.into()
}

fn try_get_name<'a>(pyobjectref: &'a PyObjectRef, name: &str) -> PyResult<&'a PyObjectRef> {
    if pyobjectref.hasattr("__getitem__")? {
        let getitem = pyobjectref.getattr("__getitem__")?;
        getitem.call1(name)
    } else {
        pyobjectref.getattr(name)
    }
}