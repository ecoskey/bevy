use std::sync::Arc;

use bevy_ecs::{
    component::{Component, HookContext},
    entity::{ContainsEntity, Entity},
    event::Event,
    observer::Trigger,
    query::Has,
    system::{Commands, Query},
    world::DeferredWorld,
};
use bevy_math::{Rect, URect, UVec2, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use tracing::warn;

use crate::{render_graph::RenderSubGraph, sync_world::SyncToRenderWorld};

use super::{
    RenderGraphDriver, RenderTarget, RenderTargetInfo, SubView, View, ViewTarget, Viewport,
};

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
    target: Arc<(RenderTarget, RenderTargetInfo)>,
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
    trigger: Trigger<CompositorEvent>,
    compositors: Query<(&mut Compositor, &CompositedViews, &RenderTarget)>,
    views: Query<(&View, &SubView, &mut ViewTarget)>,
    mut commands: Commands,
) {
    match trigger.event() {
        CompositorEvent::ViewDisabled => {}
        CompositorEvent::ViewEnabled => {}
        CompositorEvent::RenderTargetChanged => todo!(),
        CompositorEvent::SubViewChanged => todo!(),
        CompositorEvent::RefreshAll => todo!(),
    }
}
