use combine::Parser;
use combine::stream::position;
use nonempty::NonEmpty;
use std::collections::HashMap;
use std::sync::Arc;
use structured_agent::ast::ParsedModule;
use structured_agent::cli::config::ProgramSource;
use structured_agent::compiler::parser;
use structured_agent::runtime::{Context, ExpressionValue, Runtime};
use structured_agent::typecheck::TypeChecker;
use structured_agent::typecheck::TypedCheckerAstRef;
use structured_agent::typed_ast;
use structured_agent::types::{FileId, Span};
use structured_agent_stdlib::unstable::UnstableModule;

pub const TEST_FILE_ID: FileId = 0;

pub async fn run_program(source: &str) -> ExpressionValue {
    Runtime::builder(ProgramSource::Inline(source.to_string()))
        .build()
        .run()
        .await
        .expect("Program execution failed")
}

pub async fn run_program_with_unstable(source: &str) -> ExpressionValue {
    Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(UnstableModule))
        .build()
        .run()
        .await
        .expect("Program execution failed")
}

pub fn parse_and_type_check(code: &str) -> typed_ast::Module {
    let stream = position::Stream::with_positioner(code, position::IndexPositioner::default());
    let (module, _) = parser::parse_program(TEST_FILE_ID).parse(stream).unwrap();

    let parsed = ParsedModule {
        name: NonEmpty::new("test".to_string()),
        module,
        is_entry: true,
        file_id: TEST_FILE_ID,
    };
    let (typed_metadata, _) = TypeChecker::new()
        .check_modules(&[parsed], &HashMap::new())
        .unwrap();

    let definitions = typed_metadata
        .functions
        .values()
        .filter_map(|f| {
            if f.name.module.to_string() != "test" {
                return None;
            }
            if let TypedCheckerAstRef::Function(func, _) = &f.ast_ref {
                Some(typed_ast::Definition::Function((**func).clone()))
            } else {
                None
            }
        })
        .collect();

    typed_ast::Module {
        definitions,
        span: Span::dummy(),
        file_id: TEST_FILE_ID,
    }
}

pub fn make_context() -> Context {
    let runtime =
        Arc::new(Runtime::builder(ProgramSource::Inline("fn main() {}".to_string())).build());
    Context::with_runtime(runtime)
}
