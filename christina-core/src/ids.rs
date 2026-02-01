/// Unique identifier for generation operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenerationId(pub u64);

impl GenerationId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_id_new() {
        let id = GenerationId::new(42);
        assert_eq!(id.0, 42);
    }

    #[test]
    fn test_generation_id_traits() {
        let id1 = GenerationId::new(123);
        let id2 = GenerationId::new(123);
        let id3 = GenerationId::new(456);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);

        let id_copy = id1;
        assert_eq!(id_copy, id1);

        let debug_str = format!("{:?}", id1);
        assert!(debug_str.contains("GenerationId"));

        let mut set = std::collections::HashSet::new();
        set.insert(id1);
        set.insert(id2);
        assert_eq!(set.len(), 1);
    }
}
