use std::{collections::HashMap, fmt, sync::Arc};

use nonempty::NonEmpty;

pub trait SourceRef {}
pub trait AstRef {}
pub trait BodyRef {}
pub trait WitnessRef {}

#[derive(Clone)]
pub struct NoAst;
impl AstRef for NoAst {}

pub trait References {
    type Source: SourceRef;
    type Ast: AstRef;
    type Body: BodyRef;
    type Witness: WitnessRef;
}

pub trait SymbolQuery {
    type Refs: References;

    fn module(&self, name: &ModuleName) -> Option<Arc<ModuleDefinition<Self::Refs>>>;
    fn function(&self, name: &FunctionName) -> Option<Arc<FunctionDefinition<Self::Refs>>>;
    fn type_def(&self, name: &TypeName) -> Option<Arc<TypeDefinition<Self::Refs>>>;
    fn trait_def(&self, name: &TraitName) -> Option<Arc<TraitDefinition<Self::Refs>>>;
    fn impl_for(
        &self,
        type_name: &TypeName,
        trait_name: &TraitName,
    ) -> Option<Arc<ImplDefinition<Self::Refs>>>;
    fn traits_implemented_by(&self, type_name: &TypeName) -> Vec<Arc<ImplDefinition<Self::Refs>>>;

    fn all_functions(&self) -> Vec<Arc<FunctionDefinition<Self::Refs>>>;
    fn all_types(&self) -> Vec<Arc<TypeDefinition<Self::Refs>>>;
    fn functions_in_module(&self, module: &ModuleName) -> Vec<Arc<FunctionDefinition<Self::Refs>>>;
}

pub struct MetaData<R: References> {
    pub modules: HashMap<ModuleName, Arc<ModuleDefinition<R>>>,
    pub functions: HashMap<FunctionName, Arc<FunctionDefinition<R>>>,
    pub types: HashMap<TypeName, Arc<TypeDefinition<R>>>,
    pub traits: HashMap<TraitName, Arc<TraitDefinition<R>>>,
    pub impls: HashMap<ImplKey, Arc<ImplDefinition<R>>>,
}

impl<R: References> Default for MetaData<R> {
    fn default() -> Self {
        MetaData {
            modules: HashMap::new(),
            functions: HashMap::new(),
            types: HashMap::new(),
            traits: HashMap::new(),
            impls: HashMap::new(),
        }
    }
}

impl<R: References> SymbolQuery for MetaData<R> {
    type Refs = R;

    fn module(&self, name: &ModuleName) -> Option<Arc<ModuleDefinition<R>>> {
        self.modules.get(name).cloned()
    }

    fn function(&self, name: &FunctionName) -> Option<Arc<FunctionDefinition<R>>> {
        self.functions.get(name).cloned()
    }

    fn type_def(&self, name: &TypeName) -> Option<Arc<TypeDefinition<R>>> {
        self.types.get(name).cloned()
    }

    fn trait_def(&self, name: &TraitName) -> Option<Arc<TraitDefinition<R>>> {
        self.traits.get(name).cloned()
    }

    fn impl_for(
        &self,
        type_name: &TypeName,
        trait_name: &TraitName,
    ) -> Option<Arc<ImplDefinition<R>>> {
        self.impls
            .get(&ImplKey {
                type_name: type_name.clone(),
                trait_name: trait_name.clone(),
            })
            .cloned()
    }

    fn traits_implemented_by(&self, type_name: &TypeName) -> Vec<Arc<ImplDefinition<R>>> {
        self.impls
            .iter()
            .filter(|(k, _)| &k.type_name == type_name)
            .map(|(_, v)| v.clone())
            .collect()
    }

    fn all_functions(&self) -> Vec<Arc<FunctionDefinition<R>>> {
        self.functions.values().cloned().collect()
    }

    fn all_types(&self) -> Vec<Arc<TypeDefinition<R>>> {
        self.types.values().cloned().collect()
    }

    fn functions_in_module(&self, module: &ModuleName) -> Vec<Arc<FunctionDefinition<R>>> {
        self.functions
            .values()
            .filter(|f| &f.name.module == module)
            .cloned()
            .collect()
    }
}

impl<R: References> MetaData<R> {
    pub fn register_function(&mut self, name: FunctionName, def: Arc<FunctionDefinition<R>>) {
        self.functions.insert(name, def);
    }

    pub fn register_type(&mut self, name: TypeName, def: Arc<TypeDefinition<R>>) {
        self.types.insert(name, def);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleName {
    pub segments: NonEmpty<String>,
}

impl fmt::Display for ModuleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.segments.iter();
        write!(f, "{}", iter.next().unwrap())?;
        for segment in iter {
            write!(f, "::{}", segment)?;
        }
        Ok(())
    }
}

impl ModuleName {
    pub fn new(segments: NonEmpty<String>) -> Self {
        ModuleName { segments }
    }

    #[deprecated(note = "stop using magic strings for module names")]
    pub fn from_str(s: &str) -> Self {
        let v: Vec<String> = s.split("::").map(|p| p.to_string()).collect();
        ModuleName {
            segments: NonEmpty::from_vec(v).expect("split always yields at least one element"),
        }
    }

