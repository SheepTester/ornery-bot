use serde_json::{map::Map, Value};
use std::collections::HashMap;

pub fn serde_value_to_vec<T, F>(value: &Value, transform: F) -> Option<Vec<T>>
where
    F: FnMut(&Value) -> Option<T>,
{
    value
        .as_array()
        .and_then(|vec| Some(vec.iter().filter_map(transform).collect::<Vec<T>>()))
}

pub fn serde_map_to_hashmap<V, F>(map: &Map<String, Value>, transform: F) -> HashMap<String, V>
where
    F: Fn(&Value) -> Option<V>,
{
    let map_iter = map.iter();
    let (min_size, _) = map_iter.size_hint();
    let mut hash_map = HashMap::with_capacity(min_size);
    for (key, value) in map_iter {
        if let Some(val) = transform(value) {
            hash_map.insert(key.to_owned(), val);
        }
    }
    hash_map
}

pub fn hashmap_to_serde_object<V, F>(hash_map: &HashMap<String, V>, transform: F) -> Value
where
    F: Fn(&V) -> Value,
{
    let hash_map_iter = hash_map.iter();
    let (min_size, _) = hash_map_iter.size_hint();
    let mut map = Map::with_capacity(min_size);
    for (key, value) in hash_map_iter {
        map.insert(key.to_owned(), transform(value));
    }
    Value::Object(map)
}
