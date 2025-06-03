use bevy_ecs::{
    component::Component,
    entity::{ContainsEntity, Entity, EntityHashSet},
    query::{QueryState, With},
    system::{lifetimeless::Read, Commands, Query},
    world::World,
};
use bevy_platform::{collections::HashSet, sync::Arc};

use crate::{
    camera::ClearColor,
    composition::{
        render_target::{ExtractedWindows, NormalizedRenderTarget, RenderTargetInfo},
        Compositor, RenderGraphDriver, View, Views,
    },
    render_graph::{InternedRenderSubGraph, Node, NodeRunError, RenderGraphContext},
    render_resource::{
        LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    sync_world::RenderEntity,
    Extract,
};

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

            graph.run_sub_graph(compositor.render_graph, vec![], Some(entity));
        }

        //TODO: need to move ClearColor to `crate::composition`?
        let clear_color_global = world.resource::<ClearColor>();

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
                        load: LoadOp::Clear(clear_color_global.to_linear().into()),
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
