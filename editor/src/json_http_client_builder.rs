use itertools::Itertools;
use serde_json;
use std::collections::BTreeMap;

use super::async_executor::AsyncExecutor;
use super::json2;
use cs::env_genie::EnvGenie;
use cs::http_request;
use cs::json_http_client;
use cs::lang;
use cs::structs;

#[derive(Clone)]
pub struct JSONHTTPClientBuilder {
    pub test_url: String,
    pub test_run_result: Option<Result<serde_json::Value, String>>,
    pub test_run_parsed_doc: Option<json2::ParsedDocument>,
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
        Self { test_url: "https://httpbin.org/anything".to_string(),
               test_run_result: None,
               test_run_parsed_doc: None,
               return_type_candidate: None,
               json_http_client_id,
               selected_fields: vec![] }
    }

    pub fn get_selected_field(&self, nesting: &json2::Nesting) -> Option<&SelectedField> {
        self.selected_fields
            .iter()
            .find(|field| &field.nesting == nesting)
    }

    pub fn add_selected_field(&mut self, nesting: json2::Nesting, env_genie: &EnvGenie) {
        let field = self.test_run_parsed_doc
                        .as_ref()
                        .unwrap()
                        .find(&nesting)
                        .expect("couldn't find field for some reason");
        self.selected_fields
            .push(SelectedField { name: gen_field_name(&nesting),
                                  nesting,
                                  typespec_id: get_typespec_id(field) });
        self.rebuild_return_type(env_genie)
    }

    pub fn remove_selected_field(&mut self, nesting: json2::Nesting, env_genie: &EnvGenie) {
        self.selected_fields
            .drain_filter(|field| field.nesting == nesting);
        self.rebuild_return_type(env_genie)
    }

    fn rebuild_return_type(&mut self, env_genie: &EnvGenie) {
        self.return_type_candidate = build_return_type(env_genie, &self.selected_fields)
    }

    fn set_test_result(&mut self, result: Result<serde_json::Value, String>) {
        self.test_run_result = Some(result.clone());
        if let Ok(value) = result {
            self.test_run_parsed_doc = Some(json2::parse(value))
        } else {
            self.test_run_parsed_doc = None
        }
        self.selected_fields.clear()
    }

    pub fn run_test<F: FnOnce(JSONHTTPClientBuilder) + 'static>(&self,
                                                                async_executor: &mut AsyncExecutor,
                                                                callback: F) {
        let url = self.test_url.clone();
        let mut new_builder = self.clone();
        async_executor.exec(async move {
                          let val = do_get_request(url).await;
                          let result = val.map_err(|e| e.to_string());
                          new_builder.set_test_result(result);
                          callback(new_builder);
                          let ok: Result<(), ()> = Ok(());
                          ok
                      });
    }
}

fn gen_field_name(nesting: &json2::Nesting) -> String {
    nesting.iter()
           .filter_map(|n| match n {
               json2::Nest::MapKey(key) => Some(key.as_str()),
               _ => None,
           })
           .last()
           .unwrap_or("h00000what")
           .to_string()
}

fn build_return_type(env_genie: &EnvGenie,
                     selected_fields: &[SelectedField])
                     -> Option<ReturnTypeBuilderResult> {
    let return_type_spec = make_return_type_spec(selected_fields).ok()?;
    Some(ReturnTypeBuilder::new(env_genie, &return_type_spec).build())
}

async fn do_get_request(url: String) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    json_http_client::get_json(http_request::get(&url)?).await
}

pub fn get_typespec_id(parsed_doc: &json2::ParsedDocument) -> lang::ID {
    use json2::ParsedDocument;
    match parsed_doc {
        ParsedDocument::Null { .. } => lang::NULL_TYPESPEC.id,
        ParsedDocument::Bool { .. } => lang::BOOLEAN_TYPESPEC.id,
        ParsedDocument::String { .. } => lang::STRING_TYPESPEC.id,
        ParsedDocument::Number { .. } => lang::NUMBER_TYPESPEC.id,
        ParsedDocument::NonHomogeneousCantParse { .. }
        | ParsedDocument::EmptyCantInfer { .. }
        | ParsedDocument::Map { .. }
        | ParsedDocument::List { .. } => {
            panic!("we don't support selecting these types, smth's wrong")
        }
    }
}