    #[deprecated(note = "transition marker: replace with the real module name")]
    pub fn unqualified() -> Self {
        ModuleName {
            segments: NonEmpty::new(String::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub enum ExportedName {
    Module(ModuleName),
    Function(FunctionName),
    Type(TypeName),
    Trait(TraitName),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UseImport {
    pub local: String,
    pub module: ModuleName,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ModuleDefinition<R: References> {
    pub name: ModuleName,
    pub visibility: Visibility,
    pub exports: Vec<ExportedName>,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
    pub use_imports: Vec<UseImport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionName {
    pub name: String,
    pub module: ModuleName,
    pub kind: FunctionNameKind,
}

impl fmt::Display for FunctionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let module_str = self.module.to_string();
        match &self.kind {
            FunctionNameKind::Function => {
                if module_str.is_empty() {
                    write!(f, "{}", self.name)
                } else {
                    write!(f, "{}::{}", self.module, self.name)
                }
            }
            FunctionNameKind::Impl {
                type_name,
                trait_name,
            } => {
                if module_str.is_empty() {
                    write!(f, "{}::{}::{}", type_name.name, trait_name.name, self.name)
                } else {
                    write!(
                        f,
                        "{}::{}::{}::{}",
                        self.module, type_name.name, trait_name.name, self.name
                    )
                }
            }
        }
    }
}

impl FunctionName {
    #[deprecated(note = "use structured constructors")]
    pub fn plain(module: &str, name: &str) -> Self {
        FunctionName {
            name: name.to_string(),
            module: ModuleName::new(NonEmpty::new(module.to_string())),
            kind: FunctionNameKind::Function,
        }
    }

    #[deprecated(note = "use structured constructors")]
    pub fn impl_fn(module: &str, type_name: &str, trait_name: &str, fn_name: &str) -> Self {
        let module_name = ModuleName::new(NonEmpty::new(module.to_string()));
        FunctionName {
            name: fn_name.to_string(),
            module: module_name.clone(),
            kind: FunctionNameKind::Impl {
                type_name: TypeName {
                    name: type_name.to_string(),
                    module: module_name.clone(),
                },
                trait_name: TraitName {
                    name: trait_name.to_string(),
                    module: module_name,
                },
            },
        }
    }

    #[deprecated(note = "use structured constructors")]
    pub fn from_qualified_str(s: &str) -> Self {
        match s.rsplit_once("::") {
            Some((module, name)) => FunctionName {
                name: name.to_string(),
                module: ModuleName::new(
                    NonEmpty::from_vec(module.split("::").map(|s| s.to_string()).collect())
                        .unwrap(),
                ),
                kind: FunctionNameKind::Function,
            },
            None => FunctionName {
                name: s.to_string(),
                module: ModuleName::new(NonEmpty::new(String::new())),
                kind: FunctionNameKind::Function,
            },
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        s.rsplit_once("::").map(|(module_str, name)| FunctionName {
            name: name.to_string(),
            module: ModuleName::new(
                NonEmpty::from_vec(module_str.split("::").map(|s| s.to_string()).collect())
                    .expect("non-empty after rsplit_once"),
            ),
            kind: FunctionNameKind::Function,
        })
    }

    #[deprecated(note = "use .name directly")]
    pub fn fn_name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition<R: References> {
    pub name: FunctionName,
    pub visibility: Visibility,
    pub type_name: TypeName,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
    pub body_ref: Option<R::Body>,
}

#[derive(Debug, Clone)]
pub struct ParameterDefinition {
    pub name: String,
    pub type_name: TypeName,
}

#[derive(Debug, Clone)]
pub struct GenericParameterDefinition {
    pub name: String,
    pub constraints: Vec<TraitName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FunctionNameKind {
    Function,
    Impl {
        type_name: TypeName,
        trait_name: TraitName,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImplKey {
    pub type_name: TypeName,
    pub trait_name: TraitName,
}

#[derive(Debug, Clone)]
pub struct ImplDefinition<R: References> {
    pub key: ImplKey,
    pub module: ModuleName,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeName {
    pub name: String,
    pub module: ModuleName,
}

impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.module, self.name)
    }
}

#[derive(Debug, Clone)]
pub struct SignatureEntry {
    pub name: String,
    pub type_name: TypeName,
}

#[derive(Debug, Clone)]
pub struct TypeDefinition<R: References> {
    pub name: TypeName,
    pub kind: TypeDefinitionKind,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
}

#[derive(Debug, Clone)]
pub enum TypeDefinitionKind {
    Struct {
        fields: Vec<FieldDefinition>,
    },
    Function {
        parameters: Vec<ParameterDefinition>,
        generic_parameters: Vec<GenericParameterDefinition>,
        return_type: TypeName,
    },
    Signature {
        entries: Vec<SignatureEntry>,
    },
    Primitive,
}

#[derive(Debug, Clone)]
pub struct FieldDefinition {
    pub name: String,
    pub type_name: TypeName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitName {
    pub name: String,
    pub module: ModuleName,
}

impl fmt::Display for TraitName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.module, self.name)
    }
}

#[derive(Debug, Clone)]
pub struct TraitDefinition<R: References> {
    pub name: TraitName,
    pub functions: Vec<SignatureEntry>,
    pub witness_ref: R::Witness,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
}
