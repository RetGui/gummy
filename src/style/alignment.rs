//! Style types for controlling alignment.

#[cfg(feature = "parse")]
use crate::util::parse::{CssParseResult, FromCss, Parser, Token};

use crate::style::Direction;

/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-items)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum AlignItems {
    /// Uses the layout mode's normal alignment.
    #[default]
    Normal,
    /// Items are packed toward the start of the axis.
    Start,
    /// Items are packed toward the end of the axis.
    End,
    /// Items are packed toward the flex-relative start of the axis.
    FlexStart,
    /// Items are packed toward the flex-relative end of the axis.
    FlexEnd,
    /// Items are packed toward the start of the axis according to their own writing mode.
    SelfStart,
    /// Items are packed toward the end of the axis according to their own writing mode.
    SelfEnd,
    /// Items are packed around the center of the axis.
    Center,
    /// Items are aligned so their baselines align.
    Baseline,
    /// Items are stretched to fill the container.
    Stretch,
    /// Safe `start` alignment.
    SafeStart,
    /// Safe `end` alignment.
    SafeEnd,
    /// Safe `flex-start` alignment.
    SafeFlexStart,
    /// Safe `flex-end` alignment.
    SafeFlexEnd,
    /// Safe `self-start` alignment.
    SafeSelfStart,
    /// Safe `self-end` alignment.
    SafeSelfEnd,
    /// Safe `center` alignment.
    SafeCenter,
}

impl AlignItems {
    /// Uses the layout mode's normal alignment.
    pub const NORMAL: Self = Self::Normal;
    /// Items are packed toward the start of the axis.
    pub const START: Self = Self::Start;
    /// Items are packed toward the end of the axis.
    pub const END: Self = Self::End;
    /// Items are packed toward the flex-relative start of the axis.
    pub const FLEX_START: Self = Self::FlexStart;
    /// Items are packed toward the flex-relative end of the axis.
    pub const FLEX_END: Self = Self::FlexEnd;
    /// Items are packed toward their self-relative start edge.
    pub const SELF_START: Self = Self::SelfStart;
    /// Items are packed toward their self-relative end edge.
    pub const SELF_END: Self = Self::SelfEnd;
    /// Items are packed around the center of the axis.
    pub const CENTER: Self = Self::Center;
    /// Items are aligned so their baselines align.
    pub const BASELINE: Self = Self::Baseline;
    /// Items are stretched to fill the container.
    pub const STRETCH: Self = Self::Stretch;
    /// Safe `start` alignment.
    pub const SAFE_START: Self = Self::SafeStart;
    /// Safe `end` alignment.
    pub const SAFE_END: Self = Self::SafeEnd;
    /// Safe `flex-start` alignment.
    pub const SAFE_FLEX_START: Self = Self::SafeFlexStart;
    /// Safe `flex-end` alignment.
    pub const SAFE_FLEX_END: Self = Self::SafeFlexEnd;
    /// Safe `self-start` alignment.
    pub const SAFE_SELF_START: Self = Self::SafeSelfStart;
    /// Safe `self-end` alignment.
    pub const SAFE_SELF_END: Self = Self::SafeSelfEnd;
    /// Safe `center` alignment.
    pub const SAFE_CENTER: Self = Self::SafeCenter;

    /// Returns `true` if this value carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(
            self,
            Self::SafeStart
                | Self::SafeEnd
                | Self::SafeFlexStart
                | Self::SafeFlexEnd
                | Self::SafeSelfStart
                | Self::SafeSelfEnd
                | Self::SafeCenter
        )
    }
}

/// Controls alignment of an individual node in the cross/block axis.
///
/// Overrides the parent node's [`AlignItems`] property.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-self)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum AlignSelf {
    /// Uses the parent node's [`AlignItems`] value.
    #[default]
    Auto,
    /// Uses the layout mode's normal alignment.
    Normal,
    /// The item is packed toward the start of the axis.
    Start,
    /// The item is packed toward the end of the axis.
    End,
    /// The item is packed toward the flex-relative start of the axis.
    FlexStart,
    /// The item is packed toward the flex-relative end of the axis.
    FlexEnd,
    /// The item is packed toward its self-relative start edge.
    SelfStart,
    /// The item is packed toward its self-relative end edge.
    SelfEnd,
    /// The item is packed around the center of the axis.
    Center,
    /// The item is aligned by its baseline.
    Baseline,
    /// The item is stretched to fill its alignment container.
    Stretch,
    /// Safe `start` alignment.
    SafeStart,
    /// Safe `end` alignment.
    SafeEnd,
    /// Safe `flex-start` alignment.
    SafeFlexStart,
    /// Safe `flex-end` alignment.
    SafeFlexEnd,
    /// Safe `self-start` alignment.
    SafeSelfStart,
    /// Safe `self-end` alignment.
    SafeSelfEnd,
    /// Safe `center` alignment.
    SafeCenter,
}

impl AlignSelf {
    /// Uses the parent node's [`AlignItems`] value.
    pub const AUTO: Self = Self::Auto;
    /// Uses the layout mode's normal alignment.
    pub const NORMAL: Self = Self::Normal;
    /// The item is packed toward the start of the axis.
    pub const START: Self = Self::Start;
    /// The item is packed toward the end of the axis.
    pub const END: Self = Self::End;
    /// The item is packed toward the flex-relative start of the axis.
    pub const FLEX_START: Self = Self::FlexStart;
    /// The item is packed toward the flex-relative end of the axis.
    pub const FLEX_END: Self = Self::FlexEnd;
    /// The item is packed toward its self-relative start edge.
    pub const SELF_START: Self = Self::SelfStart;
    /// The item is packed toward its self-relative end edge.
    pub const SELF_END: Self = Self::SelfEnd;
    /// The item is packed around the center of the axis.
    pub const CENTER: Self = Self::Center;
    /// The item is aligned by its baseline.
    pub const BASELINE: Self = Self::Baseline;
    /// The item is stretched to fill its alignment container.
    pub const STRETCH: Self = Self::Stretch;
    /// Safe `start` alignment.
    pub const SAFE_START: Self = Self::SafeStart;
    /// Safe `end` alignment.
    pub const SAFE_END: Self = Self::SafeEnd;
    /// Safe `flex-start` alignment.
    pub const SAFE_FLEX_START: Self = Self::SafeFlexStart;
    /// Safe `flex-end` alignment.
    pub const SAFE_FLEX_END: Self = Self::SafeFlexEnd;
    /// Safe `self-start` alignment.
    pub const SAFE_SELF_START: Self = Self::SafeSelfStart;
    /// Safe `self-end` alignment.
    pub const SAFE_SELF_END: Self = Self::SafeSelfEnd;
    /// Safe `center` alignment.
    pub const SAFE_CENTER: Self = Self::SafeCenter;

