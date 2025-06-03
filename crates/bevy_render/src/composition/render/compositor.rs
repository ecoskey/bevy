use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::{QueryState, With},
    system::{lifetimeless::Read, Commands, Query},
};
use bevy_platform::sync::Arc;

use crate::{
    composition::{
        render_target::{NormalizedRenderTarget, RenderTargetInfo},
        Compositor, RenderGraphDriver, View, Views,
    },
    render_graph::{InternedRenderSubGraph, Node},
    sync_world::RenderEntity,
    Extract,
};

// -----------------------------------------------------------------------------
// Extraction / Render World Logic

#[derive(Component)]
pub struct ExtractedCompositor {
    views: Vec<Entity>,
    target: Arc<(NormalizedRenderTarget, RenderTargetInfo)>,
    sub_graph: InternedRenderSubGraph,
}

pub(super) fn extract_compositors(
    main_compoitors: Extract<Query<(RenderEntity, &Compositor, &Views, &RenderGraphDriver)>>,
    render_compositors: Query<&mut ExtractedCompositor>,
    main_views: Extract<Query<RenderEntity, With<View>>>,
    mut commands: Commands,
) {
}

pub struct RunCompositorsNode {
    compositors: QueryState<Read<ExtractedCompositor>>,
}

impl Node for RunCompositorsNode {}
