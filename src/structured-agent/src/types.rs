use std::any::Any;

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub name: String,
}

impl Type {
    pub fn string() -> Self {
        Self {
            name: "String".to_string(),
        }
    }

    pub fn unit() -> Self {
        Self {
            name: "()".to_string(),
        }
    }
}

pub trait Expression: std::fmt::Debug {
    fn evaluate(
        &self,
        context: &mut crate::runtime::Context,
    ) -> Result<crate::runtime::ExprResult, String>;
    fn return_type(&self) -> Type;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Expression>;
}

pub trait LanguageEngine {
    fn untyped(&self, context: &crate::runtime::Context) -> String;
}

pub struct PrintEngine {}

impl LanguageEngine for PrintEngine {
    fn untyped(&self, _context: &crate::runtime::Context) -> String {
        println!("PrintEngine {{}}");
        "PrintEngine {}".to_string()
    }
}