    /// Returns `true` if this value carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(
            self,
            Self::SafeStart
                | Self::SafeEnd
                | Self::SafeFlexStart
                | Self::SafeFlexEnd
                | Self::SafeSelfStart
                | Self::SafeSelfEnd
                | Self::SafeCenter
        )
    }

    /// Removes the `safe` overflow-position modifier.
    #[inline]
    pub(crate) const fn unsafe_variant(self) -> Self {
        match self {
            Self::SafeStart => Self::Start,
            Self::SafeEnd => Self::End,
            Self::SafeFlexStart => Self::FlexStart,
            Self::SafeFlexEnd => Self::FlexEnd,
            Self::SafeSelfStart => Self::SelfStart,
            Self::SafeSelfEnd => Self::SelfEnd,
            Self::SafeCenter => Self::Center,
            value => value,
        }
    }

    /// Resolves `self-start` and `self-end` against the item's own direction.
    #[inline]
    pub(crate) fn resolve_self_relative(
        self,
        item_direction: Direction,
        container_direction: Direction,
        axis_is_inline: bool,
    ) -> Self {
        let flip = axis_is_inline && item_direction != container_direction;
        match (self, flip) {
            (Self::SelfStart, false) => Self::Start,
            (Self::SelfStart, true) => Self::End,
            (Self::SelfEnd, false) => Self::End,
            (Self::SelfEnd, true) => Self::Start,
            (Self::SafeSelfStart, false) => Self::SafeStart,
            (Self::SafeSelfStart, true) => Self::SafeEnd,
            (Self::SafeSelfEnd, false) => Self::SafeEnd,
            (Self::SafeSelfEnd, true) => Self::SafeStart,
            (value, _) => value,
        }
    }
}

impl From<AlignItems> for AlignSelf {
    fn from(value: AlignItems) -> Self {
        match value {
            AlignItems::Normal => Self::Normal,
            AlignItems::Start => Self::Start,
            AlignItems::End => Self::End,
            AlignItems::FlexStart => Self::FlexStart,
            AlignItems::FlexEnd => Self::FlexEnd,
            AlignItems::SelfStart => Self::SelfStart,
            AlignItems::SelfEnd => Self::SelfEnd,
            AlignItems::Center => Self::Center,
            AlignItems::Baseline => Self::Baseline,
            AlignItems::Stretch => Self::Stretch,
            AlignItems::SafeStart => Self::SafeStart,
            AlignItems::SafeEnd => Self::SafeEnd,
            AlignItems::SafeFlexStart => Self::SafeFlexStart,
            AlignItems::SafeFlexEnd => Self::SafeFlexEnd,
            AlignItems::SafeSelfStart => Self::SafeSelfStart,
            AlignItems::SafeSelfEnd => Self::SafeSelfEnd,
            AlignItems::SafeCenter => Self::SafeCenter,
        }
    }
}

/// Controls how child nodes are aligned in the inline axis.
///
/// This does not apply to Flexbox.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-items)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum JustifyItems {
    /// Uses the layout mode's normal alignment.
    #[default]
    Normal,
    /// Items are packed toward the start of the axis.
    Start,
    /// Items are packed toward the end of the axis.
    End,
    /// Items are packed toward the flex-relative start of the axis.
    FlexStart,
    /// Items are packed toward the flex-relative end of the axis.
    FlexEnd,
    /// Items are packed toward their self-relative start edge.
    SelfStart,
    /// Items are packed toward their self-relative end edge.
    SelfEnd,
    /// Items are packed around the center of the axis.
    Center,
    /// Items are aligned so their baselines align.
    Baseline,
    /// Items are stretched to fill the container.
    Stretch,
    /// Safe `start` alignment.
    SafeStart,
    /// Safe `end` alignment.
    SafeEnd,
    /// Safe `flex-start` alignment.
    SafeFlexStart,
    /// Safe `flex-end` alignment.
    SafeFlexEnd,
    /// Safe `self-start` alignment.
    SafeSelfStart,
    /// Safe `self-end` alignment.
    SafeSelfEnd,
    /// Safe `center` alignment.
    SafeCenter,
}

impl JustifyItems {
    /// Uses the layout mode's normal alignment.
    pub const NORMAL: Self = Self::Normal;
    /// Items are packed toward the start of the axis.
    pub const START: Self = Self::Start;
    /// Items are packed toward the end of the axis.
    pub const END: Self = Self::End;
    /// Items are packed toward the flex-relative start of the axis.
    pub const FLEX_START: Self = Self::FlexStart;
    /// Items are packed toward the flex-relative end of the axis.
    pub const FLEX_END: Self = Self::FlexEnd;
    /// Items are packed toward their self-relative start edge.
    pub const SELF_START: Self = Self::SelfStart;
    /// Items are packed toward their self-relative end edge.
    pub const SELF_END: Self = Self::SelfEnd;
    /// Items are packed around the center of the axis.
    pub const CENTER: Self = Self::Center;
    /// Items are aligned so their baselines align.
    pub const BASELINE: Self = Self::Baseline;
    /// Items are stretched to fill the container.
    pub const STRETCH: Self = Self::Stretch;
    /// Safe `start` alignment.
    pub const SAFE_START: Self = Self::SafeStart;
    /// Safe `end` alignment.
    pub const SAFE_END: Self = Self::SafeEnd;
    /// Safe `flex-start` alignment.
    pub const SAFE_FLEX_START: Self = Self::SafeFlexStart;
    /// Safe `flex-end` alignment.
    pub const SAFE_FLEX_END: Self = Self::SafeFlexEnd;
    /// Safe `self-start` alignment.
    pub const SAFE_SELF_START: Self = Self::SafeSelfStart;
    /// Safe `self-end` alignment.
    pub const SAFE_SELF_END: Self = Self::SafeSelfEnd;
    /// Safe `center` alignment.
    pub const SAFE_CENTER: Self = Self::SafeCenter;

