/// Unique identifier for generation operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenerationId(pub u64);

impl GenerationId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}
