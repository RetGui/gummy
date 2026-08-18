//! Commonly used types

pub use crate::{
    geometry::{Line, Rect, Size},
    style::{
        AlignContent, AlignItems, AlignSelf, AvailableSpace, BoxSizing, CompactLength, Dimension, Display,
        JustifyContent, JustifyItems, JustifySelf, LengthPercentage, LengthPercentageAuto, Position, Style,
    },
    style_helpers::{
        FromFr, FromLength, FromPercent, GummyAuto, GummyFitContent, GummyMaxContent, GummyMinContent, GummyZero, auto,
        fit_content, length, max_content, min_content, percent, zero,
    },
    tree::{Layout, LayoutPartialTree, NodeId, PrintTree, RoundTree, TraversePartialTree, TraverseTree},
};

#[cfg(feature = "flexbox")]
pub use crate::style::{FlexDirection, FlexWrap};

#[cfg(feature = "grid")]
pub use crate::style::{
    GridAutoFlow, GridPlacement, GridTemplateComponent, MaxTrackSizingFunction, MinTrackSizingFunction,
    RepetitionCount, TrackSizingFunction,
};
#[cfg(feature = "grid")]
pub use crate::style_helpers::{
    GummyGridLine, GummyGridSpan, evenly_sized_tracks, flex, fr, line, minmax, repeat, span,
};

#[cfg(feature = "gummy_tree")]
pub use crate::GummyTree;
