/// A macro to easily create a `BTreeMap`
#[macro_export]
macro_rules! btreemap {
    () => (::std::collections::BTreeMap::new());

    // trailing comma case
    ($($key:expr_2021 => $value:expr_2021,)+) => (btreemap!($($key => $value),+));

    ($($key:expr_2021 => $value:expr_2021),*) => {
        ::std::collections::BTreeMap::from([
            $(
                ($key.into(), $value.into()),
            )*
        ])
    };
}

/// A macro to easily create an `ObjectMap`
#[macro_export]
macro_rules! objectmap {
    () => ($crate::value::ObjectMap::new());

    // trailing comma case
    ($($key:expr_2021 => $value:expr_2021,)+) => (objectmap!($($key => $value),+));

    ($($key:expr_2021 => $value:expr_2021),*) => {
        $crate::value::ObjectMap::from([
            $(
                ($key.into(), $value.into()),
            )*
        ])
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_btreemap() {
        use std::collections::BTreeMap;

        assert_eq!(btreemap! {}, BTreeMap::<usize, usize>::new());

        let mut map = BTreeMap::new();
        map.insert(1, "1");
        assert_eq!(btreemap! { 1 => "1" }, map);

        let mut map = BTreeMap::new();
        map.insert("1", "one");
        map.insert("2", "two");
        assert_eq!(btreemap! { "1" => "one", "2" => "two" }, map);
    }

    #[test]
    fn test_objectmap() {
        use crate::value::ObjectMap;

        assert_eq!(objectmap! {}, ObjectMap::new());

        let mut map = ObjectMap::new();
        map.insert("key".into(), "value".into());
        assert_eq!(objectmap! { "key" => "value" }, map);
    }
}
