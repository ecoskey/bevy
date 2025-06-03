use bevy_asset::Assets;
use bevy_color::{Color, LinearRgba};
use bevy_ecs::{
    component::{Component, HookContext},
    entity::{ContainsEntity, Entity, EntityHashSet},
    event::Event,
    observer::Trigger,
    query::{Has, QueryEntityError, QueryState, With, Without},
    system::{lifetimeless::Read, Commands, Local, Query, Res, Single},
    world::World,
};
use bevy_image::Image;
use bevy_platform::sync::Arc;
use bevy_window::{PrimaryWindow, Window};
use core::iter::Copied;

use crate::{
    render_graph::{
        InternedRenderSubGraph, Node, NodeRunError, RenderGraphContext, RenderSubGraph,
    },
    render_resource::{
        LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    sync_world::{RenderEntity, SyncToRenderWorld},
    Extract,
};

use super::{
    render_target::{ExtractedWindows, NormalizedRenderTarget, RenderTarget, RenderTargetInfo},
    ManualTextureViews, RenderGraphDriver, SubView, View, ViewTarget,
};

// -----------------------------------------------------------------------------
// Core Compositor Types

#[derive(Component, Default)]
#[require(
    RenderTarget,
    Views,
    RenderGraphDriver::new(CompositorGraph),
    SyncToRenderWorld
)]
pub struct Compositor {
    target: Option<Arc<(NormalizedRenderTarget, RenderTargetInfo)>>,
}

#[derive(Component)]
#[relationship(relationship_target = Views)]
pub struct CompositedBy(pub Entity);

impl ContainsEntity for CompositedBy {
    fn entity(&self) -> Entity {
        self.0
    }
}

//TODO: need to modify relationship hooks to trigger compositor events
//TODO: make an analogue of `children!` that works for views
#[derive(Component, Default)]
#[relationship_target(relationship = CompositedBy)]
pub struct Views(Vec<Entity>);

impl<'a> IntoIterator for &'a Views {
    type Item = Entity;

    type IntoIter = Copied<<&'a Vec<Entity> as IntoIterator>::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter().copied()
    }
}

impl FromIterator<Entity> for Views {
    fn from_iter<T: IntoIterator<Item = Entity>>(iter: T) -> Self {
        Self(Vec::from_iter(iter))
    }
}

impl Views {
    pub fn iter(&self) -> impl Iterator<Item = Entity> {
        self.into_iter()
    }
}

// -----------------------------------------------------------------------------
// Compositor Events

#[derive(Event)]
#[event(auto_propagate, traversal = &'static CompositedBy)]
pub(super) enum CompositorEvent {
    CompositorChanged,
    ViewChanged(Entity),
}

//TODO: handle window events

fn handle_compositor_events(
    trigger: Trigger<CompositorEvent>,
    mut compositors: Query<(&mut Compositor, &RenderTarget, &Views), Without<CompositedBy>>,
    mut views: Query<(&View, Option<&SubView>)>,
    primary_window: Option<Single<Entity, With<PrimaryWindow>>>,
    windows: Query<(Entity, &Window)>,
    images: Res<Assets<Image>>,
    manual_texture_views: Res<ManualTextureViews>,
    mut commands: Commands,
) {
    let Ok((mut compositor, render_target, composited_views)) =
        compositors.get_mut(trigger.target())
    else {
        // events propagate up the compositor's tree, so the target may not be a compositor yet
        return;
    };

    fn update_compositor<'a>(
        compositor: &mut Compositor,
        render_target: &RenderTarget,
        primary_window: Option<Entity>,
        windows: impl IntoIterator<Item = (Entity, &'a Window)>,
        images: &Assets<Image>,
        manual_texture_views: &ManualTextureViews,
    ) {
        compositor.target = render_target
            .normalize(primary_window)
            .and_then(|normalized_target| {
                Some(normalized_target.clone()).zip(normalized_target.get_render_target_info(
                    windows,
                    images,
                    manual_texture_views,
                ))
            })
            .map(Arc::new);
    }

    fn update_view(
        compositor: &Compositor,
        view: Entity,
        mut views: Query<(&View, Option<&SubView>)>,
        mut commands: Commands,
    ) {
        let Some(target) = &compositor.target else {
            //todo: warn about invalid compositor state;
            return;
        };

        match views.get_mut(view) {
            Ok((View::Enabled, sub_view)) => {
                let viewport =
                    sub_view.map(|sub_view| sub_view.get_viewport(target.1.physical_size));
                let new_target = ViewTarget {
                    target: target.clone(),
                    viewport,
                };
                commands.entity(view).insert(new_target);
            }
            Ok((View::Disabled, ..)) => {
                // view was disabled, remove its target
                commands.entity(view).remove::<ViewTarget>();
            }
            Err(QueryEntityError::QueryDoesNotMatch(..)) => {
                // if entity is not a view, we should remove it from the relationship
                commands.entity(view).remove::<(ViewTarget, CompositedBy)>();
            }
            // view was despawned, ignore.
            _ => {}
        }
    }

    match *trigger.event() {
        CompositorEvent::CompositorChanged => {
            update_compositor(
                &mut compositor,
                render_target,
                primary_window.as_deref().copied(),
                windows,
                &images,
                &manual_texture_views,
            );

            composited_views.iter().for_each(|view| {
                update_view(&compositor, view, views.reborrow(), commands.reborrow());
            });
        }
        CompositorEvent::ViewChanged(view) => {
            update_view(&compositor, view, views, commands);
        }
    }
}

