mod compositor;
mod view;
pub use compositor::*;
pub use view::*;

use crate::{
    camera::{ClearColor, ExtractedCamera},
    render_graph::{Node, NodeRunError, RenderGraphContext, RenderLabel, RenderSubGraph},
    renderer::RenderContext,
};
use bevy_ecs::{
    entity::ContainsEntity, prelude::QueryState, system::lifetimeless::Read, world::World,
};
use bevy_platform::collections::HashSet;
use wgpu::{LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp};

use super::{CompositedBy, Compositor, RenderGraphDriver, Views};

// TODO:
// - [ ] setup compositor graph structure, and defer to view render graph
// - [ ] extraction and such
// - [x] module structure. This all probably shouldn't still live in `Camera`.
// - [x] move `ComputedCameraValues` around. merge with Frustum?
// - [ ] investigate utility camera query data
// - [ ] fix event dispatch
// - [ ] fix relationship hooks
// - [ ] fix everything else oh god
