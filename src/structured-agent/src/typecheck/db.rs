use crate::ast::Module as AstModule;
use crate::types::FileId;

#[salsa::db]
pub(super) trait TypeCheckDatabase: salsa::Database {}

#[salsa::db]
#[derive(Default)]
pub(super) struct TypeCheckDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for TypeCheckDb {}

#[salsa::db]
impl TypeCheckDatabase for TypeCheckDb {}

#[salsa::input]
pub(super) struct ParsedModuleInput {
    pub(super) name: String,
    pub(super) is_entry: bool,
    pub(super) file_id: FileId,
    pub(super) module: AstModule,
}
