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
    match serde_json::from_str(str) {
        Ok(code_node) => Ok(code_node),
        Err(e) => Err(Error::from(e)),
    }
}
