use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[cfg(test)]
mod tests {
    use crate::util::calculate_hash;

    #[test]
    fn hash_test() {
        assert_eq!(2748490571820495778, calculate_hash(&6));
        assert_eq!(7392818472452443754, calculate_hash(&512));
        assert_eq!(2270354339022497229, calculate_hash(&3123));
        assert_eq!(14334111852693195205, calculate_hash(&643113));
        assert_eq!(2236313024366553744, calculate_hash(&431678));
        assert_eq!(10363793956940938451, calculate_hash(&(547635431 as u64)));
        assert_eq!(17234162834277073614, calculate_hash(&(5134687543 as u64)));
        assert_eq!(8308702756688553632, calculate_hash(&(12381298312 as u64)));
    }
}
