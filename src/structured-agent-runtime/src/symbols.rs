use std::{collections::HashMap, fmt, sync::Arc};

use crate::runtime_value::RuntimeValueFactory;

use nonempty::NonEmpty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionKind {
    Bytecode,
    External,
}

pub trait SourceRef: fmt::Debug + Clone {}
pub trait AstRef {}
pub trait BodyRef {}
pub trait WitnessRef: fmt::Debug + Clone {}
pub trait TypeAnnotation: fmt::Debug + Clone {}

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

    fn module(&self, name: &DefinitionPath) -> Option<Arc<ModuleDefinition<Self::Refs>>>;
    fn function(&self, name: &DefinitionPath) -> Option<Arc<FunctionDefinition<Self::Refs>>>;
    fn type_def(&self, name: &DefinitionPath) -> Option<Arc<TypeDefinition<Self::Refs>>>;
    fn all_functions(&self) -> Vec<Arc<FunctionDefinition<Self::Refs>>>;
    fn all_types(&self) -> Vec<Arc<TypeDefinition<Self::Refs>>>;
    fn functions_in_module(
        &self,
        module: &DefinitionPath,
    ) -> Vec<Arc<FunctionDefinition<Self::Refs>>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefinitionPath {
    pub segments: NonEmpty<DefinitionSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DefinitionSegment {
    Root,
    Module(String),
    Function(String),
    Type(String),
    Impl { discriminator: Option<u32> },
}

impl DefinitionPath {
    pub fn root() -> Self {
        DefinitionPath {
            segments: NonEmpty::new(DefinitionSegment::Root),
        }
    }

    pub fn parent(self) -> Self {
        let segs: Vec<DefinitionSegment> = self.segments.into_iter().collect();
        if segs.len() == 1 {
            return DefinitionPath::root();
        }
        DefinitionPath {
            segments: NonEmpty::from_vec(segs[..segs.len() - 1].to_vec())
                .expect("at least one segment"),
        }
    }

    pub fn for_module(segments: NonEmpty<String>) -> Self {
        let mut segs = vec![DefinitionSegment::Root];
        segs.extend(segments.into_iter().map(DefinitionSegment::Module));
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least Root"),
        }
    }

    pub fn with_module(self, module_symbol: String) -> Self {
        let mut segs: Vec<DefinitionSegment> = self.segments.into_iter().collect();
        segs.push(DefinitionSegment::Module(module_symbol));
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        }
    }

    pub fn for_type(module: DefinitionPath, name: impl Into<String>) -> Self {
        let mut segs: Vec<DefinitionSegment> = module.segments.into_iter().collect();
        segs.push(DefinitionSegment::Type(name.into()));
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        }
    }

    pub fn with_type(self, type_symbol: String) -> Self {
        let mut segs: Vec<DefinitionSegment> = self.segments.into_iter().collect();
        segs.push(DefinitionSegment::Type(type_symbol));
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        }
    }

    pub fn for_function(module: DefinitionPath, name: impl Into<String>) -> Self {
        let mut segs: Vec<DefinitionSegment> = module.segments.into_iter().collect();
        segs.push(DefinitionSegment::Function(name.into()));
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        }
    }

    pub fn with_function(self, function_symbol: String) -> Self {
        let mut segs: Vec<DefinitionSegment> = self.segments.into_iter().collect();
        segs.push(DefinitionSegment::Function(function_symbol));
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        }
    }

    pub fn for_impl_fn(impl_key: &DefinitionPath, name: impl Into<String>) -> Self {
        let mut segs: Vec<DefinitionSegment> = impl_key.segments.iter().cloned().collect();
        segs.push(DefinitionSegment::Function(name.into()));
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        }
    }

    pub fn for_impl(module: DefinitionPath, discriminator: Option<u32>) -> Self {
        let mut segs: Vec<DefinitionSegment> = module.segments.into_iter().collect();
        segs.push(DefinitionSegment::Impl { discriminator });
        DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        }
    }

    pub fn module_prefix(&self) -> DefinitionPath {
        let segs: Vec<DefinitionSegment> = self
            .segments
            .iter()
            .take_while(|s| matches!(s, DefinitionSegment::Root | DefinitionSegment::Module(_)))
            .cloned()
            .collect();
        DefinitionPath {
            segments: NonEmpty::from_vec(segs)
                .unwrap_or_else(|| NonEmpty::new(DefinitionSegment::Root)),
        }
    }

    pub fn last_name(&self) -> &str {
        match self.segments.last() {
            DefinitionSegment::Root => "",
            DefinitionSegment::Module(n) => n,
            DefinitionSegment::Function(n) => n,
            DefinitionSegment::Type(n) => n,
            DefinitionSegment::Impl { .. } => "",
        }
    }

    pub fn is_impl_fn(&self) -> bool {
        self.segments
            .iter()
            .any(|s| matches!(s, DefinitionSegment::Impl { .. }))
    }

    pub fn impl_discriminator(&self) -> Option<u32> {
        match self.segments.last() {
            DefinitionSegment::Impl { discriminator } => *discriminator,
            _ => panic!("DefinitionPath does not end with Impl segment"),
        }
    }
}

