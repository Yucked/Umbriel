use crate::keywords::keywords;

/// Direction of the connection edge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    /// Outgoing edge
    Out,
    /// Incoming edge
    In,
}

impl EdgeDirection {
    /// Get the arrow symbol for the edge direction
    pub fn as_arrow_symbol(&self) -> Arrow {
        match self {
            EdgeDirection::Out => Arrow::Right,
            EdgeDirection::In => Arrow::Left,
        }
    }
}

keywords! {
    /// Arrow for relation edge direction
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Arrow: render {
        /// Right arrow
        Right => "->",
        /// Left arrow
        Left => "<-",
        /// `<->`: edges in both directions.
        Both => "<->",
    }
}
