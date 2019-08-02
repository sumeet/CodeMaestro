use super::lang;
use objekt::clone_trait_object;

pub trait SettableArgs: objekt::Clone + lang::Function {
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>);
}

clone_trait_object!(SettableArgs);
