use super::lang;

pub trait ModifyableFunc: lang::Function {
    fn set_return_type(&mut self, return_type: lang::Type);
    fn clone(&self) -> Self;
}
