#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Float,
    Double,
    Str,
    Generic(String),
    Union(Vec<Type>),
    Alias(String),
}

#[derive(Debug, Clone)]
pub struct FnSig {
    pub type_params: Vec<String>,
    pub param_types: Vec<Type>,
    pub return_type: Type,
}

pub struct Aliases(pub std::collections::HashMap<String, Vec<Type>>);

impl Aliases {
    pub fn members(&self, name: &str) -> Option<&Vec<Type>> {
        self.0.get(name)
    }

    pub fn flatten(&self, ty: &Type) -> Vec<Type> {
        match ty {
            Type::Union(vs) => vs.iter().flat_map(|v| self.flatten(v)).collect(),
            Type::Alias(name) => self
                .members(name)
                .map(|ms| ms.iter().flat_map(|m| self.flatten(m)).collect())
                .unwrap_or_default(),
            other => vec![other.clone()],
        }
    }

    pub fn is_subtype(&self, from: &Type, to: &Type) -> bool {
        let from_members = self.flatten(from);
        let to_members = self.flatten(to);
        from_members.iter().all(|f| to_members.contains(f))
    }

    pub fn apply_subst(&self, ty: &Type, subst: &std::collections::HashMap<String, Type>) -> Type {
        match ty {
            Type::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Union(vs) => Type::Union(vs.iter().map(|v| self.apply_subst(v, subst)).collect()),
            other => other.clone(),
        }
    }
}
