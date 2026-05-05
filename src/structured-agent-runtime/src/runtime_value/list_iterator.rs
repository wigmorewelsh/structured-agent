use std::any::Any;
use std::sync::{Arc, Mutex};

use arrow::array::{Array, ListArray};

use crate::expression::ExpressionValue;
use crate::runtime_value::{RuntimeValue, arrow_col_to_expression};

struct ListIteratorState {
    list: Arc<ListArray>,
    index: Option<usize>,
}

pub struct ListIteratorValue {
    inner: Arc<Mutex<ListIteratorState>>,
}

impl Clone for ListIteratorValue {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl std::fmt::Debug for ListIteratorValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ListIteratorValue")
    }
}

impl ListIteratorValue {
    pub fn new(list: Arc<ListArray>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ListIteratorState { list, index: None })),
        }
    }

    pub fn move_next(&self) -> bool {
        let mut state = self.inner.lock().unwrap();
        let list_len = if state.list.is_empty() {
            0
        } else {
            state.list.value(0).len()
        };
        let next = match state.index {
            None => 0,
            Some(i) => i + 1,
        };
        state.index = Some(next);
        next < list_len
    }

    pub fn current(&self) -> Option<ExpressionValue> {
        let state = self.inner.lock().unwrap();
        let index = state.index?;
        if state.list.is_empty() {
            return None;
        }
        let values = state.list.value(0);
        if index >= values.len() {
            return None;
        }
        Some(arrow_col_to_expression(values.slice(index, 1)))
    }
}

impl RuntimeValue for ListIteratorValue {
    fn type_name(&self) -> &str {
        "ListIterator"
    }

    fn format_for_llm(&self) -> String {
        "<ListIterator>".to_string()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<ListIteratorValue>()
            .map(|v| Arc::ptr_eq(&self.inner, &v.inner))
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        Arc::new(arrow::array::NullArray::new(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_value::ListValue;

    fn make_string_list(items: Vec<&str>) -> Arc<ListArray> {
        let values: Vec<ExpressionValue> =
            items.iter().map(|s| ExpressionValue::string(*s)).collect();
        let list_val = ListValue::from_elements(values).unwrap();
        list_val.list_arc()
    }

    #[test]
    fn list_iterator_type_name() {
        let iter = ListIteratorValue::new(make_string_list(vec![]));
        assert_eq!(iter.type_name(), "ListIterator");
    }

    #[test]
    fn list_iterator_move_next_on_empty_returns_false() {
        let iter = ListIteratorValue::new(make_string_list(vec![]));
        assert!(!iter.move_next());
    }

    #[test]
    fn list_iterator_move_next_returns_true_until_exhausted() {
        let iter = ListIteratorValue::new(make_string_list(vec!["a", "b"]));
        assert!(iter.move_next());
        assert!(iter.move_next());
        assert!(!iter.move_next());
    }

    #[test]
    fn list_iterator_current_before_move_next_is_none() {
        let iter = ListIteratorValue::new(make_string_list(vec!["a"]));
        assert!(iter.current().is_none());
    }

    #[test]
    fn list_iterator_current_after_exhaustion_is_none() {
        let iter = ListIteratorValue::new(make_string_list(vec!["a"]));
        iter.move_next();
        iter.move_next();
        assert!(iter.current().is_none());
    }

    #[test]
    fn list_iterator_current_returns_correct_element() {
        let iter = ListIteratorValue::new(make_string_list(vec!["x", "y", "z"]));
        iter.move_next();
        assert_eq!(iter.current().unwrap().as_string().unwrap(), "x");
        iter.move_next();
        assert_eq!(iter.current().unwrap().as_string().unwrap(), "y");
        iter.move_next();
        assert_eq!(iter.current().unwrap().as_string().unwrap(), "z");
    }

    #[test]
    fn list_iterator_iterates_all_elements() {
        let iter = ListIteratorValue::new(make_string_list(vec!["a", "b", "c"]));
        let mut results = vec![];
        while iter.move_next() {
            results.push(iter.current().unwrap().as_string().unwrap());
        }
        assert_eq!(results, vec!["a", "b", "c"]);
    }

    #[test]
    fn list_iterator_clone_shares_state() {
        let iter = ListIteratorValue::new(make_string_list(vec!["a", "b"]));
        let cloned = iter.clone();
        iter.move_next();
        assert_eq!(cloned.current().unwrap().as_string().unwrap(), "a");
    }

    #[test]
    fn list_iterator_eq_same_arc() {
        let iter = ListIteratorValue::new(make_string_list(vec![]));
        let cloned = iter.clone();
        assert!(RuntimeValue::eq(&iter, cloned.as_any()));
    }

    #[test]
    fn list_iterator_eq_different_instance_false() {
        let a = ListIteratorValue::new(make_string_list(vec![]));
        let b = ListIteratorValue::new(make_string_list(vec![]));
        assert!(!RuntimeValue::eq(&a, b.as_any()));
    }

    #[test]
    fn list_iterator_format_for_llm() {
        let iter = ListIteratorValue::new(make_string_list(vec![]));
        assert_eq!(iter.format_for_llm(), "<ListIterator>");
    }
}
