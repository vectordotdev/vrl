use super::ValueCollection;
use crate::path::BorrowedSegment;
use crate::value::Value;

pub fn get<'a>(
    mut value: &Value,
    mut path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
) -> Option<&Value> {
    loop {
        match (path_iter.next(), value) {
            (None, _) => return Some(value),
            (Some(BorrowedSegment::Field(key)), Value::Object(map)) => {
                value = map.get_value(key.as_ref())?;
            }
            (Some(BorrowedSegment::Index(index)), Value::Array(array)) => {
                value = array.get_value(&index)?;
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::path;
    use serde_json::json;

    #[test]
    fn test_negative_index() {
        assert_eq!(
            Value::from(json!([0, 1, 2, 3]))
                .get(path!(-1_isize))
                .cloned(),
            Some(Value::from(3))
        );
        assert_eq!(
            Value::from(json!([0, 1, 2, 3]))
                .get(path!(-5_isize))
                .cloned(),
            None
        );
    }
}
