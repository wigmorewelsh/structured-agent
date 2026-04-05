use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionName {
    pub module: String,
    pub kind: FunctionNameKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FunctionNameKind {
    Function {
        name: String,
    },
    Impl {
        type_name: String,
        trait_name: String,
        name: String,
    },
}

impl FunctionName {
    pub fn plain(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            kind: FunctionNameKind::Function { name: name.into() },
        }
    }

    pub fn impl_fn(
        module: impl Into<String>,
        type_name: impl Into<String>,
        trait_name: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            module: module.into(),
            kind: FunctionNameKind::Impl {
                type_name: type_name.into(),
                trait_name: trait_name.into(),
                name: name.into(),
            },
        }
    }

    pub fn from_qualified_str(s: &str) -> Self {
        let parts: Vec<&str> = s.splitn(2, "::").collect();
        if parts.len() == 2 {
            Self::plain(parts[0], parts[1])
        } else {
            Self::plain("", s)
        }
    }

    pub fn fn_name(&self) -> &str {
        match &self.kind {
            FunctionNameKind::Function { name } => name,
            FunctionNameKind::Impl { name, .. } => name,
        }
    }

    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = module.into();
        self
    }
}

impl fmt::Display for FunctionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FunctionNameKind::Function { name } => {
                if self.module.is_empty() {
                    write!(f, "{}", name)
                } else {
                    write!(f, "{}::{}", self.module, name)
                }
            }
            FunctionNameKind::Impl {
                type_name,
                trait_name,
                name,
            } => {
                if self.module.is_empty() {
                    write!(f, "{}::{}::{}", type_name, trait_name, name)
                } else {
                    write!(
                        f,
                        "{}::{}::{}::{}",
                        self.module, type_name, trait_name, name
                    )
                }
            }
        }
    }
}