    /// Returns `true` if this value carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(
            self,
            Self::SafeStart
                | Self::SafeEnd
                | Self::SafeFlexStart
                | Self::SafeFlexEnd
                | Self::SafeSelfStart
                | Self::SafeSelfEnd
                | Self::SafeCenter
        )
    }
}

/// Controls alignment of an individual node in the inline axis.
///
/// Overrides the parent node's [`JustifyItems`] property.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-self)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum JustifySelf {
    /// Uses the parent node's [`JustifyItems`] value.
    #[default]
    Auto,
    /// Uses the layout mode's normal alignment.
    Normal,
    /// The item is packed toward the start of the axis.
    Start,
    /// The item is packed toward the end of the axis.
    End,
    /// The item is packed toward the flex-relative start of the axis.
    FlexStart,
    /// The item is packed toward the flex-relative end of the axis.
    FlexEnd,
    /// The item is packed toward its self-relative start edge.
    SelfStart,
    /// The item is packed toward its self-relative end edge.
    SelfEnd,
    /// The item is packed around the center of the axis.
    Center,
    /// The item is aligned by its baseline.
    Baseline,
    /// The item is stretched to fill its alignment container.
    Stretch,
    /// Safe `start` alignment.
    SafeStart,
    /// Safe `end` alignment.
    SafeEnd,
    /// Safe `flex-start` alignment.
    SafeFlexStart,
    /// Safe `flex-end` alignment.
    SafeFlexEnd,
    /// Safe `self-start` alignment.
    SafeSelfStart,
    /// Safe `self-end` alignment.
    SafeSelfEnd,
    /// Safe `center` alignment.
    SafeCenter,
}

impl JustifySelf {
    /// Uses the parent node's [`JustifyItems`] value.
    pub const AUTO: Self = Self::Auto;
    /// Uses the layout mode's normal alignment.
    pub const NORMAL: Self = Self::Normal;
    /// The item is packed toward the start of the axis.
    pub const START: Self = Self::Start;
    /// The item is packed toward the end of the axis.
    pub const END: Self = Self::End;
    /// The item is packed toward the flex-relative start of the axis.
    pub const FLEX_START: Self = Self::FlexStart;
    /// The item is packed toward the flex-relative end of the axis.
    pub const FLEX_END: Self = Self::FlexEnd;
    /// The item is packed toward its self-relative start edge.
    pub const SELF_START: Self = Self::SelfStart;
    /// The item is packed toward its self-relative end edge.
    pub const SELF_END: Self = Self::SelfEnd;
    /// The item is packed around the center of the axis.
    pub const CENTER: Self = Self::Center;
    /// The item is aligned by its baseline.
    pub const BASELINE: Self = Self::Baseline;
    /// The item is stretched to fill its alignment container.
    pub const STRETCH: Self = Self::Stretch;
    /// Safe `start` alignment.
    pub const SAFE_START: Self = Self::SafeStart;
    /// Safe `end` alignment.
    pub const SAFE_END: Self = Self::SafeEnd;
    /// Safe `flex-start` alignment.
    pub const SAFE_FLEX_START: Self = Self::SafeFlexStart;
    /// Safe `flex-end` alignment.
    pub const SAFE_FLEX_END: Self = Self::SafeFlexEnd;
    /// Safe `self-start` alignment.
    pub const SAFE_SELF_START: Self = Self::SafeSelfStart;
    /// Safe `self-end` alignment.
    pub const SAFE_SELF_END: Self = Self::SafeSelfEnd;
    /// Safe `center` alignment.
    pub const SAFE_CENTER: Self = Self::SafeCenter;

    /// Returns `true` if this value carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(
            self,
            Self::SafeStart
                | Self::SafeEnd
                | Self::SafeFlexStart
                | Self::SafeFlexEnd
                | Self::SafeSelfStart
                | Self::SafeSelfEnd
                | Self::SafeCenter
        )
    }

    /// Removes the `safe` overflow-position modifier.
    #[inline]
    pub(crate) const fn unsafe_variant(self) -> Self {
        match self {
            Self::SafeStart => Self::Start,
            Self::SafeEnd => Self::End,
            Self::SafeFlexStart => Self::FlexStart,
            Self::SafeFlexEnd => Self::FlexEnd,
            Self::SafeSelfStart => Self::SelfStart,
            Self::SafeSelfEnd => Self::SelfEnd,
            Self::SafeCenter => Self::Center,
            value => value,
        }
    }

    /// Resolves `self-start` and `self-end` against the item's own direction.
    #[inline]
    pub(crate) fn resolve_self_relative(
        self,
        item_direction: Direction,
        container_direction: Direction,
        axis_is_inline: bool,
    ) -> Self {
        let flip = axis_is_inline && item_direction != container_direction;
        match (self, flip) {
            (Self::SelfStart, false) => Self::Start,
            (Self::SelfStart, true) => Self::End,
            (Self::SelfEnd, false) => Self::End,
            (Self::SelfEnd, true) => Self::Start,
            (Self::SafeSelfStart, false) => Self::SafeStart,
            (Self::SafeSelfStart, true) => Self::SafeEnd,
            (Self::SafeSelfEnd, false) => Self::SafeEnd,
            (Self::SafeSelfEnd, true) => Self::SafeStart,
            (value, _) => value,
        }
    }
}

