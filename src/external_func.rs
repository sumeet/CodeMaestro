use super::lang;

pub trait ModifyableFunc: lang::Function {
    fn set_return_type(&mut self, return_type: lang::Type);
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>);
    fn clone(&self) -> Self;
}
