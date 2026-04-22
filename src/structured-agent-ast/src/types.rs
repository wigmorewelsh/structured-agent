pub type FileId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn dummy() -> Self {
        Self { start: 0, end: 0 }
    }

    pub fn to_byte_range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

pub trait Spanned {
    fn span(&self) -> Span;
}
