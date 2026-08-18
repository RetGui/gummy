//! CSS alignment code shared by the Flexbox and CSS Grid algorithms.

use crate::style::{AlignContent, AlignSelf, JustifyContent, JustifySelf};


/// Resolve the `safe`/`unsafe` overflow-position fallback for a self-level alignment value
/// (used by `align-self` / `justify-self`-style sites and by absolutely-positioned items in
/// flex/grid). If the alignment subject overflows its alignment container and the requested
/// alignment is `safe`, fall back to logical `Start` per CSS Box Alignment
/// <https://www.w3.org/TR/css-align-3/#overflow-values>. Otherwise drop the safety modifier
/// and return the bare keyword.
#[inline]
pub(crate) fn resolve_align_self_safety(alignment: AlignSelf, overflows: bool) -> AlignSelf {
    if alignment.is_safe() && overflows { AlignSelf::START } else { alignment.unsafe_variant() }
}

/// Resolve the `safe`/`unsafe` overflow-position fallback for a self-level alignment value
/// (used by `align-self` / `justify-self`-style sites and by absolutely-positioned items in
/// flex/grid). If the alignment subject overflows its alignment container and the requested
/// alignment is `safe`, fall back to logical `Start` per CSS Box Alignment
/// <https://www.w3.org/TR/css-align-3/#overflow-values>. Otherwise drop the safety modifier
/// and return the bare keyword.
#[inline]
pub(crate) fn resolve_justify_self_safety(alignment: JustifySelf, overflows: bool) -> JustifySelf {
    if alignment.is_safe() && overflows { JustifySelf::START } else { alignment.unsafe_variant() }
}

/// Resolve spec-defined distribution and overflow fallbacks for `align-content`.
///
/// In addition to CSS Box Alignment, this follows the resolution of
/// <https://github.com/w3c/csswg-drafts/issues/10154>.
pub(crate) fn apply_align_content_fallback(
    free_space: f32,
    num_items: usize,
    alignment_mode: AlignContent,
) -> AlignContent {
    let mut alignment = alignment_mode.unsafe_variant();
    let mut is_safe = alignment_mode.is_safe();

    // 1. If there is only a single item being aligned or the items overflow the container, the
    //    distributed alignment keywords (`stretch`, `space-*`) fall back to a positional keyword
    //    and gain implicit `safe` semantics so step 2 can flip them to `Start` on overflow.
    //    https://www.w3.org/TR/css-align-3/#distribution-values
    if num_items <= 1 || free_space <= 0.0 {
        (alignment, is_safe) = match alignment {
            AlignContent::Stretch | AlignContent::SpaceBetween => (AlignContent::FLEX_START, true),
            AlignContent::SpaceAround | AlignContent::SpaceEvenly => (AlignContent::CENTER, true),
            other => (other, is_safe),
        };
    }

    // 2. Safe alignment falls back to `Start` whenever the alignment subject would overflow the
    //    alignment container.
    if free_space <= 0.0 && is_safe { AlignContent::START } else { alignment }
}

/// Resolve spec-defined distribution and overflow fallbacks for `justify-content`.
pub(crate) fn apply_justify_content_fallback(
    free_space: f32,
    num_items: usize,
    alignment_mode: JustifyContent,
) -> JustifyContent {
    let mut alignment = alignment_mode.unsafe_variant();
    let mut is_safe = alignment_mode.is_safe();


    // 1. If there is only a single item being aligned or the items overflow the container, the
    //    distributed alignment keywords (`stretch`, `space-*`) fall back to a positional keyword
    //    and gain implicit `safe` semantics so step 2 can flip them to `Start` on overflow.
    //    https://www.w3.org/TR/css-align-3/#distribution-values
    if num_items <= 1 || free_space <= 0.0 {
        (alignment, is_safe) = match alignment {
            JustifyContent::Stretch | JustifyContent::SpaceBetween => (JustifyContent::FLEX_START, true),
            JustifyContent::SpaceAround | JustifyContent::SpaceEvenly => (JustifyContent::CENTER, true),
            other => (other, is_safe),
        };
    }

    // 2. Safe alignment falls back to `Start` whenever the alignment subject would overflow the
    //    alignment container.
    if free_space <= 0.0 && is_safe { JustifyContent::START } else { alignment }
}

