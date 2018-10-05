use super::lang;
use super::lang::CodeNode;
use super::{Print};
use super::failure::{Error,err_msg};
use super::serde_json;
use super::uuid::Uuid;


//fn load(filename: &str) -> Result<CodeNode,Error> {
//    let mut f = File::open(filename)?;
//}


pub fn serialize(code_node: &CodeNode) -> Result<String,Error> {
    match serde_json::to_string(&code_node) {
       Ok(string) => Ok(string),
       Err(e) => Err(Error::from(e)),
    }
}

pub fn deserialize(str: &str) -> Result<CodeNode,Error> {
    let value = serde_json::from_str(str)?;
    deserialize_value(&value)
}

pub fn deserialize_value(value: &serde_json::Value) -> Result<CodeNode, Error> {
    match value {
        serde_json::Value::Object(map) => {
            if map.len() != 1 {
                return Err(err_msg("Node definitions only have a single key"))
            }
            deserialize_node(map.keys().next().unwrap(), &map.values().next().unwrap())
        }
        _ => {
            Err(err_msg(format!("invalid value: {:?}", value)))
        }
    }
}

pub fn deserialize_values(array: &Vec<serde_json::Value>) -> Result<Vec<CodeNode>, Error> {
    array.iter().map(deserialize_value).collect()
}

pub fn deserialize_node(node_name: &str, value: &serde_json::Value) -> Result<CodeNode, Error> {
    match node_name {
        "Block" => {
            Ok(CodeNode::Block(lang::Block {
                //expressions: deserialize_values(expressions_value.unwrap().as_array().unwrap())?,
                expressions: read_code_nodes(value, "expressions")?,
                id: read_id(value, "id")?
            }))
        },

        "Assignment" => {
            Ok(CodeNode::Assignment(lang::Assignment {
                name: read_string(value, "name")?.to_string(),
                expression: Box::new(read_code_node(value, "expression")?),
                id: read_id(value, "id")?
            }))
        },

        "StringLiteral" => {
            Ok(CodeNode::StringLiteral(lang::StringLiteral {
                value: read_string(value, "value")?.to_string(),
                id: read_id(value, "id")?
            }))
        },


        "FunctionCall" => {
            Ok(CodeNode::FunctionCall(lang::FunctionCall {
                function: Box::new(Print{}),
                args: read_code_nodes(value, "args")?,
                id: read_id(value, "id")?
            }))
        },


        "VariableReference" => {
            Ok(CodeNode::VariableReference(lang::VariableReference {
                assignment_id: read_id(value, "assignment_id")?,
                id: read_id(value, "id")?
            }))
        },

        _ => Err(err_msg(format!("invalid node name: {}", node_name)))
    }
}

fn read_string<'a, 'b>(value: &'a serde_json::Value, field_name: &'b str) -> Result<&'a str,Error> {
    let field = value.get(field_name);
    match field {
        Some(serde_json::Value::String(string)) => Ok(string),
        _ => Err(err_msg(format!("no code node field found")))
    }

}

fn read_code_node(value: &serde_json::Value, field_name: &str) -> Result<lang::CodeNode,Error> {
    let field = value.get(field_name);
    match field {
        Some(serde_json::Value::Object(_)) => deserialize_value(field.unwrap()),
        _ => Err(err_msg(format!("no code node field found")))
    }
}


fn read_code_nodes(value: &serde_json::Value, field_name: &str) -> Result<Vec<lang::CodeNode>,Error> {
    let field = value.get(field_name);
    match field {
        Some(serde_json::Value::Array(array)) => deserialize_values(array),
        _ => Err(err_msg(format!("no code node field found")))
    }
}

fn read_id(value: &serde_json::Value, field_name: &str) -> Result<lang::ID, Error> {
    match value.get(field_name) {
        Some(serde_json::Value::String(string)) => Ok(Uuid::parse_str(string)?),
        _ => Err(err_msg(format!("no ID field found in {:?}", value)))
    }
}