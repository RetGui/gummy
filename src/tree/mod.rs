//! Contains both a high-level interface to Gummy using a ready-made node tree, and a set of traits for defining custom node trees.
//!
//! - For documentation on the high-level API, see the [`GummyTree`] struct.
//! - For documentation on the low-level trait-based API, see the [`traits`] module.

// Submodules
mod cache;
mod layout;
mod node;
pub mod traits;

pub use cache::{Cache, ClearState};
pub use layout::{CollapsibleMarginSet, Layout, LayoutInput, LayoutOutput, RequestedAxis, RunMode, SizingMode};
pub use node::NodeId;
pub(crate) use traits::LayoutPartialTreeExt;
pub use traits::{LayoutPartialTree, PrintTree, RoundTree, TraversePartialTree, TraverseTree};

#[cfg(feature = "flexbox")]
pub use traits::LayoutFlexboxContainer;

#[cfg(feature = "grid")]
pub use traits::LayoutGridContainer;

#[cfg(feature = "block_layout")]
pub use traits::LayoutBlockContainer;

#[cfg(feature = "gummy_tree")]
mod gummy_tree;
#[cfg(feature = "gummy_tree")]
pub use gummy_tree::{GummyError, GummyResult, GummyTree};

#[cfg(feature = "detailed_layout_info")]
pub use layout::DetailedLayoutInfo;
