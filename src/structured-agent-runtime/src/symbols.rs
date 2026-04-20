use std::{collections::HashMap, fmt, sync::Arc};

use crate::runtime_value::RuntimeValueFactory;

use nonempty::NonEmpty;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefinitionPath {
    pub segments: NonEmpty<DefinitionSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DefinitionSegment {
    Module(String),
    Function(String),
    Type(String),
    Impl { discriminator: Option<u32> },
}

impl DefinitionPath {
    pub fn from_module_strings(segments: NonEmpty<String>) -> Self {
        DefinitionPath {
            segments: segments.map(DefinitionSegment::Module),
        }
    }
}

fn module_prefix_of(path: &DefinitionPath) -> ModuleName {
    let segs: Vec<String> = path
        .segments
        .iter()
        .take_while(|s| matches!(s, DefinitionSegment::Module(_)))
        .map(|s| match s {
            DefinitionSegment::Module(n) => n.clone(),
            _ => unreachable!(),
        })
        .collect();
    ModuleName::new(NonEmpty::from_vec(segs).unwrap_or_else(|| NonEmpty::new(String::new())))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleName(pub DefinitionPath);

impl ModuleName {
    pub fn new(segments: NonEmpty<String>) -> Self {
        ModuleName(DefinitionPath::from_module_strings(segments))
    }

    pub fn last_segment(&self) -> &str {
        match self.0.segments.last() {
            DefinitionSegment::Module(n) => n,
            _ => panic!("ModuleName path contains non-Module segment"),
        }
    }

    #[deprecated(note = "stop using magic strings for module names")]
    pub fn from_str(s: &str) -> Self {
        let v: Vec<String> = s.split("::").map(|p| p.to_string()).collect();
        ModuleName::new(NonEmpty::from_vec(v).expect("split always yields at least one element"))
    }

    #[deprecated(note = "transition marker: replace with the real module name")]
    pub fn unqualified() -> Self {
        ModuleName::new(NonEmpty::new(String::new()))
    }
}

impl fmt::Display for ModuleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for seg in self.0.segments.iter() {
            if let DefinitionSegment::Module(n) = seg {
                if !first {
                    write!(f, "::")?;
                }
                write!(f, "{}", n)?;
                first = false;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionName(pub DefinitionPath);

impl FunctionName {
    pub fn new(module: ModuleName, name: impl Into<String>) -> Self {
        let mut segs: Vec<DefinitionSegment> = module.0.segments.into_iter().collect();
        segs.push(DefinitionSegment::Function(name.into()));
        FunctionName(DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        })
    }

    pub fn new_impl(
        module: ModuleName,
        type_name: impl Into<String>,
        trait_name: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        let mut segs: Vec<DefinitionSegment> = module.0.segments.into_iter().collect();
        segs.push(DefinitionSegment::Type(type_name.into()));
        segs.push(DefinitionSegment::Type(trait_name.into()));
        segs.push(DefinitionSegment::Impl {
            discriminator: None,
        });
        segs.push(DefinitionSegment::Function(name.into()));
        FunctionName(DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        })
    }

    pub fn name(&self) -> &str {
        match self.0.segments.last() {
            DefinitionSegment::Function(n) => n,
            _ => panic!("FunctionName path does not end with Function segment"),
        }
    }

    pub fn module(&self) -> ModuleName {
        module_prefix_of(&self.0)
    }

    pub fn is_impl_fn(&self) -> bool {
        self.0
            .segments
            .iter()
            .any(|s| matches!(s, DefinitionSegment::Impl { .. }))
    }

    #[deprecated(note = "use .name() directly")]
    pub fn fn_name(&self) -> &str {
        self.name()
    }
}

impl fmt::Display for FunctionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for seg in self.0.segments.iter() {
            let text = match seg {
                DefinitionSegment::Module(n) if n.is_empty() => continue,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeName(pub DefinitionPath);

impl TypeAnnotation for TypeName {}

impl TypeName {
    pub fn new(module: ModuleName, name: impl Into<String>) -> Self {
        let mut segs: Vec<DefinitionSegment> = module.0.segments.into_iter().collect();
        segs.push(DefinitionSegment::Type(name.into()));
        TypeName(DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        })
    }

    pub fn name(&self) -> &str {
        match self.0.segments.last() {
            DefinitionSegment::Type(n) => n,
            _ => panic!("TypeName path does not end with Type segment"),
        }
    }

    pub fn module(&self) -> ModuleName {
        module_prefix_of(&self.0)
    }
}

impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.module(), self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImplKey(pub DefinitionPath);

impl ImplKey {
    pub fn new(
        module: ModuleName,
        type_name: impl Into<String>,
        trait_name: impl Into<String>,
    ) -> Self {
        let mut segs: Vec<DefinitionSegment> = module.0.segments.into_iter().collect();
        segs.push(DefinitionSegment::Type(type_name.into()));
        segs.push(DefinitionSegment::Type(trait_name.into()));
        segs.push(DefinitionSegment::Impl {
            discriminator: None,
        });
        ImplKey(DefinitionPath {
            segments: NonEmpty::from_vec(segs).expect("at least one segment"),
        })
    }

    pub fn type_name(&self) -> &str {
        let segs: Vec<&DefinitionSegment> = self.0.segments.iter().collect();
        let impl_pos = segs
            .iter()
            .rposition(|s| matches!(s, DefinitionSegment::Impl { .. }))
            .expect("ImplKey must contain an Impl segment");
        match segs[impl_pos - 2] {
            DefinitionSegment::Type(n) => n,
            _ => panic!("ImplKey: expected Type segment for type_name"),
        }
    }

    pub fn trait_name(&self) -> &str {
        let segs: Vec<&DefinitionSegment> = self.0.segments.iter().collect();
        let impl_pos = segs
            .iter()
            .rposition(|s| matches!(s, DefinitionSegment::Impl { .. }))
            .expect("ImplKey must contain an Impl segment");
        match segs[impl_pos - 1] {
            DefinitionSegment::Type(n) => n,
            _ => panic!("ImplKey: expected Type segment for trait_name"),
        }
    }

    pub fn impl_module(&self) -> ModuleName {
        module_prefix_of(&self.0)
    }
}

#[derive(Clone)]
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
            .find(|(k, _)| k.type_name() == type_name.name() && k.trait_name() == trait_name.name())
            .map(|(_, v)| v.clone())
    }

    fn traits_implemented_by(&self, type_name: &TypeName) -> Vec<Arc<ImplDefinition<R>>> {
        self.impls
            .iter()
            .filter(|(k, _)| k.type_name() == type_name.name())
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
            .filter(|f| &f.name.module() == module)
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
    pub parent_module: Option<ModuleName>,
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
    pub source_ref: R::Source,
}

#[derive(Debug, Clone)]
pub struct GenericParameterDefinition<R: References> {
    pub name: String,
    pub constraints: Vec<R::TypeAnnotation>,
}

#[derive(Debug, Clone)]
pub struct ImplDefinition<R: References> {
    pub key: ImplKey,
    pub module: ModuleName,
    pub source_ref: R::Source,
    pub ast_ref: R::Ast,
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
    R2: References<TypeAnnotation = TypeName, Source = R1::Source>,
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
