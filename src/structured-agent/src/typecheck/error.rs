pub use structured_agent_runtime::TypeError;
use salsa::Accumulator;

#[salsa::accumulator]
pub struct TypeErrorAccumulator(pub TypeError);

pub trait OrAccumulateError<T> {
    fn or_accumulate<Db: ?Sized + salsa::Database>(self, db: &Db, error: TypeError) -> Option<T>;
}

impl<T> OrAccumulateError<T> for Option<T> {
    fn or_accumulate<Db: ?Sized + salsa::Database>(self, db: &Db, error: TypeError) -> Option<T> {
        self.or_else(|| {
            TypeErrorAccumulator(error).accumulate(db);
            None
        })
    }
}

#[macro_export]
macro_rules! ensure_or_accumulate {
    ($condition:expr, $db:expr, $error:expr) => {
        if !($condition) {
            $crate::typecheck::error::TypeErrorAccumulator($error).accumulate($db);
            return None;
        }
    };
}
