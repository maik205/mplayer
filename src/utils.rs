#[derive(Clone)]
pub struct Range {
    min: u32,
    max: u32,
}

impl std::fmt::Debug for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Range")
            .field("min", &self.min)
            .field("max", &self.max)
            .finish()
    }
}
impl Range {
    pub fn new(min: u32, max: u32) -> Range {
        Range { min, max }
    }
}