impl From<JustifyItems> for JustifySelf {
    fn from(value: JustifyItems) -> Self {
        match value {
            JustifyItems::Normal => Self::Normal,
            JustifyItems::Start => Self::Start,
            JustifyItems::End => Self::End,
            JustifyItems::FlexStart => Self::FlexStart,
            JustifyItems::FlexEnd => Self::FlexEnd,
            JustifyItems::SelfStart => Self::SelfStart,
            JustifyItems::SelfEnd => Self::SelfEnd,
            JustifyItems::Center => Self::Center,
            JustifyItems::Baseline => Self::Baseline,
            JustifyItems::Stretch => Self::Stretch,
            JustifyItems::SafeStart => Self::SafeStart,
            JustifyItems::SafeEnd => Self::SafeEnd,
            JustifyItems::SafeFlexStart => Self::SafeFlexStart,
            JustifyItems::SafeFlexEnd => Self::SafeFlexEnd,
            JustifyItems::SafeSelfStart => Self::SafeSelfStart,
            JustifyItems::SafeSelfEnd => Self::SafeSelfEnd,
            JustifyItems::SafeCenter => Self::SafeCenter,
        }
    }
}

/// Sets the distribution of space between and around content items in the block axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-content)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum AlignContent {
    /// Uses the layout mode's normal alignment.
    #[default]
    Normal,
    /// Content is packed toward the start of the axis.
    Start,
    /// Content is packed toward the end of the axis.
    End,
    /// Content is packed toward the flex-relative start of the axis.
    FlexStart,
    /// Content is packed toward the flex-relative end of the axis.
    FlexEnd,
    /// Content is centered within the container.
    Center,
    /// Content is stretched to fill the container.
    Stretch,
    /// The first and last items are flush with the container edges.
    SpaceBetween,
    /// The gaps before, between, and after items are equal.
    SpaceEvenly,
    /// The outer gaps are half the size of the gaps between items.
    SpaceAround,
    /// Safe `start` alignment.
    SafeStart,
    /// Safe `end` alignment.
    SafeEnd,
    /// Safe `flex-start` alignment.
    SafeFlexStart,
    /// Safe `flex-end` alignment.
    SafeFlexEnd,
    /// Safe `center` alignment.
    SafeCenter,
}

impl AlignContent {
    /// Uses the layout mode's normal alignment.
    pub const NORMAL: Self = Self::Normal;
    /// Content is packed toward the start of the axis.
    pub const START: Self = Self::Start;
    /// Content is packed toward the end of the axis.
    pub const END: Self = Self::End;
    /// Content is packed toward the flex-relative start of the axis.
    pub const FLEX_START: Self = Self::FlexStart;
    /// Content is packed toward the flex-relative end of the axis.
    pub const FLEX_END: Self = Self::FlexEnd;
    /// Content is centered within the container.
    pub const CENTER: Self = Self::Center;
    /// Content is stretched to fill the container.
    pub const STRETCH: Self = Self::Stretch;
    /// The first and last items are flush with the container edges.
    pub const SPACE_BETWEEN: Self = Self::SpaceBetween;
    /// The gaps before, between, and after items are equal.
    pub const SPACE_EVENLY: Self = Self::SpaceEvenly;
    /// The outer gaps are half the size of the gaps between items.
    pub const SPACE_AROUND: Self = Self::SpaceAround;
    /// Safe `start` alignment.
    pub const SAFE_START: Self = Self::SafeStart;
    /// Safe `end` alignment.
    pub const SAFE_END: Self = Self::SafeEnd;
    /// Safe `flex-start` alignment.
    pub const SAFE_FLEX_START: Self = Self::SafeFlexStart;
    /// Safe `flex-end` alignment.
    pub const SAFE_FLEX_END: Self = Self::SafeFlexEnd;
    /// Safe `center` alignment.
    pub const SAFE_CENTER: Self = Self::SafeCenter;

    /// Returns `true` if this value carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(self, Self::SafeStart | Self::SafeEnd | Self::SafeFlexStart | Self::SafeFlexEnd | Self::SafeCenter)
    }

    /// Removes the `safe` overflow-position modifier.
    #[inline]
    pub(crate) const fn unsafe_variant(self) -> Self {
        match self {
            Self::SafeStart => Self::Start,
            Self::SafeEnd => Self::End,
            Self::SafeFlexStart => Self::FlexStart,
            Self::SafeFlexEnd => Self::FlexEnd,
            Self::SafeCenter => Self::Center,
            value => value,
        }
    }

    /// Returns the reversed alignment for right-to-left contexts.
    #[inline]
    pub(crate) const fn reversed(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
            Self::FlexStart => Self::FlexEnd,
            Self::FlexEnd => Self::FlexStart,
            Self::SafeStart => Self::SafeEnd,
            Self::SafeEnd => Self::SafeStart,
            Self::SafeFlexStart => Self::SafeFlexEnd,
            Self::SafeFlexEnd => Self::SafeFlexStart,
            Self::Stretch => Self::End,
            value => value,
        }
    }
}

/// Sets the distribution of space between and around content items in the inline axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-content)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum JustifyContent {
    /// Uses the layout mode's normal alignment.
    #[default]
    Normal,
    /// Content is packed toward the start of the axis.
    Start,
    /// Content is packed toward the end of the axis.
    End,
    /// Content is packed toward the flex-relative start of the axis.
    FlexStart,
    /// Content is packed toward the flex-relative end of the axis.
    FlexEnd,
    /// Content is centered within the container.
    Center,
    /// Content is stretched to fill the container.
    Stretch,
    /// The first and last items are flush with the container edges.
    SpaceBetween,
    /// The gaps before, between, and after items are equal.
    SpaceEvenly,
    /// The outer gaps are half the size of the gaps between items.
    SpaceAround,
    /// Safe `start` alignment.
    SafeStart,
    /// Safe `end` alignment.
    SafeEnd,
    /// Safe `flex-start` alignment.
    SafeFlexStart,
    /// Safe `flex-end` alignment.
    SafeFlexEnd,
    /// Safe `center` alignment.
    SafeCenter,
}

