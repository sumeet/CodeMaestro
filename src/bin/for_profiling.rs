extern crate cs;

use cs::code_loading::TheWorld;
use cs::env::Interpreter;
use cs::{asynk, code_loading};
use tokio::runtime::current_thread::Runtime;

fn main() {
    let script_id = std::env::args().nth(1).unwrap().parse().unwrap();

    let mut interp = cs::init_interpreter();
    let codestring = include_str!("../../codesample.json");
    let the_world: code_loading::TheWorld = code_loading::deserialize(codestring).unwrap();
    load_world(&mut interp, &the_world);
    let script = the_world.scripts
                          .iter()
                          .find(|script| script.id() == script_id)
                          .unwrap();
    let mut runtime = Runtime::new().unwrap();
    runtime.block_on(asynk::backward(async {
                         let start_time = std::time::SystemTime::now();
                         println!("starting evaluation");
                         let value = interp.evaluate(&script.code()).await;
                         println!("got value: {:?}", value);
                         println!("total time: {:?}",
                                  std::time::SystemTime::now().duration_since(start_time));
                         Ok::<(), ()>(())
                     }))
           .unwrap();
}

// TODO: this is duped from lib.rs
pub fn load_world(interp: &mut Interpreter, world: &TheWorld) {
    let mut env = interp.env.borrow_mut();
    for function in &world.functions {
        env.add_function_box(function.clone());
    }
    for typespec in &world.typespecs {
        env.add_typespec_box(typespec.clone());
    }
}
