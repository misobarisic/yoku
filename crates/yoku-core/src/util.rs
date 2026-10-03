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
        let value = 6_u64;
        assert_eq!(calculate_hash(&value), calculate_hash(&value));
        assert_ne!(calculate_hash(&value), calculate_hash(&512_u64));
    }
}
