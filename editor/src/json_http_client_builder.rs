use itertools::Itertools;
use lazy_static::lazy_static;
use serde_json;
use std::collections::BTreeMap;

use super::async_executor::AsyncExecutor;
use super::json2;
use crate::schema_builder;
use crate::schema_builder::{Schema, SchemaType};
use cs::await_eval_result;
use cs::builtins::{new_option, new_result};
use cs::env;
use cs::env_genie::EnvGenie;
use cs::json_http_client::{fetch_json, serde_value_to_lang_value_wrapped_in_enum, JSONHTTPClient};
use cs::lang;
use cs::structs;

mod fake_http_client;

pub const NAME_OF_ROOT: &'static str = "Response";

// TODO: move this one func somewhere else? or keep it in this file?
// i think this is always going to be a Result, because an HTTP request can always fail
pub fn value_response_for_test_output(
    env: &env::ExecutionEnvironment,
    serde_json_value: &serde_json::Value,
    return_type_candidate: &ReturnTypeBuilderResult)
    -> Result<HTTPResponseIntermediateValue, Box<dyn std::error::Error>> {
    let mut new_fake_env = env.clone();
    for strukt in &return_type_candidate.structs_to_be_added {
        new_fake_env.add_typespec(strukt.clone());
    }
    let value = serde_value_to_lang_value_wrapped_in_enum(serde_json_value,
                                                          &return_type_candidate.typ,
                                                          &new_fake_env)?;
    Ok(HTTPResponseIntermediateValue { env: new_fake_env,
                                       value })
}

// TODO: this should probably contain the raw HTTP response as well
pub struct HTTPResponseIntermediateValue {
    // note that the Value only works with the env from before
    pub value: lang::Value,
    pub env: env::ExecutionEnvironment,
}

impl HTTPResponseIntermediateValue {
    pub fn from_builder(env: &env::ExecutionEnvironment,
                        builder: &JSONHTTPClientBuilder)
                        -> Option<Self> {
        // value_response_for_test_output(env,
        //                                builder.test_run_result.as_ref()?.as_ref().ok()?,
        //                                builder.return_type_candidate.as_ref()?).ok()
        Some(value_response_for_test_output(env,
                                            builder.test_run_result.as_ref()?.as_ref().ok()?,
                                            builder.return_type_candidate.as_ref()?).unwrap())
    }
}

#[derive(Clone)]
pub struct JSONHTTPClientBuilder {
    pub test_run_result: Option<Result<serde_json::Value, String>>,
    pub test_run_parsed_doc: Option<json2::ParsedDocument>,
    pub external_schema: Option<schema_builder::Schema>,
    pub json_http_client_id: lang::ID,
    pub selected_fields: Vec<SelectedField>,
    pub return_type_candidate: Option<ReturnTypeBuilderResult>,
}

#[derive(Clone, Debug)]
pub struct SelectedField {
    pub name: String,
    pub nesting: json2::Nesting,
    pub typespec_id: lang::ID,
}

impl JSONHTTPClientBuilder {
    // TODO: this will need to change to not make a GET request, but use the method from the
    // client
    pub fn new(json_http_client_id: lang::ID) -> Self {
        Self { test_run_result: None,
               test_run_parsed_doc: None,
               external_schema: None,
               return_type_candidate: None,
               json_http_client_id,
               selected_fields: vec![] }
    }

    // this function is where i need to strike next
    pub fn rebuild_return_type(&mut self, env: &mut env::ExecutionEnvironment) {
        // TODO: might not want to denormalize structs, but instead read them off the client
        // but for now we'll denormalize
        if let Some(return_type_candidate) = self.return_type_candidate.as_ref() {
            for strukt in &return_type_candidate.structs_to_be_added {
                env.delete_typespec(strukt.id);
            }
        }

        let env_genie = EnvGenie::new(env);

        if self.external_schema.is_none() {
            return;
        }

        let external_schema = self.external_schema.as_ref().unwrap();
        self.return_type_candidate =
            Some(build_return_type(&env_genie, &external_schema.typ, external_schema.optional));
        // TODO: inside here, append the structs to the actual JSON HTTP function, and also
        // stick them into the environment
        if self.return_type_candidate.is_none() {
            return;
        }
        let return_type_candidate = self.return_type_candidate.as_ref().unwrap();
        let mut http_client = env_genie.get_json_http_client(self.json_http_client_id)
                                       .unwrap()
                                       .clone();
        http_client.intermediate_parse_structs = return_type_candidate.structs_to_be_added.clone();
        http_client.intermediate_parse_schema = return_type_candidate.type_wrapped_in_result_enum();
        http_client.intermediate_parse_argument =
            JSONHTTPClient::build_intermediate_parse_argument(return_type_candidate.type_wrapped_in_result_enum());
        for strukt in &http_client.intermediate_parse_structs {
            env.add_typespec(strukt.clone());
        }
        env.add_function(http_client);
    }

