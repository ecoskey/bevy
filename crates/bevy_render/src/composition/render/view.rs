/// Describes a view in the render world.
///
/// Each entity in the main world can potentially extract to multiple subviews,
/// each of which has a [`RetainedViewEntity::subview_index`]. For instance, 3D
/// cameras extract to both a 3D camera subview with index 0 and a special UI
/// subview with index 1. Likewise, point lights with shadows extract to 6
/// subviews, one for each side of the shadow cubemap.
#[derive(Component)]
pub struct ExtractedView {
    /// The entity in the main world corresponding to this render world view.
    pub retained_view_entity: RetainedViewEntity,
    pub render_graph: InternedRenderSubGraph,
    /// The render target entity associated with this View
    pub target: NormalizedRenderTarget,
    pub physical_target_size: Option<UVec2>,
    // uvec4(origin.x, origin.y, width, height)
    pub viewport: Option<UVec4>,
}

pub fn extract_views(
    mut commands: Commands,
    views: Extract<
        Query<(
            Entity,
            RenderEntity,
            &View,
            Option<&ViewTarget>,
            &RenderGraphDriver,
        )>,
    >,
    primary_window: Extract<Option<Single<Entity, With<PrimaryWindow>>>>,
    mapper: Extract<Query<&RenderEntity>>,
) {
    for (main_entity, render_entity, view, view_target, view_render_graph) in &views {
        let extracted_view = view_target.map(|view_target| ExtractedView {
            retained_view_entity: RetainedViewEntity::new(main_entity.into(), None, 0),
            render_graph: **view_render_graph,
            target: view_target.target.0.clone(),
            viewport: todo!(),
            physical_target_size: todo!(),
        });
    }
    // let primary_window = primary_window.iter().next();
    // for (
    //     main_entity,
    //     render_entity,
    //     view,
    //     camera_render_graph,
    //     camera,
    //     transform,
    //     visible_entities,
    //     frustum,
    //     hdr,
    //     color_grading,
    //     exposure,
    //     temporal_jitter,
    //     render_layers,
    //     projection,
    //     no_indirect_drawing,
    // ) in query.iter()
    // {
    //     if !camera.is_active {
    //         commands.entity(render_entity).remove::<(
    //             ExtractedCamera,
    //             ExtractedView,
    //             RenderVisibleEntities,
    //             TemporalJitter,
    //             RenderLayers,
    //             Projection,
    //             NoIndirectDrawing,
    //             ViewUniformOffset,
    //         )>();
    //         continue;
    //     }
    //
    //     let color_grading = color_grading.unwrap_or(&ColorGrading::default()).clone();
    //
    //     if let (
    //         Some(URect {
    //             min: viewport_origin,
    //             ..
    //         }),
    //         Some(viewport_size),
    //         Some(target_size),
    //     ) = (
    //         camera.physical_viewport_rect(),
    //         camera.physical_viewport_size(),
    //         camera.physical_target_size(),
    //     ) {
    //         if target_size.x == 0 || target_size.y == 0 {
    //             continue;
    //         }
    //
    //         let render_visible_entities = RenderVisibleEntities {
    //             entities: visible_entities
    //                 .entities
    //                 .iter()
    //                 .map(|(type_id, entities)| {
    //                     let entities = entities
    //                         .iter()
    //                         .map(|entity| {
    //                             let render_entity = mapper
    //                                 .get(*entity)
    //                                 .cloned()
    //                                 .map(|entity| entity.id())
    //                                 .unwrap_or(Entity::PLACEHOLDER);
    //                             (render_entity, (*entity).into())
    //                         })
    //                         .collect();
    //                     (*type_id, entities)
    //                 })
    //                 .collect(),
    //         };
    //
    //         let mut commands = commands.entity(render_entity);
    //         commands.insert((
    //             ExtractedCamera {
    //                 target: camera.target.normalize(primary_window),
    //                 viewport: camera.viewport.clone(),
    //                 physical_viewport_size: Some(viewport_size),
    //                 physical_target_size: Some(target_size),
    //                 render_graph: camera_render_graph.0,
    //                 order: camera.order,
    //                 output_mode: camera.output_mode,
    //                 msaa_writeback: camera.msaa_writeback,
    //                 clear_color: camera.clear_color,
    //                 // this will be set in sort_cameras
    //                 sorted_camera_index_for_target: 0,
    //                 exposure: exposure
    //                     .map(Exposure::exposure)
    //                     .unwrap_or_else(|| Exposure::default().exposure()),
    //                 hdr,
    //             },
    //             ExtractedView {
    //                 retained_view_entity: RetainedViewEntity::new(main_entity.into(), None, 0),
    //                 clip_from_view: camera.clip_from_view(),
    //                 world_from_view: *transform,
    //                 clip_from_world: None,
    //                 hdr,
    //                 viewport: UVec4::new(
    //                     viewport_origin.x,
    //                     viewport_origin.y,
    //                     viewport_size.x,
    //                     viewport_size.y,
    //                 ),
    //                 color_grading,
    //             },
    //             render_visible_entities,
    //             *frustum,
    //         ));
    //
    //         if let Some(temporal_jitter) = temporal_jitter {
    //             commands.insert(temporal_jitter.clone());
    //         }
    //
    //         if let Some(render_layers) = render_layers {
    //             commands.insert(render_layers.clone());
    //         }
    //
    //         if let Some(perspective) = projection {
    //             commands.insert(perspective.clone());
    //         }
    //
    //         if no_indirect_drawing
    //             || !matches!(
    //                 gpu_preprocessing_support.max_supported_mode,
    //                 GpuPreprocessingMode::Culling
    //             )
    //         {
    //             commands.insert(NoIndirectDrawing);
    //         }
    //     };
    // }
}