/// Compute an `align-content` offset.
///
/// CSS Grid does not apply gaps as part of alignment, so `gap` is zero for Grid.
pub(crate) fn compute_align_content_offset(
    free_space: f32,
    num_items: usize,
    gap: f32,
    alignment_mode: AlignContent,
    layout_is_flex_reversed: bool,
    is_first: bool,
) -> f32 {
    if is_first {
        match alignment_mode {
            AlignContent::Start => 0.0,
            AlignContent::FlexStart => {
                if layout_is_flex_reversed {
                    free_space
                } else {
                    0.0
                }
            }
            AlignContent::End => free_space,
            AlignContent::FlexEnd => {
                if layout_is_flex_reversed {
                    0.0
                } else {
                    free_space
                }
            }
            AlignContent::Center => free_space / 2.0,
            AlignContent::Stretch | AlignContent::SpaceBetween => 0.0,
            AlignContent::SpaceAround => {
                if free_space >= 0.0 {
                    (free_space / num_items as f32) / 2.0
                } else {
                    free_space / 2.0
                }
            }
            AlignContent::SpaceEvenly => {
                if free_space >= 0.0 {
                    free_space / (num_items + 1) as f32
                } else {
                    free_space / 2.0
                }
            }
            AlignContent::Normal
            | AlignContent::SafeStart
            | AlignContent::SafeEnd
            | AlignContent::SafeFlexStart
            | AlignContent::SafeFlexEnd
            | AlignContent::SafeCenter => {
                unreachable!("align-content must be resolved before computing offsets")
            }
        }
    } else {
        let free_space = free_space.max(0.0);
        gap + match alignment_mode {
            AlignContent::Start
            | AlignContent::FlexStart
            | AlignContent::End
            | AlignContent::FlexEnd
            | AlignContent::Center
            | AlignContent::Stretch => 0.0,
            AlignContent::SpaceBetween => free_space / (num_items - 1) as f32,
            AlignContent::SpaceAround => free_space / num_items as f32,
            AlignContent::SpaceEvenly => free_space / (num_items + 1) as f32,
            AlignContent::Normal
            | AlignContent::SafeStart
            | AlignContent::SafeEnd
            | AlignContent::SafeFlexStart
            | AlignContent::SafeFlexEnd
            | AlignContent::SafeCenter => {
                unreachable!("align-content must be resolved before computing offsets")
            }
        }
    }
}

/// Compute a `justify-content` offset.
pub(crate) fn compute_justify_content_offset(
    free_space: f32,
    num_items: usize,
    gap: f32,
    alignment_mode: JustifyContent,
    layout_is_flex_reversed: bool,
    is_first: bool,
) -> f32 {
    if is_first {
        match alignment_mode {
            JustifyContent::Start => 0.0,
            JustifyContent::FlexStart => {
                if layout_is_flex_reversed {
                    free_space
                } else {
                    0.0
                }
            }
            JustifyContent::End => free_space,
            JustifyContent::FlexEnd => {
                if layout_is_flex_reversed {
                    0.0
                } else {
                    free_space
                }
            }
            JustifyContent::Center => free_space / 2.0,
            JustifyContent::Stretch | JustifyContent::SpaceBetween => 0.0,
            JustifyContent::SpaceAround => {
                if free_space >= 0.0 {
                    (free_space / num_items as f32) / 2.0
                } else {
                    free_space / 2.0
                }
            }
            JustifyContent::SpaceEvenly => {
                if free_space >= 0.0 {
                    free_space / (num_items + 1) as f32
                } else {
                    free_space / 2.0
                }
            }
            JustifyContent::Normal
            | JustifyContent::SafeStart
            | JustifyContent::SafeEnd
            | JustifyContent::SafeFlexStart
            | JustifyContent::SafeFlexEnd
            | JustifyContent::SafeCenter => {
                unreachable!("justify-content must be resolved before computing offsets")
            }
        }
    } else {
        let free_space = free_space.max(0.0);
        gap + match alignment_mode {
            JustifyContent::Start
            | JustifyContent::FlexStart
            | JustifyContent::End
            | JustifyContent::FlexEnd
            | JustifyContent::Center
            | JustifyContent::Stretch => 0.0,
            JustifyContent::SpaceBetween => free_space / (num_items - 1) as f32,
            JustifyContent::SpaceAround => free_space / num_items as f32,
            JustifyContent::SpaceEvenly => free_space / (num_items + 1) as f32,
            JustifyContent::Normal
            | JustifyContent::SafeStart
            | JustifyContent::SafeEnd
            | JustifyContent::SafeFlexStart
            | JustifyContent::SafeFlexEnd
            | JustifyContent::SafeCenter => {
                unreachable!("justify-content must be resolved before computing offsets")
            }
        }
    }
}