// -----------------------------------------------------------------------------
// Extraction / Render World Logic

#[derive(Component)]
pub struct ExtractedCompositor {
    views: Vec<Entity>,
    target: Arc<(NormalizedRenderTarget, RenderTargetInfo)>,
    render_graph: InternedRenderSubGraph,
}

pub(super) fn extract_compositors(
    _main_compositors: Extract<Query<(RenderEntity, &Compositor, &Views, &RenderGraphDriver)>>,
    _render_compositors: Query<&mut ExtractedCompositor>,
    _main_views: Extract<Query<RenderEntity, With<View>>>,
    mut _commands: Commands,
) {
    todo!()
}

// -----------------------------------------------------------------------------
// Render Graph

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, RenderSubGraph)]
pub struct CompositorGraph;

pub struct RunCompositorsNode {
    compositors: QueryState<(Entity, Read<ExtractedCompositor>)>,
}

impl Node for RunCompositorsNode {
    fn update(&mut self, world: &mut World) {
        self.compositors.update_archetypes(world);
    }

    fn run(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let mut rendered_windows = EntityHashSet::default();
        for (entity, compositor) in self.compositors.query_manual(world) {
            /* TODO: include this logic in main world, while processing targets on the compositor
            if let Some(NormalizedRenderTarget::Window(window_ref)) = camera.target {
                let window_entity = window_ref.entity();
                if windows
                    .windows
                    .get(&window_entity)
                    .is_some_and(|w| w.physical_width > 0 && w.physical_height > 0)
                {
                    camera_windows.insert(window_entity);
                } else {
                    // The window doesn't exist anymore or zero-sized so we don't need to run the graph
                    run_graph = false;
                }
            }
            */
            if let NormalizedRenderTarget::Window(window_ref) = compositor.target.0 {
                rendered_windows.insert(window_ref.entity());
            }

            let _ = graph.run_sub_graph(compositor.render_graph, vec![], Some(entity));
        }

        // wgpu (and some backends) require doing work for swap chains if you call `get_current_texture()` and `present()`
        // This ensures that Bevy doesn't crash, even when there are no cameras (and therefore no work submitted).
        for (entity, window) in world.resource::<ExtractedWindows>().iter() {
            if rendered_windows.contains(entity) {
                continue;
            }

            let Some(swap_chain_texture) = &window.swap_chain_texture_view else {
                continue;
            };

            #[cfg(feature = "trace")]
            let _span = tracing::info_span!("no_camera_clear_pass").entered();
            let pass_descriptor = RenderPassDescriptor {
                label: Some("window"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: swap_chain_texture,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(LinearRgba::BLACK.into()),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            };

            render_context
                .command_encoder()
                .begin_render_pass(&pass_descriptor);
        }

        Ok(())
    }
}
