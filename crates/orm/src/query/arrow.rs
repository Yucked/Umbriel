use crate::{Render, Renderer};

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

/// Arrow for relation edge direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    /// Right arrow
    Right,
    /// Left arrow
    Left,
    /// `<->`: edges in both directions.
    Both,
}

impl Render for Arrow {
    fn render(&self, out: &mut Renderer) {
        out.push_str(match self {
            Arrow::Right => "->",
            Arrow::Left => "<-",
            Arrow::Both => "<->",
        });
    }
}
