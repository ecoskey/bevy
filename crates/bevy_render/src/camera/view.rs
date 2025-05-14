use bevy_ecs::{
    component::{Component, HookContext},
    query::Has,
    world::DeferredWorld,
};
use bevy_math::{Rect, URect, UVec2, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use tracing::warn;

use core::ops::Range;
use std::sync::Arc;

use crate::sync_world::SyncToRenderWorld;

use super::{
    CompositedBy, CompositorEvent, CompositorEventType, NormalizedRenderTarget, RenderGraphDriver,
    RenderTargetInfo,
};

#[derive(Copy, Clone, Default, Debug, Component, Reflect)]
#[component(immutable, on_insert = Self::on_insert, on_remove = Self::on_remove)]
#[require(RenderGraphDriver, SyncToRenderWorld)]
pub enum View {
    Disabled,
    #[default]
    Enabled,
}

impl View {
    pub fn enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    fn on_insert(mut world: DeferredWorld, ctx: HookContext) {
        world.trigger_targets(
            CompositorEvent {
                source: ctx.entity,
                ty: CompositorEventType::ViewChanged,
            },
            ctx.entity,
        );

        if world.entity(ctx.entity).get::<CompositedBy>().is_none() {
            warn!(
                concat!(
                    "{}Entity {} has a View component, but it doesn't have a compositor configured.",
                    "Consider adding a `CompositedBy` component that points to an entity with a Compositor."
                ),
                ctx.caller.map(|location| format!("{location}: ")).unwrap_or_default(), ctx.entity,
            );
        }
    }

    fn on_remove(mut world: DeferredWorld, ctx: HookContext) {
        world.trigger_targets(
            CompositorEvent {
                source: ctx.entity,
                ty: CompositorEventType::ViewChanged,
            },
            ctx.entity,
        );
    }
}

/// Settings to define a sub view.
///
/// When [`Camera::sub_camera_view`] is `Some`, only the sub-section of the
/// image defined by `size` and `offset` (relative to the `full_size` of the
/// whole image) is projected to the cameras viewport.
///
/// Take the example of the following multi-monitor setup:
/// ```css
/// ┌───┬───┐
/// │ A │ B │
/// ├───┼───┤
/// │ C │ D │
/// └───┴───┘
/// ```
/// If each monitor is 1920x1080, the whole image will have a resolution of
/// 3840x2160. For each monitor we can use a single camera with a viewport of
/// the same size as the monitor it corresponds to. To ensure that the image is
/// cohesive, we can use a different sub view on each camera:
/// - Camera A: `full_size` = 3840x2160, `size` = 1920x1080, `offset` = 0,0
/// - Camera B: `full_size` = 3840x2160, `size` = 1920x1080, `offset` = 1920,0
/// - Camera C: `full_size` = 3840x2160, `size` = 1920x1080, `offset` = 0,1080
/// - Camera D: `full_size` = 3840x2160, `size` = 1920x1080, `offset` =
///   1920,1080
///
/// However since only the ratio between the values is important, they could all
/// be divided by 120 and still produce the same image. Camera D would for
/// example have the following values:
/// `full_size` = 32x18, `size` = 16x9, `offset` = 16,9
#[derive(Debug, Component, Clone, Reflect, PartialEq)]
#[component(immutable, on_insert = Self::on_insert)]
#[reflect(Clone, PartialEq, Default)]
pub struct SubView {
    pub rect: SubRect,
    /// The minimum and maximum depth to render (on a scale from 0.0 to 1.0).
    pub depth: Range<f32>,
}

impl SubView {
    fn on_insert(mut world: DeferredWorld, ctx: HookContext) {
        world.trigger_targets(
            CompositorEvent {
                source: ctx.entity,
                ty: CompositorEventType::SubViewChanged,
            },
            ctx.entity,
        );
    }

    fn on_remove(mut world: DeferredWorld, ctx: HookContext) {
        world.trigger_targets(
            CompositorEvent {
                source: ctx.entity,
                ty: CompositorEventType::SubViewChanged,
            },
            ctx.entity,
        );

        if world
            .get_entity(ctx.entity)
            .is_ok_and(|e| e.get::<View>().is_some())
        {
            world
                .commands()
                .entity(ctx.entity)
                .insert(SubView::default());

            warn!(
                "{}Entity {} has a View component but its SubView was removed. Reinserting a default SubView.",
                ctx.caller.map(|location| format!("{location}: ")).unwrap_or_default(), ctx.entity,
            );
        }
    }
}

impl Default for SubView {
    fn default() -> Self {
        Self {
            rect: Default::default(),
            depth: 0.0..1.0,
        }
    }
}

/// Render viewport configuration for the [`Camera`] component.
///
/// The viewport defines the area on the render target to which the camera renders its image.
/// You can overlay multiple cameras in a single window using viewports to create effects like
/// split screen, minimaps, and character viewers.
#[derive(Reflect, Debug, Clone)]
#[reflect(Default, Clone)]
pub struct Viewport {
    /// The physical position to render this viewport to within the [`RenderTarget`] of this [`Camera`].
    /// (0,0) corresponds to the top-left corner
    pub physical_position: UVec2,
    /// The physical size of the viewport rectangle to render to within the [`RenderTarget`] of this [`Camera`].
    /// The origin of the rectangle is in the top-left corner.
    pub physical_size: UVec2,
    /// The minimum and maximum depth to render (on a scale from 0.0 to 1.0).
    pub depth: Range<f32>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            physical_position: Default::default(),
            physical_size: UVec2::new(1, 1),
            depth: 0.0..1.0,
        }
    }
}

