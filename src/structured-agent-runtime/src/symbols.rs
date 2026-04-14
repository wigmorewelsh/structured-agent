use std::{collections::HashMap, fmt, sync::Arc};

use crate::runtime_value::RuntimeValueFactory;

use nonempty::NonEmpty;

pub trait SourceRef {}
pub trait AstRef {}
pub trait BodyRef {}
pub trait WitnessRef: fmt::Debug + Clone {}
pub trait TypeAnnotation: fmt::Debug + Clone {}

impl TypeAnnotation for TypeName {}

#[derive(Clone)]
pub struct NoAst;
impl AstRef for NoAst {}

pub trait References {
    type Source: SourceRef;
    type Ast: AstRef;
    type Body: BodyRef;
    type Witness: WitnessRef;
    type TypeAnnotation: TypeAnnotation;
}

pub trait SymbolQuery {
    type Refs: References;

    fn module(&self, name: &ModuleName) -> Option<Arc<ModuleDefinition<Self::Refs>>>;
    fn function(&self, name: &FunctionName) -> Option<Arc<FunctionDefinition<Self::Refs>>>;
    fn type_def(&self, name: &TypeName) -> Option<Arc<TypeDefinition<Self::Refs>>>;
    fn impl_for(
        &self,
        type_name: &TypeName,
        trait_name: &TypeName,
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
    pub impls: HashMap<ImplKey, Arc<ImplDefinition<R>>>,
}

#[allow(deprecated)]
impl<R: References> Default for MetaData<R> {
    fn default() -> Self {
        MetaData {
            modules: HashMap::new(),
            functions: HashMap::new(),
            types: HashMap::new(),
            impls: HashMap::new(),
        }
    }
}

#[allow(deprecated)]
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

    fn impl_for(
        &self,
        type_name: &TypeName,
        trait_name: &TypeName,
    ) -> Option<Arc<ImplDefinition<R>>> {
        self.impls
            .iter()
            .find(|(k, _)| k.type_name == type_name.name && k.trait_name == trait_name.name)
            .map(|(_, v)| v.clone())
    }

    fn traits_implemented_by(&self, type_name: &TypeName) -> Vec<Arc<ImplDefinition<R>>> {
        self.impls
            .iter()
            .filter(|(k, _)| k.type_name == type_name.name)
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
    Trait(TypeName),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UseImport {
    pub local: String,
    pub module: ModuleName,
    pub name: String,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct ModuleDefinition<R: References> {
    pub name: ModuleName,
    pub visibility: Visibility,
    pub is_entry: bool,
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
                    write!(f, "{}::{}::{}", type_name, trait_name, self.name)
                } else {
                    write!(
                        f,
                        "{}::{}::{}::{}",
                        self.module, type_name, trait_name, self.name
                    )
                }
            }
        }
    }
}

impl FunctionName {
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
pub struct ParameterDefinition<R: References> {
    pub name: String,
    pub type_name: R::TypeAnnotation,
}

#[derive(Debug, Clone)]
pub struct GenericParameterDefinition<R: References> {
    pub name: String,
    pub constraints: Vec<R::TypeAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FunctionNameKind {
    Function,
    Impl {
        type_name: String,
        trait_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImplKey {
    pub type_name: String,
    pub trait_name: String,
    pub impl_module: ModuleName,
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

// this should be for pretty printing only.
impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.module, self.name)
    }
}

#[derive(Debug, Clone)]
pub struct SignatureEntry<R: References> {
    pub name: String,
    pub type_name: R::TypeAnnotation,
}

#[derive(Debug, Clone)]
pub struct TypeDefinition<R: References> {
    pub name: TypeName,
    pub kind: TypeDefinitionKind<R>,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
}

#[derive(Debug, Clone)]
pub enum TypeDefinitionKind<R: References> {
    Struct {
        fields: Vec<FieldDefinition<R>>,
        generic_parameters: Vec<GenericParameterDefinition<R>>,
    },
    Function {
        parameters: Vec<ParameterDefinition<R>>,
        generic_parameters: Vec<GenericParameterDefinition<R>>,
        return_type: R::TypeAnnotation,
    },
    Signature {
        entries: Vec<SignatureEntry<R>>,
    },
    Trait {
        functions: Vec<SignatureEntry<R>>,
        witness_ref: R::Witness,
    },
    Primitive,
    Native {
        generic_parameters: Vec<GenericParameterDefinition<R>>,
        factory: Arc<dyn RuntimeValueFactory>,
    },
}

#[derive(Debug, Clone)]
pub struct FieldDefinition<R: References> {
    pub name: String,
    pub type_name: R::TypeAnnotation,
}

pub fn clone_kind_typenames<R1, R2>(kind: &TypeDefinitionKind<R1>) -> TypeDefinitionKind<R2>
where
    R1: References<TypeAnnotation = TypeName>,
    R2: References<TypeAnnotation = TypeName>,
    R2::Witness: Default,
{
    match kind {
        TypeDefinitionKind::Struct {
            fields,
            generic_parameters,
        } => TypeDefinitionKind::Struct {
            fields: fields
                .iter()
                .map(|f| FieldDefinition {
                    name: f.name.clone(),
                    type_name: f.type_name.clone(),
                })
                .collect(),
            generic_parameters: generic_parameters
                .iter()
                .map(|gp| GenericParameterDefinition {
                    name: gp.name.clone(),
                    constraints: gp.constraints.clone(),
                })
                .collect(),
        },
        TypeDefinitionKind::Function {
            parameters,
            generic_parameters,
            return_type,
        } => TypeDefinitionKind::Function {
            parameters: parameters
                .iter()
                .map(|p| ParameterDefinition {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                })
                .collect(),
            generic_parameters: generic_parameters
                .iter()
                .map(|gp| GenericParameterDefinition {
                    name: gp.name.clone(),
                    constraints: gp.constraints.clone(),
                })
                .collect(),
            return_type: return_type.clone(),
        },
        TypeDefinitionKind::Signature { entries } => TypeDefinitionKind::Signature {
            entries: entries
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: e.type_name.clone(),
                })
                .collect(),
        },
        TypeDefinitionKind::Trait { functions, .. } => TypeDefinitionKind::Trait {
            functions: functions
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: e.type_name.clone(),
                })
                .collect(),
            witness_ref: R2::Witness::default(),
        },
        TypeDefinitionKind::Primitive => TypeDefinitionKind::Primitive,
        TypeDefinitionKind::Native {
            generic_parameters,
            factory,
        } => TypeDefinitionKind::Native {
            generic_parameters: generic_parameters
                .iter()
                .map(|gp| GenericParameterDefinition {
                    name: gp.name.clone(),
                    constraints: gp.constraints.clone(),
                })
                .collect(),
            factory: factory.clone(),
        },
    }
}
