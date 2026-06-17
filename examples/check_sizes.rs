use std::mem;
use vrl::value::{KeyString, ObjectMap, Value};

fn main() {
    println!("ObjectMap:    {} bytes", mem::size_of::<ObjectMap>());
    println!("Value:        {} bytes", mem::size_of::<Value>());
    println!(
        "Vec<(K,V)>:   {} bytes",
        mem::size_of::<Vec<(KeyString, Value)>>()
    );
    println!(
        "BTreeMap:     {} bytes",
        mem::size_of::<std::collections::BTreeMap<KeyString, Value>>()
    );
    println!(
        "HashMap:      {} bytes",
        mem::size_of::<std::collections::HashMap<KeyString, Value>>()
    );
    println!(
        "IndexMap:     {} bytes",
        mem::size_of::<indexmap::IndexMap<KeyString, Value>>()
    );
}
