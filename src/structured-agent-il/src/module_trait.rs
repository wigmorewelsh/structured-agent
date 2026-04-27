pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn native_functions(&self) -> Vec<crate::NativeFunctionDef>;
}