impl Viewport {
    /// Cut the viewport rectangle so that it lies inside a rectangle of the
    /// given size.
    ///
    /// If either of the viewport's position coordinates lies outside the given
    /// dimensions, it will be moved just inside first. If either of the given
    /// dimensions is zero, the position and size of the viewport rectangle will
    /// both be set to zero in that dimension.
    pub fn clamp_to_size(&mut self, size: UVec2) {
        // If the origin of the viewport rect is outside, then adjust so that
        // it's just barely inside. Then, cut off the part that is outside.
        if self.physical_size.x + self.physical_position.x > size.x {
            if self.physical_position.x < size.x {
                self.physical_size.x = size.x - self.physical_position.x;
            } else if size.x > 0 {
                self.physical_position.x = size.x - 1;
                self.physical_size.x = 1;
            } else {
                self.physical_position.x = 0;
                self.physical_size.x = 0;
            }
        }
        if self.physical_size.y + self.physical_position.y > size.y {
            if self.physical_position.y < size.y {
                self.physical_size.y = size.y - self.physical_position.y;
            } else if size.y > 0 {
                self.physical_position.y = size.y - 1;
                self.physical_size.y = 1;
            } else {
                self.physical_position.y = 0;
                self.physical_size.y = 0;
            }
        }
    }
}

#[derive(Component, Clone)]
pub struct ViewTarget {
    pub(crate) target: Arc<(NormalizedRenderTarget, RenderTargetInfo)>,
    pub(crate) viewport: Option<Viewport>,
}

//TODO: UPDATE METHOD DOCS
impl ViewTarget {
    #[inline]
    pub fn target(&self) -> &NormalizedRenderTarget {
        &self.target.0
    }

    #[inline]
    pub fn target_info(&self) -> &RenderTargetInfo {
        &self.target.1
    }

    #[inline]
    pub fn viewport(&self) -> Option<&Viewport> {
        self.viewport.as_ref()
    }

    /// Converts a physical size in this `Camera` to a logical size.
    #[inline]
    pub fn to_logical(&self, physical_size: UVec2) -> Vec2 {
        let scale = self.target_info().scale_factor;
        physical_size.as_vec2() / scale
    }

    /// The rendered physical bounds [`URect`] of the camera. If the `viewport` field is
    /// set to [`Some`], this will be the rect of that custom viewport. Otherwise it will default to
    /// the full physical rect of the current [`RenderTarget`].
    #[inline]
    pub fn physical_viewport_rect(&self) -> URect {
        let min = self
            .viewport
            .as_ref()
            .map(|v| v.physical_position)
            .unwrap_or(UVec2::ZERO);
        let max = min + self.physical_viewport_size();
        URect { min, max }
    }

    /// The rendered logical bounds [`Rect`] of the camera. If the `viewport` field is set to
    /// [`Some`], this will be the rect of that custom viewport. Otherwise it will default to the
    /// full logical rect of the current [`RenderTarget`].
    #[inline]
    pub fn logical_viewport_rect(&self) -> Rect {
        let URect { min, max } = self.physical_viewport_rect();
        Rect {
            min: self.to_logical(min),
            max: self.to_logical(max),
        }
    }

    /// The logical size of this camera's viewport. If the `viewport` field is set to [`Some`], this
    /// will be the size of that custom viewport. Otherwise it will default to the full logical size
    /// of the current [`RenderTarget`].
    ///
    /// For logic that requires the full logical size of the
    /// [`RenderTarget`], prefer [`Camera::logical_target_size`].
    #[inline]
    pub fn logical_viewport_size(&self) -> Vec2 {
        self.viewport
            .as_ref()
            .map(|v| self.to_logical(v.physical_size))
            .unwrap_or(self.logical_target_size())
    }

    /// The physical size of this camera's viewport (in physical pixels).
    /// If the `viewport` field is set to [`Some`], this
    /// will be the size of that custom viewport. Otherwise it will default to the full physical size of
    /// the current [`RenderTarget`].
    /// For logic that requires the full physical size of the [`RenderTarget`], prefer [`Camera::physical_target_size`].
    #[inline]
    pub fn physical_viewport_size(&self) -> UVec2 {
        self.viewport
            .as_ref()
            .map(|v| v.physical_size)
            .unwrap_or(self.physical_target_size())
    }

    /// The full logical size of this camera's [`RenderTarget`], ignoring custom `viewport` configuration.
    /// Note that if the `viewport` field is [`Some`], this will not represent the size of the rendered area.
    /// For logic that requires the size of the actually rendered area, prefer [`Camera::logical_viewport_size`].
    #[inline]
    pub fn logical_target_size(&self) -> Vec2 {
        self.to_logical(self.target_info().physical_size)
    }

    /// The full physical size of this camera's [`RenderTarget`] (in physical pixels),
    /// ignoring custom `viewport` configuration.
    /// Note that if the `viewport` field is [`Some`], this will not represent the size of the rendered area.
    /// For logic that requires the size of the actually rendered area, prefer [`Camera::physical_viewport_size`].
    #[inline]
    pub fn physical_target_size(&self) -> UVec2 {
        self.target_info().physical_size
    }

    #[inline]
    pub fn target_scaling_factor(&self) -> f32 {
        self.target_info().scale_factor
    }
}