impl JustifyContent {
    /// Uses the layout mode's normal alignment.
    pub const NORMAL: Self = Self::Normal;
    /// Content is packed toward the start of the axis.
    pub const START: Self = Self::Start;
    /// Content is packed toward the end of the axis.
    pub const END: Self = Self::End;
    /// Content is packed toward the flex-relative start of the axis.
    pub const FLEX_START: Self = Self::FlexStart;
    /// Content is packed toward the flex-relative end of the axis.
    pub const FLEX_END: Self = Self::FlexEnd;
    /// Content is centered within the container.
    pub const CENTER: Self = Self::Center;
    /// Content is stretched to fill the container.
    pub const STRETCH: Self = Self::Stretch;
    /// The first and last items are flush with the container edges.
    pub const SPACE_BETWEEN: Self = Self::SpaceBetween;
    /// The gaps before, between, and after items are equal.
    pub const SPACE_EVENLY: Self = Self::SpaceEvenly;
    /// The outer gaps are half the size of the gaps between items.
    pub const SPACE_AROUND: Self = Self::SpaceAround;
    /// Safe `start` alignment.
    pub const SAFE_START: Self = Self::SafeStart;
    /// Safe `end` alignment.
    pub const SAFE_END: Self = Self::SafeEnd;
    /// Safe `flex-start` alignment.
    pub const SAFE_FLEX_START: Self = Self::SafeFlexStart;
    /// Safe `flex-end` alignment.
    pub const SAFE_FLEX_END: Self = Self::SafeFlexEnd;
    /// Safe `center` alignment.
    pub const SAFE_CENTER: Self = Self::SafeCenter;

    /// Returns `true` if this value carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(self, Self::SafeStart | Self::SafeEnd | Self::SafeFlexStart | Self::SafeFlexEnd | Self::SafeCenter)
    }

    /// Removes the `safe` overflow-position modifier.
    #[inline]
    pub(crate) const fn unsafe_variant(self) -> Self {
        match self {
            Self::SafeStart => Self::Start,
            Self::SafeEnd => Self::End,
            Self::SafeFlexStart => Self::FlexStart,
            Self::SafeFlexEnd => Self::FlexEnd,
            Self::SafeCenter => Self::Center,
            value => value,
        }
    }

    /// Returns the reversed alignment for right-to-left contexts.
    #[inline]
    pub(crate) const fn reversed(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
            Self::FlexStart => Self::FlexEnd,
            Self::FlexEnd => Self::FlexStart,
            Self::SafeStart => Self::SafeEnd,
            Self::SafeEnd => Self::SafeStart,
            Self::SafeFlexStart => Self::SafeFlexEnd,
            Self::SafeFlexEnd => Self::SafeFlexStart,
            Self::Stretch => Self::End,
            value => value,
        }
    }
}