fn make_return_type_spec(selected_fields: &[SelectedField]) -> Result<ReturnTypeSpec, &str> {
    if selected_fields.is_empty() {
        return Err("no selected fields");
    }

    if selected_fields.len() == 1 && selected_fields[0].nesting.is_empty() {
        return Ok(ReturnTypeSpec::Scalar { typespec_id: selected_fields[0].typespec_id });
    }

    // this is a placeholder, really this could be anything
    let mut return_type_spec = ReturnTypeSpec::Struct(BTreeMap::new());
    for selected_field in selected_fields {
        let scalar = ReturnTypeSpec::Scalar { typespec_id: selected_field.typespec_id };

        let mut current_return_type_spec = &mut return_type_spec;
        for nest in &selected_field.nesting {
            match nest {
                json2::Nest::MapKey(key) => {
                    current_return_type_spec =
                        current_return_type_spec.insert_key(key, scalar.clone());
                }
                json2::Nest::ListElement(_) => {
                    current_return_type_spec = current_return_type_spec.into_list(scalar.clone());
                }
            }
        }
    }
    Ok(return_type_spec)
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum ReturnTypeSpec {
    Struct(BTreeMap<String, ReturnTypeSpec>),
    List(Box<ReturnTypeSpec>),
    Scalar { typespec_id: lang::ID },
}

impl ReturnTypeSpec {
    fn insert_key(&mut self, key: &String, rts: ReturnTypeSpec) -> &mut ReturnTypeSpec {
        match self {
            ReturnTypeSpec::Struct(map) => map.entry(key.clone()).or_insert(rts),
            ReturnTypeSpec::List(box of) => of.insert_key(key, rts),
            // in this instance, the Scalar acts as a placeholder. let's clobber it!
            ReturnTypeSpec::Scalar { .. } => {
                *self = ReturnTypeSpec::Struct(BTreeMap::new());
                self.insert_key(key, rts)
            }
        }
    }

    fn into_list(&mut self, rts: ReturnTypeSpec) -> &mut ReturnTypeSpec {
        match self {
            ReturnTypeSpec::Struct(_) | ReturnTypeSpec::Scalar { .. } => {
                *self = ReturnTypeSpec::List(Box::new(rts));
                self
            }
            ReturnTypeSpec::List(_) => self,
        }
    }
}

pub struct ReturnTypeBuilder<'a> {
    pub built_structs: Vec<structs::Struct>,
    pub env_genie: &'a EnvGenie<'a>,
    pub return_type_spec: &'a ReturnTypeSpec,
}

impl<'a> ReturnTypeBuilder<'a> {
    pub fn new(env_genie: &'a EnvGenie<'a>, return_type: &'a ReturnTypeSpec) -> Self {
        Self { env_genie,
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
                let mut result =
                    ReturnTypeBuilder::new(self.env_genie, returntypespec.as_ref()).build();
                result.typ = lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![result.typ]);
                result
            }
            ReturnTypeSpec::Struct(map) => {
                let mut structs_to_be_added = vec![];
                let struct_fields = map.iter()
                                       .map(|(key, returntypespec)| {
                                           let result =
                                               ReturnTypeBuilder::new(self.env_genie,
                                                                      returntypespec).build();
                                           structs_to_be_added.extend(result.structs_to_be_added);
                                           structs::StructField::new(key.clone(),
                                              "Auto derived by JSON inspector".into(), result.typ)
                                       })
                                       .collect_vec();
                let typespec_id = self.find_existing_struct_matching(&struct_fields)
                                      .map(|strukt| strukt.id)
                                      .unwrap_or_else(|| {
                                          let mut strukt = structs::Struct::new();
                                          strukt.fields = struct_fields;
                                          let id = strukt.id;
                                          structs_to_be_added.push(strukt);
                                          id
                                      });
                // not sure if this actually works
                structs_to_be_added.dedup_by_key(|s| normalize_struct_fields(&s.fields));
                ReturnTypeBuilderResult { structs_to_be_added,
                                          typ: lang::Type::from_spec_id(typespec_id, vec![]) }
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

fn normalize_struct_fields(fields: &[structs::StructField]) -> BTreeMap<String, lang::ID> {
    fields.iter()
          .map(|field| (field.name.clone(), field.field_type.id()))
          .collect()
}

#[derive(Debug, Clone)]
pub struct ReturnTypeBuilderResult {
    pub structs_to_be_added: Vec<structs::Struct>,
    pub typ: lang::Type,
}
