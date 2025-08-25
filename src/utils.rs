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

    pub fn range_check_inclusive(&self, num: u32) -> RangeCheck {
        if num > self.max {return RangeCheck::Higher}
        if num < self.min {return RangeCheck::Lower}
        RangeCheck::InRange
    }

    pub fn range_check(&self, num: u32) -> RangeCheck {
        if num >= self.max {return RangeCheck::Higher}
        if num <= self.min {return RangeCheck::Lower}
        RangeCheck::InRange
    }
}


pub enum RangeCheck {
    Lower,
    InRange,
    Higher
}