#[cfg(feature = "parse")]
impl FromCss for AlignItems {
    fn from_css<'i>(input: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        let first = input.expect_ident()?.clone();
        cssparser::match_ignore_ascii_case! { &*first,
            "safe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::SAFE_START),
                    "end" => Ok(Self::SAFE_END),
                    "flex-start" => Ok(Self::SAFE_FLEX_START),
                    "flex-end" => Ok(Self::SAFE_FLEX_END),
                    "self-start" => Ok(Self::SAFE_SELF_START),
                    "self-end" => Ok(Self::SAFE_SELF_END),
                    "center" => Ok(Self::SAFE_CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "unsafe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::START),
                    "end" => Ok(Self::END),
                    "flex-start" => Ok(Self::FLEX_START),
                    "flex-end" => Ok(Self::FLEX_END),
                    "self-start" => Ok(Self::SELF_START),
                    "self-end" => Ok(Self::SELF_END),
                    "center" => Ok(Self::CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "normal" => Ok(Self::NORMAL),
            "start" => Ok(Self::START),
            "end" => Ok(Self::END),
            "flex-start" => Ok(Self::FLEX_START),
            "flex-end" => Ok(Self::FLEX_END),
            "self-start" => Ok(Self::SELF_START),
            "self-end" => Ok(Self::SELF_END),
            "center" => Ok(Self::CENTER),
            "baseline" => Ok(Self::BASELINE),
            "stretch" => Ok(Self::STRETCH),
            _ => Err(input.new_unexpected_token_error(Token::Ident(first))),
        }
    }
}

#[cfg(feature = "parse")]
crate::util::parse::from_str_from_css!(AlignItems);

#[cfg(feature = "parse")]
impl FromCss for AlignSelf {
    fn from_css<'i>(input: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        let first = input.expect_ident()?.clone();
        cssparser::match_ignore_ascii_case! { &*first,
            "safe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::SAFE_START),
                    "end" => Ok(Self::SAFE_END),
                    "flex-start" => Ok(Self::SAFE_FLEX_START),
                    "flex-end" => Ok(Self::SAFE_FLEX_END),
                    "self-start" => Ok(Self::SAFE_SELF_START),
                    "self-end" => Ok(Self::SAFE_SELF_END),
                    "center" => Ok(Self::SAFE_CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "unsafe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::START),
                    "end" => Ok(Self::END),
                    "flex-start" => Ok(Self::FLEX_START),
                    "flex-end" => Ok(Self::FLEX_END),
                    "self-start" => Ok(Self::SELF_START),
                    "self-end" => Ok(Self::SELF_END),
                    "center" => Ok(Self::CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "auto" => Ok(Self::AUTO),
            "normal" => Ok(Self::NORMAL),
            "start" => Ok(Self::START),
            "end" => Ok(Self::END),
            "flex-start" => Ok(Self::FLEX_START),
            "flex-end" => Ok(Self::FLEX_END),
            "self-start" => Ok(Self::SELF_START),
            "self-end" => Ok(Self::SELF_END),
            "center" => Ok(Self::CENTER),
            "baseline" => Ok(Self::BASELINE),
            "stretch" => Ok(Self::STRETCH),
            _ => Err(input.new_unexpected_token_error(Token::Ident(first))),
        }
    }
}

#[cfg(feature = "parse")]
crate::util::parse::from_str_from_css!(AlignSelf);

#[cfg(feature = "parse")]
impl FromCss for JustifyItems {
    fn from_css<'i>(input: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        let first = input.expect_ident()?.clone();
        cssparser::match_ignore_ascii_case! { &*first,
            "safe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::SAFE_START),
                    "end" => Ok(Self::SAFE_END),
                    "flex-start" => Ok(Self::SAFE_FLEX_START),
                    "flex-end" => Ok(Self::SAFE_FLEX_END),
                    "self-start" => Ok(Self::SAFE_SELF_START),
                    "self-end" => Ok(Self::SAFE_SELF_END),
                    "center" => Ok(Self::SAFE_CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "unsafe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::START),
                    "end" => Ok(Self::END),
                    "flex-start" => Ok(Self::FLEX_START),
                    "flex-end" => Ok(Self::FLEX_END),
                    "self-start" => Ok(Self::SELF_START),
                    "self-end" => Ok(Self::SELF_END),
                    "center" => Ok(Self::CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "normal" => Ok(Self::NORMAL),
            "start" => Ok(Self::START),
            "end" => Ok(Self::END),
            "flex-start" => Ok(Self::FLEX_START),
            "flex-end" => Ok(Self::FLEX_END),
            "self-start" => Ok(Self::SELF_START),
            "self-end" => Ok(Self::SELF_END),
            "center" => Ok(Self::CENTER),
            "baseline" => Ok(Self::BASELINE),
            "stretch" => Ok(Self::STRETCH),
            _ => Err(input.new_unexpected_token_error(Token::Ident(first))),
        }
    }
}

#[cfg(feature = "parse")]
crate::util::parse::from_str_from_css!(JustifyItems);

#[cfg(feature = "parse")]
impl FromCss for JustifySelf {
    fn from_css<'i>(input: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        let first = input.expect_ident()?.clone();
        cssparser::match_ignore_ascii_case! { &*first,
            "safe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::SAFE_START),
                    "end" => Ok(Self::SAFE_END),
                    "flex-start" => Ok(Self::SAFE_FLEX_START),
                    "flex-end" => Ok(Self::SAFE_FLEX_END),
                    "self-start" => Ok(Self::SAFE_SELF_START),
                    "self-end" => Ok(Self::SAFE_SELF_END),
                    "center" => Ok(Self::SAFE_CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "unsafe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::START),
                    "end" => Ok(Self::END),
                    "flex-start" => Ok(Self::FLEX_START),
                    "flex-end" => Ok(Self::FLEX_END),
                    "self-start" => Ok(Self::SELF_START),
                    "self-end" => Ok(Self::SELF_END),
                    "center" => Ok(Self::CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "auto" => Ok(Self::AUTO),
            "normal" => Ok(Self::NORMAL),
            "start" => Ok(Self::START),
            "end" => Ok(Self::END),
            "flex-start" => Ok(Self::FLEX_START),
            "flex-end" => Ok(Self::FLEX_END),
            "self-start" => Ok(Self::SELF_START),
            "self-end" => Ok(Self::SELF_END),
            "center" => Ok(Self::CENTER),
            "baseline" => Ok(Self::BASELINE),
            "stretch" => Ok(Self::STRETCH),
            _ => Err(input.new_unexpected_token_error(Token::Ident(first))),
        }
    }
}

#[cfg(feature = "parse")]
crate::util::parse::from_str_from_css!(JustifySelf);

#[cfg(feature = "parse")]
impl FromCss for AlignContent {
    fn from_css<'i>(input: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        let first = input.expect_ident()?.clone();
        cssparser::match_ignore_ascii_case! { &*first,
            "safe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::SAFE_START),
                    "end" => Ok(Self::SAFE_END),
                    "flex-start" => Ok(Self::SAFE_FLEX_START),
                    "flex-end" => Ok(Self::SAFE_FLEX_END),
                    "center" => Ok(Self::SAFE_CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "unsafe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::START),
                    "end" => Ok(Self::END),
                    "flex-start" => Ok(Self::FLEX_START),
                    "flex-end" => Ok(Self::FLEX_END),
                    "center" => Ok(Self::CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "normal" => Ok(Self::NORMAL),
            "start" => Ok(Self::START),
            "end" => Ok(Self::END),
            "flex-start" => Ok(Self::FLEX_START),
            "flex-end" => Ok(Self::FLEX_END),
            "center" => Ok(Self::CENTER),
            "stretch" => Ok(Self::STRETCH),
            "space-between" => Ok(Self::SPACE_BETWEEN),
            "space-evenly" => Ok(Self::SPACE_EVENLY),
            "space-around" => Ok(Self::SPACE_AROUND),
            _ => Err(input.new_unexpected_token_error(Token::Ident(first))),
        }
    }
}

#[cfg(feature = "parse")]
crate::util::parse::from_str_from_css!(AlignContent);

#[cfg(feature = "parse")]
impl FromCss for JustifyContent {
    fn from_css<'i>(input: &mut Parser<'i, '_>) -> CssParseResult<'i, Self> {
        let first = input.expect_ident()?.clone();
        cssparser::match_ignore_ascii_case! { &*first,
            "safe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::SAFE_START),
                    "end" => Ok(Self::SAFE_END),
                    "flex-start" => Ok(Self::SAFE_FLEX_START),
                    "flex-end" => Ok(Self::SAFE_FLEX_END),
                    "center" => Ok(Self::SAFE_CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "unsafe" => {
                let position = input.expect_ident()?.clone();
                cssparser::match_ignore_ascii_case! { &*position,
                    "start" => Ok(Self::START),
                    "end" => Ok(Self::END),
                    "flex-start" => Ok(Self::FLEX_START),
                    "flex-end" => Ok(Self::FLEX_END),
                    "center" => Ok(Self::CENTER),
                    _ => Err(input.new_unexpected_token_error(Token::Ident(position))),
                }
            },
            "normal" => Ok(Self::NORMAL),
            "start" => Ok(Self::START),
            "end" => Ok(Self::END),
            "flex-start" => Ok(Self::FLEX_START),
            "flex-end" => Ok(Self::FLEX_END),
            "center" => Ok(Self::CENTER),
            "stretch" => Ok(Self::STRETCH),
            "space-between" => Ok(Self::SPACE_BETWEEN),
            "space-evenly" => Ok(Self::SPACE_EVENLY),
            "space-around" => Ok(Self::SPACE_AROUND),
            _ => Err(input.new_unexpected_token_error(Token::Ident(first))),
        }
    }
}

#[cfg(feature = "parse")]
crate::util::parse::from_str_from_css!(JustifyContent);

/// Deserializes an [`AlignItems`] value, treating the old nullable representation as `normal`.
#[cfg(feature = "serde")]
pub(crate) fn deserialize_align_items_or_normal<'de, D>(deserializer: D) -> Result<AlignItems, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_alignment_or(deserializer, AlignItems::NORMAL)
}

/// Deserializes an [`AlignSelf`] value, treating the old nullable representation as `auto`.
#[cfg(feature = "serde")]
pub(crate) fn deserialize_align_self_or_auto<'de, D>(deserializer: D) -> Result<AlignSelf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_alignment_or(deserializer, AlignSelf::AUTO)
}

/// Deserializes a [`JustifyItems`] value, treating the old nullable representation as `normal`.
#[cfg(feature = "serde")]
pub(crate) fn deserialize_justify_items_or_normal<'de, D>(deserializer: D) -> Result<JustifyItems, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_alignment_or(deserializer, JustifyItems::NORMAL)
}

/// Deserializes a [`JustifySelf`] value, treating the old nullable representation as `auto`.
#[cfg(feature = "serde")]
pub(crate) fn deserialize_justify_self_or_auto<'de, D>(deserializer: D) -> Result<JustifySelf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_alignment_or(deserializer, JustifySelf::AUTO)
}

/// Deserializes an [`AlignContent`] value, treating the old nullable representation as `normal`.
#[cfg(feature = "serde")]
pub(crate) fn deserialize_align_content_or_normal<'de, D>(deserializer: D) -> Result<AlignContent, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_alignment_or(deserializer, AlignContent::NORMAL)
}

/// Deserializes a [`JustifyContent`] value, treating the old nullable representation as `normal`.
#[cfg(feature = "serde")]
pub(crate) fn deserialize_justify_content_or_normal<'de, D>(deserializer: D) -> Result<JustifyContent, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_alignment_or(deserializer, JustifyContent::NORMAL)
}

