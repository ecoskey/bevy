use std::sync::Arc;

use bevy_ecs::{
    component::{Component, HookContext},
    entity::{ContainsEntity, Entity, MapEntities},
    event::Event,
    observer::Trigger,
    query::Has,
    reflect::ReflectComponent,
    system::{Commands, Query},
    world::DeferredWorld,
};
use bevy_math::{Rect, URect, UVec2, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};

use crate::{render_graph::RenderSubGraph, sync_world::SyncToRenderWorld};

use super::{RenderGraphDriver, RenderTarget, RenderTargetInfo, SubView, View, ViewTarget};

// -----------------------------------------------------------------------------
// Core Compositor Types

#[derive(Component, Default, MapEntities)]
#[require(
    RenderTarget,
    CompositedViews,
    RenderGraphDriver::new(SimpleCompositorGraph),
    SyncToRenderWorld
)]
pub struct Compositor {
    #[entities]
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
    let src_entity = todo!();
    match trigger.event() {
        CompositorEvent::ViewDisabled => {
            compositors.get_mut(src_entity);
            commands.entity(src_entity)
        }
        CompositorEvent::ViewEnabled => {}
        CompositorEvent::RenderTargetChanged => todo!(),
        CompositorEvent::SubViewChanged => todo!(),
        CompositorEvent::RefreshAll => todo!(),
    }
}
