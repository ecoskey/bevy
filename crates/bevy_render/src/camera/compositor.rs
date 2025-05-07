use bevy_ecs::{
    component::{Component, HookContext},
    entity::{ContainsEntity, Entity},
    event::Event,
    observer::Trigger,
    query::Has,
    system::Query,
    world::DeferredWorld,
};
use bevy_math::{Rect, URect, UVec2, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use tracing::warn;

use crate::{render_graph::RenderSubGraph, sync_world::SyncToRenderWorld};

use super::{RenderGraphDriver, RenderTarget, RenderTargetInfo, Viewport};

// -----------------------------------------------------------------------------
// Core Compositor Types

#[derive(Component, Default)]
#[require(
    RenderTarget,
    CompositedViews,
    RenderGraphDriver::new(SimpleCompositorGraph)
)]
pub struct Compositor {
    views: Vec<Entity>,
    invalid: bool,
}

#[derive(Component)]
#[relationship(relationship_target = CompositedViews)]
pub struct CompositedBy(pub Entity);

impl ContainsEntity for CompositedBy {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Default)]
#[relationship_target(relationship = CompositedBy)]
pub struct CompositedViews(Vec<Entity>);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, RenderSubGraph)]
pub struct SimpleCompositorGraph;

// -----------------------------------------------------------------------------
// Compositor Events

#[derive(Event)]
#[event(auto_propagate, traversal = &'static CompositedBy)]
pub enum CompositorEvent {
    ViewDisabled,
    ViewEnabled,
    RenderTargetChanged,
    SubViewChanged,
    RefreshAll,
}

fn handle_compositor_events(
    ev: Trigger<CompositorEvent>,
    compositors: Query<(&mut Compositor, &CompositedViews, &RenderTarget)>,
    views: Query<(&View, &SubView, &mut ViewTarget)>,
) {
}

// -----------------------------------------------------------------------------
// Views

#[derive(Component, Default)]
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
        let (view, is_composited) = world
            .entity(ctx.entity)
            .get_components::<(&View, Has<CompositedBy>)>()
            .unwrap();

        let ev = match view {
            View::Disabled => CompositorEvent::ViewDisabled,
            View::Enabled => CompositorEvent::ViewEnabled,
        };

        world.trigger_targets(ev, ctx.entity);

        if !is_composited {
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
        world.trigger_targets(CompositorEvent::ViewDisabled, ctx.entity);
    }
}

/// Settings to define a camera sub view.
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
#[derive(Debug, Component, Clone, Copy, Reflect, PartialEq)]
#[component(immutable, on_insert = Self::on_insert)]
#[reflect(Clone, PartialEq, Default)]
pub struct SubView {
    /// Size of the entire camera view
    pub full_size: UVec2,
    /// Offset of the sub camera
    pub offset: Vec2,
    /// Size of the sub camera
    pub size: UVec2,
}

impl SubView {
    fn on_insert(mut world: DeferredWorld, ctx: HookContext) {
        world.trigger_targets(CompositorEvent::SubViewChanged, ctx.entity);
    }
}

impl Default for SubView {
    fn default() -> Self {
        Self {
            full_size: UVec2::new(1, 1),
            offset: Vec2::new(0., 0.),
            size: UVec2::new(1, 1),
        }
    }
}

#[derive(Component, Default)]
pub struct ViewTarget {
    target: RenderTarget,
    target_info: RenderTargetInfo,
    viewport: Option<Viewport>,
}

//TODO: UPDATE METHOD DOCS
impl ViewTarget {
    #[inline]
    pub fn target(&self) -> &RenderTarget {
        &self.target
    }

    #[inline]
    pub fn target_info(&self) -> &RenderTargetInfo {
        &self.target_info
    }

    #[inline]
    pub fn viewport(&self) -> Option<&Viewport> {
        self.viewport.as_ref()
    }

    /// Converts a physical size in this `Camera` to a logical size.
    #[inline]
    pub fn to_logical(&self, physical_size: UVec2) -> Vec2 {
        let scale = self.target_info.scale_factor;
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
        self.to_logical(self.target_info.physical_size)
    }

    /// The full physical size of this camera's [`RenderTarget`] (in physical pixels),
    /// ignoring custom `viewport` configuration.
    /// Note that if the `viewport` field is [`Some`], this will not represent the size of the rendered area.
    /// For logic that requires the size of the actually rendered area, prefer [`Camera::physical_viewport_size`].
    #[inline]
    pub fn physical_target_size(&self) -> UVec2 {
        self.target_info.physical_size
    }

    #[inline]
    pub fn target_scaling_factor(&self) -> f32 {
        self.target_info.scale_factor
    }
}