/// Deserializes either an alignment value or a legacy `null` value.
#[cfg(feature = "serde")]
fn deserialize_alignment_or<'de, D, T>(deserializer: D, fallback: T) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(<Option<T> as serde::Deserialize>::deserialize(deserializer)?.unwrap_or(fallback))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::any::TypeId;
    use core::mem::size_of;

    #[test]
    fn alignment_types_are_single_byte_enums() {
        assert_eq!(size_of::<AlignItems>(), 1);
        assert_eq!(size_of::<AlignSelf>(), 1);
        assert_eq!(size_of::<JustifyItems>(), 1);
        assert_eq!(size_of::<JustifySelf>(), 1);
        assert_eq!(size_of::<AlignContent>(), 1);
        assert_eq!(size_of::<JustifyContent>(), 1);
    }

    #[test]
    fn alignment_properties_have_distinct_types() {
        let types = [
            TypeId::of::<AlignItems>(),
            TypeId::of::<AlignSelf>(),
            TypeId::of::<JustifyItems>(),
            TypeId::of::<JustifySelf>(),
            TypeId::of::<AlignContent>(),
            TypeId::of::<JustifyContent>(),
        ];

        for (index, lhs) in types.iter().enumerate() {
            for rhs in &types[index + 1..] {
                assert_ne!(lhs, rhs);
            }
        }
    }

    #[test]
    fn property_defaults_match_css_initial_values() {
        assert_eq!(AlignItems::default(), AlignItems::NORMAL);
        assert_eq!(AlignSelf::default(), AlignSelf::AUTO);
        assert_eq!(JustifyItems::default(), JustifyItems::NORMAL);
        assert_eq!(JustifySelf::default(), JustifySelf::AUTO);
        assert_eq!(AlignContent::default(), AlignContent::NORMAL);
        assert_eq!(JustifyContent::default(), JustifyContent::NORMAL);
    }

    #[test]
    fn safe_values_are_identified_and_stripped() {
        assert!(AlignItems::SAFE_SELF_START.is_safe());
        assert!(!AlignItems::SELF_START.is_safe());

        assert!(AlignSelf::SAFE_CENTER.is_safe());
        assert_eq!(AlignSelf::SAFE_CENTER.unsafe_variant(), AlignSelf::CENTER);
        assert!(!AlignSelf::AUTO.is_safe());

        assert!(JustifyItems::SAFE_END.is_safe());
        assert!(!JustifyItems::NORMAL.is_safe());

        assert!(JustifySelf::SAFE_FLEX_END.is_safe());
        assert_eq!(JustifySelf::SAFE_FLEX_END.unsafe_variant(), JustifySelf::FLEX_END);
        assert!(!JustifySelf::AUTO.is_safe());

        assert!(AlignContent::SAFE_START.is_safe());
        assert_eq!(AlignContent::SAFE_START.unsafe_variant(), AlignContent::START);
        assert!(!AlignContent::SPACE_BETWEEN.is_safe());

        assert!(JustifyContent::SAFE_CENTER.is_safe());
        assert_eq!(JustifyContent::SAFE_CENTER.unsafe_variant(), JustifyContent::CENTER);
        assert!(!JustifyContent::SPACE_AROUND.is_safe());
    }

    #[test]
    fn item_to_self_conversions_preserve_values() {
        assert_eq!(AlignSelf::from(AlignItems::NORMAL), AlignSelf::NORMAL);
        assert_eq!(AlignSelf::from(AlignItems::SAFE_SELF_END), AlignSelf::SAFE_SELF_END);
        assert_eq!(JustifySelf::from(JustifyItems::STRETCH), JustifySelf::STRETCH);
        assert_eq!(JustifySelf::from(JustifyItems::SAFE_FLEX_START), JustifySelf::SAFE_FLEX_START);
    }

    #[test]
    fn self_relative_alignment_resolves_per_axis_and_direction() {
        use Direction::{Ltr, Rtl};

        assert_eq!(AlignSelf::SELF_START.resolve_self_relative(Ltr, Ltr, true), AlignSelf::START);
        assert_eq!(AlignSelf::SELF_START.resolve_self_relative(Ltr, Rtl, true), AlignSelf::END);
        assert_eq!(AlignSelf::SAFE_SELF_END.resolve_self_relative(Rtl, Ltr, true), AlignSelf::SAFE_START);
        assert_eq!(AlignSelf::SELF_START.resolve_self_relative(Ltr, Rtl, false), AlignSelf::START);

        assert_eq!(JustifySelf::SELF_END.resolve_self_relative(Rtl, Rtl, true), JustifySelf::END);
        assert_eq!(JustifySelf::SELF_END.resolve_self_relative(Rtl, Ltr, true), JustifySelf::START);
        assert_eq!(JustifySelf::SAFE_SELF_START.resolve_self_relative(Rtl, Ltr, false), JustifySelf::SAFE_START);
    }

    #[test]
    fn content_alignment_reverses_directional_values() {
        assert_eq!(AlignContent::START.reversed(), AlignContent::END);
        assert_eq!(AlignContent::SAFE_FLEX_END.reversed(), AlignContent::SAFE_FLEX_START);
        assert_eq!(AlignContent::STRETCH.reversed(), AlignContent::END);
        assert_eq!(AlignContent::SPACE_AROUND.reversed(), AlignContent::SPACE_AROUND);

        assert_eq!(JustifyContent::END.reversed(), JustifyContent::START);
        assert_eq!(JustifyContent::SAFE_START.reversed(), JustifyContent::SAFE_END);
        assert_eq!(JustifyContent::CENTER.reversed(), JustifyContent::CENTER);
    }

    #[cfg(feature = "parse")]
    #[test]
    fn auto_is_only_accepted_by_self_alignment_properties() {
        assert_eq!("auto".parse::<AlignSelf>().unwrap(), AlignSelf::AUTO);
        assert_eq!("AUTO".parse::<JustifySelf>().unwrap(), JustifySelf::AUTO);
        assert!("auto".parse::<AlignItems>().is_err());
        assert!("auto".parse::<JustifyItems>().is_err());
        assert!("auto".parse::<AlignContent>().is_err());
        assert!("auto".parse::<JustifyContent>().is_err());
    }

    #[cfg(feature = "parse")]
    #[test]
    fn parses_each_property_specific_type() {
        assert_eq!("normal".parse::<AlignItems>().unwrap(), AlignItems::NORMAL);
        assert_eq!("safe self-start".parse::<AlignSelf>().unwrap(), AlignSelf::SAFE_SELF_START);
        assert_eq!("baseline".parse::<JustifyItems>().unwrap(), JustifyItems::BASELINE);
        assert_eq!("unsafe center".parse::<JustifySelf>().unwrap(), JustifySelf::CENTER);
        assert_eq!("space-between".parse::<AlignContent>().unwrap(), AlignContent::SPACE_BETWEEN);
        assert_eq!("safe flex-end".parse::<JustifyContent>().unwrap(), JustifyContent::SAFE_FLEX_END);
    }

    #[cfg(feature = "parse")]
    #[test]
    fn rejects_invalid_overflow_position_combinations() {
        assert!("safe normal".parse::<AlignItems>().is_err());
        assert!("unsafe auto".parse::<AlignSelf>().is_err());
        assert!("safe stretch".parse::<JustifyItems>().is_err());
        assert!("unsafe baseline".parse::<JustifySelf>().is_err());
        assert!("safe space-around".parse::<AlignContent>().is_err());
        assert!("unsafe space-between".parse::<JustifyContent>().is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_uses_enum_variant_tags() {
        assert_eq!(serde_json::to_string(&AlignItems::SAFE_SELF_END).unwrap(), "\"SafeSelfEnd\"");
        assert_eq!(serde_json::to_string(&AlignSelf::AUTO).unwrap(), "\"Auto\"");
        assert_eq!(serde_json::to_string(&JustifyItems::NORMAL).unwrap(), "\"Normal\"");
        assert_eq!(serde_json::to_string(&JustifySelf::SAFE_CENTER).unwrap(), "\"SafeCenter\"");
        assert_eq!(serde_json::to_string(&AlignContent::SPACE_BETWEEN).unwrap(), "\"SpaceBetween\"");
        assert_eq!(serde_json::to_string(&JustifyContent::SAFE_FLEX_END).unwrap(), "\"SafeFlexEnd\"");

        assert_eq!(serde_json::from_str::<AlignItems>("\"Start\"").unwrap(), AlignItems::START);
        assert_eq!(serde_json::from_str::<AlignSelf>("\"Auto\"").unwrap(), AlignSelf::AUTO);
        assert_eq!(serde_json::from_str::<JustifyItems>("\"Stretch\"").unwrap(), JustifyItems::STRETCH);
        assert_eq!(serde_json::from_str::<JustifySelf>("\"SelfEnd\"").unwrap(), JustifySelf::SELF_END);
        assert_eq!(serde_json::from_str::<AlignContent>("\"SpaceEvenly\"").unwrap(), AlignContent::SPACE_EVENLY);
        assert_eq!(serde_json::from_str::<JustifyContent>("\"Normal\"").unwrap(), JustifyContent::NORMAL);
    }
}