    fn set_test_result(&mut self, result: Result<serde_json::Value, String>) {
        self.test_run_result = Some(result.clone());
        if let Ok(value) = result {
            let parsed_doc = json2::parse(value);
            self.external_schema = Some(Schema::from_parsed_doc_root(&parsed_doc));
            self.test_run_parsed_doc = Some(parsed_doc);
        } else {
            self.test_run_parsed_doc = None
        }
        self.selected_fields.clear()
    }

    pub fn run_test<F: FnOnce(Self) + 'static>(&self,
                                               interp: &env::Interpreter,
                                               async_executor: &mut AsyncExecutor,
                                               callback: F) {
        // this is very tricky, we're going to call out to the actual implementation of the HTTP
        // client (which is a work in progress). we don't want to actually execute the function
        // though, we instead want to intercept the HTTP request it makes and grab the response.

        // so this is a crazy mess of closures, Arc<Mutex<>> and we even clone the
        // ExecutionEnvironment (interpreter environment) and include a fake version of this
        // Function
        let mut fake_interp = interp.deep_clone_env();

        let mut fake_http_client = {
            let mut fake_env = fake_interp.env.borrow_mut();
            let real_http_client = {
                let env_genie = EnvGenie::new(&fake_env);
                env_genie.get_json_http_client(self.json_http_client_id)
                         .unwrap()
                         .clone()
            };
            let fake_http_client = fake_http_client::FakeHTTPClient::new(real_http_client);
            fake_env.add_function(fake_http_client.clone());
            fake_http_client
        };

        let mut new_builder = self.clone();
        let test_code = fake_http_client.test_code();
        async_executor.exec(async move {
            let fut = fake_interp.evaluate(&test_code);
            await_eval_result!(fut);
            let req =
                fake_http_client.take_made_request()
                                .expect("need to handle when there was no request, probably a popup");
            let json_value_result = fetch_json(req).await;
            new_builder.set_test_result(json_value_result.map_err(|e| e.to_string()));
            callback(new_builder);
            let ok: Result<(), ()> = Ok(());
            ok
        });
    }
}

fn build_return_type(env_genie: &EnvGenie,
                     schema_type: &SchemaType,
                     optional: bool)
                     -> ReturnTypeBuilderResult {
    let return_type_spec = ReturnTypeSpec::from_schema_type(schema_type, optional);
    ReturnTypeBuilder::new(NAME_OF_ROOT, env_genie, &return_type_spec).build()
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum ReturnTypeSpec {
    Struct(BTreeMap<String, ReturnTypeSpec>),
    List(Box<ReturnTypeSpec>),
    Scalar { typespec_id: lang::ID },
    Optional(Box<ReturnTypeSpec>),
}

impl ReturnTypeSpec {
    pub fn from_schema_type(schema_type: &SchemaType, optional: bool) -> Self {
        let return_type_spec = match schema_type {
            SchemaType::String { example: _ } => {
                ReturnTypeSpec::Scalar { typespec_id: lang::STRING_TYPESPEC.id }
            }
            SchemaType::Number { example: _ } => {
                ReturnTypeSpec::Scalar { typespec_id: lang::NUMBER_TYPESPEC.id }
            }
            SchemaType::Boolean { example: _ } => {
                ReturnTypeSpec::Scalar { typespec_id: lang::BOOLEAN_TYPESPEC.id }
            }
            SchemaType::Null => ReturnTypeSpec::Scalar { typespec_id: lang::NULL_TYPESPEC.id },
            SchemaType::List { schema } => {
                ReturnTypeSpec::List(Box::new(Self::from_schema_type(&schema.typ, schema.optional)))
            }
            SchemaType::Object { map } => {
                let return_type_spec_by_name = map.iter().map(|(field_name, inner_schema)| {
                    (field_name.to_owned(), Self::from_schema_type(&inner_schema.typ, inner_schema.optional))
                }).collect();
                ReturnTypeSpec::Struct(return_type_spec_by_name)
            }
            SchemaType::CameFromUnsupportedList => {
                panic!("schema contained either heterogeneous list or empty list, no handling for this yet")
            }
        };
        if optional {
            Self::Optional(Box::new(return_type_spec))
        } else {
            return_type_spec
        }
    }
}

pub struct ReturnTypeBuilder<'a> {
    current_field_name: &'a str,
    pub built_structs: Vec<structs::Struct>,
    pub env_genie: &'a EnvGenie<'a>,
    pub return_type_spec: &'a ReturnTypeSpec,
}

