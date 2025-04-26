use bevy_ecs::{
    component::{Component, HookContext},
    entity::Entity,
    event::Event,
    world::DeferredWorld,
};
use bevy_math::{Rect, URect, UVec2, Vec2};
use tracing::warn;

use crate::sync_world::SyncToRenderWorld;

use super::{RenderGraphDriver, RenderTarget, RenderTargetInfo, Viewport};

#[derive(Component, Default)]
#[require(RenderTarget, CompositedViews)]
pub struct Compositor;

#[derive(Event)]
pub struct RefreshCompositorLayout;

#[derive(Component)]
#[relationship(relationship_target = CompositedViews)]
#[require(View)]
pub struct CompositedBy(pub Entity);

#[derive(Component, Default)]
#[relationship_target(relationship = CompositedBy)]
#[require(Compositor)]
pub struct CompositedViews(Vec<Entity>);

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
        try_refresh_layout(&mut world, &ctx).inspect_err(|()| warn!(
            concat!(
                "{}Entity {} has a View component, but it doesn't have a compositor configured.",
                "Consider adding a `CompositedBy` component that points to an entity with a Compositor."
            ),
            ctx.caller.map(|location| format!("{location}: ")).unwrap_or_default(), ctx.entity,
        ));
    }

    fn on_remove(world: DeferredWorld, ctx: HookContext) {
        if world
            .get::<CompositedBy>(ctx.entity)
            .and_then(|compositor| world.get::<Compositor>(compositor.0))
            .is_some()
        {
            //parent exists and has compositor -> send reeval event
        }
    }
}

fn try_refresh_layout(mut world: &mut DeferredWorld, ctx: &HookContext) -> Result<Entity, ()> {
    match world.get::<CompositedBy>(ctx.entity) {
        Some(CompositedBy(compositor)) => {
            world.trigger_targets(RefreshCompositorLayout, *compositor);
            Ok(*compositor)
        }
        None => Err(()),
    }
}

#[derive(Component, Default)]
pub struct ViewTarget {
    target: RenderTarget,
    target_info: RenderTargetInfo,
    viewport: Option<Viewport>,
}

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
    ///  For logic that requires the full logical size of the
    /// [`RenderTarget`], prefer [`Camera::logical_target_size`].
    ///
    /// Returns `None` if either:
    /// - the function is called just after the `Camera` is created, before `camera_system` is executed,
    /// - the [`RenderTarget`] isn't correctly set:
    ///   - it references the [`PrimaryWindow`](RenderTarget::Window) when there is none,
    ///   - it references a [`Window`](RenderTarget::Window) entity that doesn't exist or doesn't actually have a `Window` component,
    ///   - it references an [`Image`](RenderTarget::Image) that doesn't exist (invalid handle),
    ///   - it references a [`TextureView`](RenderTarget::TextureView) that doesn't exist (invalid handle).
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