impl TypeAnnotation for DefinitionPath {}

impl fmt::Display for DefinitionPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for seg in self.segments.iter() {
            let text = match seg {
                DefinitionSegment::Root => continue,
                DefinitionSegment::Module(n) => n.as_str(),
                DefinitionSegment::Function(n) => n.as_str(),
                DefinitionSegment::Type(n) => n.as_str(),
                DefinitionSegment::Impl { .. } => continue,
            };
            if !first {
                write!(f, "::")?;
            }
            write!(f, "{}", text)?;
            first = false;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct MetaData<R: References> {
    pub modules: HashMap<DefinitionPath, Arc<ModuleDefinition<R>>>,
    pub functions: HashMap<DefinitionPath, Arc<FunctionDefinition<R>>>,
    pub types: HashMap<DefinitionPath, Arc<TypeDefinition<R>>>,
    pub impls: HashMap<DefinitionPath, Arc<ImplDefinition<R>>>,
}

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

impl<R: References> SymbolQuery for MetaData<R> {
    type Refs = R;

    fn module(&self, name: &DefinitionPath) -> Option<Arc<ModuleDefinition<R>>> {
        self.modules.get(name).cloned()
    }

    fn function(&self, name: &DefinitionPath) -> Option<Arc<FunctionDefinition<R>>> {
        self.functions.get(name).cloned()
    }

    fn type_def(&self, name: &DefinitionPath) -> Option<Arc<TypeDefinition<R>>> {
        self.types.get(name).cloned()
    }

    fn all_functions(&self) -> Vec<Arc<FunctionDefinition<R>>> {
        self.functions.values().cloned().collect()
    }

    fn all_types(&self) -> Vec<Arc<TypeDefinition<R>>> {
        self.types.values().cloned().collect()
    }

    fn functions_in_module(&self, module: &DefinitionPath) -> Vec<Arc<FunctionDefinition<R>>> {
        self.functions
            .values()
            .filter(|f| &f.name.module_prefix() == module)
            .cloned()
            .collect()
    }
}

impl<R: References> MetaData<R> {
    pub fn register_function(&mut self, name: DefinitionPath, def: Arc<FunctionDefinition<R>>) {
        self.functions.insert(name, def);
    }

    pub fn register_type(&mut self, name: DefinitionPath, def: Arc<TypeDefinition<R>>) {
        self.types.insert(name, def);
    }
}

#[derive(Debug, Clone)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub enum ExportedName {
    Module(DefinitionPath),
    Function(DefinitionPath),
    Type(DefinitionPath),
    Trait(DefinitionPath),
}

#[derive(Debug, Clone)]
pub struct ModuleDefinition<R: References> {
    pub name: DefinitionPath,
    pub visibility: Visibility,
    pub is_entry: bool,
    pub exports: Vec<ExportedName>,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
    pub parent_module: Option<DefinitionPath>,
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition<R: References> {
    pub name: DefinitionPath,
    pub visibility: Visibility,
    pub type_name: DefinitionPath,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
    pub body_ref: Option<R::Body>,
}

#[derive(Debug, Clone)]
pub struct ParameterDefinition<R: References> {
    pub name: String,
    pub type_name: R::TypeAnnotation,
    pub source_ref: R::Source,
}

#[derive(Debug, Clone)]
pub struct GenericParameterDefinition<R: References> {
    pub name: String,
    pub constraints: Vec<R::TypeAnnotation>,
}

#[derive(Debug, Clone)]
pub struct ImplDefinition<R: References> {
    pub key: DefinitionPath,
    pub module: DefinitionPath,
    pub type_name: R::TypeAnnotation,
    pub trait_name: Option<R::TypeAnnotation>,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
}

#[derive(Debug, Clone)]
pub struct SignatureEntry {
    pub name: String,
    pub type_name: DefinitionPath,
}

#[derive(Debug, Clone)]
pub struct TypeDefinition<R: References> {
    pub name: DefinitionPath,
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
        entries: Vec<SignatureEntry>,
    },
    Trait {
        functions: Vec<SignatureEntry>,
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
    R1: References<TypeAnnotation = DefinitionPath>,
    R2: References<TypeAnnotation = DefinitionPath, Source = R1::Source>,
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
                    source_ref: p.source_ref.clone(),
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
            entries: entries.clone(),
        },
        TypeDefinitionKind::Trait { functions, .. } => TypeDefinitionKind::Trait {
            functions: functions.clone(),
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