impl<'a> ReturnTypeBuilder<'a> {
    pub fn new(current_field_name: &'a str,
               env_genie: &'a EnvGenie<'a>,
               return_type: &'a ReturnTypeSpec)
               -> Self {
        Self { current_field_name,
               env_genie,
               built_structs: vec![],
               return_type_spec: return_type }
    }

    fn build(&mut self) -> ReturnTypeBuilderResult {
        match self.return_type_spec {
            ReturnTypeSpec::Scalar { typespec_id } => {
                ReturnTypeBuilderResult { structs_to_be_added: vec![],
                                          typ: lang::Type::from_spec_id(*typespec_id, vec![]) }
            }
            ReturnTypeSpec::List(returntypespec) => {
                let mut result = ReturnTypeBuilder::new(self.current_field_name,
                                                        self.env_genie,
                                                        returntypespec.as_ref()).build();
                result.typ = lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![result.typ]);
                result
            }
            ReturnTypeSpec::Struct(map) => {
                let mut structs_to_be_added = vec![];
                let struct_fields = map.iter()
                                       .map(|(key, returntypespec)| {
                                           let result =
                                               ReturnTypeBuilder::new(key,
                                                                      self.env_genie,
                                                                      returntypespec).build();
                                           structs_to_be_added.extend(result.structs_to_be_added);
                                           structs::StructField::new(key.clone(),
                                              "Auto derived by JSON inspector".into(), result.typ)
                                       })
                                       .collect_vec();
                // TODO: should kill off this find existing struct matching part... because now the
                // struct is an intermediate representation. no need to match it with external structs
                let typespec_id = self.find_existing_struct_matching(&struct_fields)
                                      .map(|strukt| strukt.id)
                                      .unwrap_or_else(|| {
                                          let mut strukt = structs::Struct::new();
                                          strukt.name = self.current_field_name.to_owned();
                                          strukt.fields = struct_fields;
                                          let id = strukt.id;
                                          structs_to_be_added.push(strukt);
                                          id
                                      });
                // not sure if this actually works
                //structs_to_be_added.dedup_by_key(|s| normalize_struct_fields(&s.fields));
                ReturnTypeBuilderResult { structs_to_be_added,
                                          typ: lang::Type::from_spec_id(typespec_id, vec![]) }
            }
            ReturnTypeSpec::Optional(inner_return_type_spec) => {
                let mut result = ReturnTypeBuilder::new(self.current_field_name,
                                                        self.env_genie,
                                                        inner_return_type_spec.as_ref()).build();
                result.typ = new_option(result.typ);
                result
            }
        }
    }

    fn find_existing_struct_matching(&self,
                                     structfields: &Vec<structs::StructField>)
                                     -> Option<&structs::Struct> {
        self.env_genie.list_structs().find(|strukt| {
                                         strukt.fields.len() == structfields.len() && {
                normalize_struct_fields(&strukt.fields) == normalize_struct_fields(&structfields)
            }
                                     })
    }
}

lazy_static! {
    static ref HTTP_ERROR_TYPESPEC_ID: lang::ID =
        uuid::Uuid::parse_str("5e9e5cec-415f-4949-b178-7793fba5ad5c").unwrap();
    static ref HTTP_ERROR_TYPE: lang::Type =
        lang::Type::from_spec_id(*HTTP_ERROR_TYPESPEC_ID, vec![]);
}

fn normalize_struct_fields(fields: &[structs::StructField]) -> BTreeMap<String, lang::ID> {
    fields.iter()
          .map(|field| (field.name.clone(), field.field_type.hash()))
          .collect()
}

#[derive(Debug, Clone)]
pub struct ReturnTypeBuilderResult {
    pub structs_to_be_added: Vec<structs::Struct>,
    typ: lang::Type,
}

impl ReturnTypeBuilderResult {
    pub fn type_wrapped_in_result_enum(&self) -> lang::Type {
        new_result(self.typ.clone(), HTTP_ERROR_TYPE.clone())
    }
}
