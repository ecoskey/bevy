use bevy_asset::Assets;
use bevy_ecs::{
    component::{Component, HookContext},
    entity::{ContainsEntity, Entity, MapEntities},
    event::Event,
    observer::Trigger,
    query::{Has, QueryEntityError, With},
    reflect::ReflectComponent,
    system::{Commands, Query, Res, Single},
    world::DeferredWorld,
};
use bevy_image::Image;
use bevy_platform::sync::Arc;
use bevy_window::{PrimaryWindow, Window};
use tracing::warn;

use crate::{render_graph::RenderSubGraph, sync_world::SyncToRenderWorld};

use super::{
    ManualTextureViews, NormalizedRenderTarget, RenderGraphDriver, RenderTarget, RenderTargetInfo,
    SubView, View, ViewTarget,
};

// -----------------------------------------------------------------------------
// Core Compositor Types

#[derive(Component, Default)]
#[require(
    RenderTarget,
    CompositedViews,
    RenderGraphDriver::new(SimpleCompositorGraph),
    SyncToRenderWorld
)]
pub struct Compositor {
    target: Option<Arc<(NormalizedRenderTarget, RenderTargetInfo)>>,
}

#[derive(Component)]
#[relationship(relationship_target = CompositedViews)]
pub struct CompositedBy(pub Entity);

impl ContainsEntity for CompositedBy {
    fn entity(&self) -> Entity {
        self.0
    }
}

//TODO: need to modify relationship hooks to trigger compositor events

#[derive(Component, Default)]
#[relationship_target(relationship = CompositedBy)]
pub struct CompositedViews(Vec<Entity>);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, RenderSubGraph)]
pub struct SimpleCompositorGraph;

// -----------------------------------------------------------------------------
// Compositor Events

#[derive(Event)]
#[event(auto_propagate, traversal = &'static CompositedBy)]
pub struct CompositorEvent {
    pub source: Entity,
    pub ty: CompositorEventType,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum CompositorEventType {
    ViewChanged,
    SubViewChanged,
    RenderTargetChanged,
    RefreshAll,
}

fn handle_compositor_events(
    trigger: Trigger<CompositorEvent>,
    mut compositors: Query<(&mut Compositor, &RenderTarget, &CompositedViews)>,
    mut views: Query<(&View, &SubView, &mut ViewTarget)>,
    primary_window: Option<Single<Entity, With<PrimaryWindow>>>,
    windows: Query<(Entity, &Window)>,
    images: Res<Assets<Image>>,
    manual_texture_views: Res<ManualTextureViews>,
    mut commands: Commands,
) {
    let CompositorEvent { source, ty } = *trigger.event();
    let Ok((mut compositor, render_target, composited_views)) =
        compositors.get_mut(trigger.target())
    else {
        warn!("");
        return;
    };

    let mut handle_render_target_changed = || {
        match render_target.normalize(primary_window.as_deref().copied()) {
            Some(normalized_target) => {
                let target_info = normalized_target.get_render_target_info(
                    windows,
                    &images,
                    &manual_texture_views,
                );
                let new_target = Arc::new((render_target.clone(), render_target));
                //todo: recalculate render target info, propagate to all views
            }
            None => {
                compositor.target = None;
            }
        }
    };

    let mut handle_view_changed =
        |view: Entity, target: Arc<(NormalizedRenderTarget, RenderTargetInfo)>| {
            match views.get_mut(view) {
                Ok((View::Enabled, sub_view, mut view_target)) => {
                    //TODO: no unwrap, actually calculate viewports
                    *view_target = ViewTarget {
                        target,
                        viewport: None,
                    };
                }
                Err(QueryEntityError::QueryDoesNotMatch(..)) => {
                    commands.entity(view).remove::<ViewTarget>();
                }
                // entity does not exist, or mutable access error. In either case, we can ignore it.
                _ => {}
            }
        };

    let handle_sub_view_changed = |view: Entity| {
        match views.get_mut(view) {
            Ok((View::Enabled, sub_view, mut view_target)) => {}
            Err(QueryEntityError::QueryDoesNotMatch(..)) => {
                commands.entity(view).remove::<ViewTarget>();
            }
            // entity does not exist, or mutable access error. In either case, we can ignore it.
            _ => {}
        }
    };

    match ty {
        CompositorEventType::RenderTargetChanged => handle_render_target_changed(),
        CompositorEventType::ViewChanged => {
            if let Some(target) = compositor.target.as_ref() {
                handle_view_changed(source, target.clone());
            } else {
                //warn on invalid compositor state;
            }
        }
        CompositorEventType::SubViewChanged => {
            match views.get_mut(source) {
                Ok((view, sub_view, mut view_target)) => {
                    //todo: recalculate viewport
                }
                Err(QueryEntityError::EntityDoesNotExist(..)) => {
                    // View was despawned or the event incorrectly targeted a non-view.
                    // Either way, don't need to do anything.
                }
                _ => unreachable!(),
            }
        }

        CompositorEventType::RefreshAll => {}
    }
}
