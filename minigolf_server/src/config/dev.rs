use {
    crate::{
        Configuration, CourseState, GameState, HoleState, ServerState, config::ServerPlugin,
        course::setup::CourseConfiguration,
    },
    bevy::{
        asset::{ReflectAsset, UntypedAssetId},
        ecs::system::RunSystemOnce,
        math::{DQuat, DVec3},
        prelude::*,
        reflect::TypeRegistry,
        render::camera::{CameraProjection, Viewport},
        tasks::IoTaskPool,
        window::PrimaryWindow,
    },
    bevy_egui::{EguiContext, EguiContextPass, EguiContextSettings, EguiPlugin},
    bevy_inspector_egui::{
        DefaultInspectorConfigPlugin,
        bevy_inspector::hierarchy::hierarchy_ui,
        bevy_inspector::{
            self, hierarchy::SelectedEntities, ui_for_entities_shared_components,
            ui_for_entity_with_children,
        },
    },
    egui_dock::{DockArea, DockState, NodeIndex, Style},
    std::{any::TypeId, fs::File, io::Write},
    transform_gizmo_egui::{Gizmo, GizmoConfig, GizmoExt, GizmoOrientation, mint},
};

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins);

        app.add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        });
        app.add_plugins(DefaultInspectorConfigPlugin);

        app.register_type::<Option<Handle<Image>>>();
        app.register_type::<AlphaMode>();

        app.insert_resource(UiState::new());
        app.init_resource::<SceneLoaderState>();

        app.add_systems(Startup, setup);
        app.add_systems(EguiContextPass, show_ui_system);
        app.add_systems(PostUpdate, set_camera_viewport.after(show_ui_system));
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        Transform::from_xyz(3.0, 0.0, 10.0),
    ));
}

fn show_ui_system(world: &mut World) {
    let Ok(context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .single(world)
    else {
        return;
    };

    let mut context = context.clone();
    world.resource_scope::<UiState, _>(|world, mut ui_state| ui_state.ui(world, context.get_mut()));
}

// make camera only render to view not obstructed by UI
fn set_camera_viewport(
    ui_state: Res<UiState>,
    primary_window: Query<&mut Window, With<PrimaryWindow>>,
    settings: Single<&EguiContextSettings>,
    mut cam: Single<&mut Camera>,
) {
    let Ok(window) = primary_window.single() else {
        return;
    };

    let scale_factor = window.scale_factor() * settings.scale_factor;

    let viewport_pos = ui_state.viewport_rect.left_top().to_vec2() * scale_factor;
    let viewport_size = ui_state.viewport_rect.size() * scale_factor;

    let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
    let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

    // The desired viewport rectangle at its offset in "physical pixel space"
    let rect = physical_position + physical_size;

    let window_size = window.physical_size();
    // wgpu will panic if trying to set a viewport rect which has coordinates extending
    // past the size of the render target, i.e. the physical window in our case.
    // Typically, this shouldn't happen- but during init and resizing etc. edge cases might occur.
    // Simply do nothing in those cases.
    if rect.x <= window_size.x && rect.y <= window_size.y {
        cam.viewport = Some(Viewport {
            physical_position,
            physical_size,
            depth: 0.0..1.0,
        });
    }
}

#[derive(Eq, PartialEq)]
enum InspectorSelection {
    Entities,
    Resource(TypeId, String),
    Asset(TypeId, String, UntypedAssetId),
}

#[derive(Resource)]
struct UiState {
    state: DockState<EditorWindow>,
    viewport_rect: egui::Rect,
    selected_entities: SelectedEntities,
    selection: InspectorSelection,
    gizmo: Gizmo,
}

impl UiState {
    pub fn new() -> Self {
        let mut state = DockState::new(vec![EditorWindow::GameView]);

        let tree = state.main_surface_mut();
        let [game, inspector] =
            tree.split_right(NodeIndex::root(), 0.75, vec![EditorWindow::Inspector]);

        let [_inspector, _scene] =
            tree.split_below(inspector, 0.8, vec![EditorWindow::SceneLoader]);

        let [game, hierarchy] = tree.split_left(game, 0.2, vec![EditorWindow::Hierarchy]);

        let [_hierarchy, _states] = tree.split_below(hierarchy, 0.8, vec![EditorWindow::States]);

        let [_game, _bottom] = tree.split_below(
            game,
            0.8,
            vec![EditorWindow::Resources, EditorWindow::Assets],
        );

        Self {
            state,
            selected_entities: SelectedEntities::default(),
            selection: InspectorSelection::Entities,
            viewport_rect: egui::Rect::NOTHING,
            gizmo: Gizmo::default(),
        }
    }

    fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
        let mut tab_viewer = TabViewer {
            world,
            viewport_rect: &mut self.viewport_rect,
            selected_entities: &mut self.selected_entities,
            selection: &mut self.selection,
            gizmo: &mut self.gizmo,
        };
        DockArea::new(&mut self.state)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut tab_viewer);
    }
}

#[derive(Debug)]
enum EditorWindow {
    GameView,
    Hierarchy,
    Resources,
    Assets,
    Inspector,
    SceneLoader,
    States,
}

struct TabViewer<'a> {
    world: &'a mut World,
    selected_entities: &'a mut SelectedEntities,
    selection: &'a mut InspectorSelection,
    viewport_rect: &'a mut egui::Rect,
    gizmo: &'a mut Gizmo,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = EditorWindow;

    fn title(&mut self, window: &mut Self::Tab) -> egui::WidgetText {
        format!("{window:?}").into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, window: &mut Self::Tab) {
        let type_registry = self.world.resource::<AppTypeRegistry>().0.clone();
        let type_registry = type_registry.read();

        match window {
            EditorWindow::GameView => {
                *self.viewport_rect = ui.clip_rect();

                draw_gizmo(ui, &mut self.gizmo, self.world, self.selected_entities);
            }

            EditorWindow::Hierarchy => {
                let selected = hierarchy_ui(self.world, ui, self.selected_entities);
                if selected {
                    *self.selection = InspectorSelection::Entities;
                }
            }

            EditorWindow::Resources => select_resource(ui, &type_registry, self.selection),

            EditorWindow::Assets => select_asset(ui, &type_registry, self.world, self.selection),

            EditorWindow::Inspector => match *self.selection {
                InspectorSelection::Entities => match self.selected_entities.as_slice() {
                    &[entity] => ui_for_entity_with_children(self.world, entity, ui),
                    entities => ui_for_entities_shared_components(self.world, entities, ui),
                },

                InspectorSelection::Resource(type_id, ref name) => {
                    ui.label(name);
                    bevy_inspector::by_type_id::ui_for_resource(
                        self.world,
                        type_id,
                        ui,
                        name,
                        &type_registry,
                    )
                }

                InspectorSelection::Asset(type_id, ref name, handle) => {
                    ui.label(name);
                    bevy_inspector::by_type_id::ui_for_asset(
                        self.world,
                        type_id,
                        handle,
                        ui,
                        &type_registry,
                    );
                }
            },

            EditorWindow::SceneLoader => scene_loader(ui, self.world),

            EditorWindow::States => states(ui, self.world),
        }
    }

    fn clear_background(&self, window: &Self::Tab) -> bool {
        !matches!(window, EditorWindow::GameView)
    }
}

fn draw_gizmo(
    ui: &mut egui::Ui,
    gizmo: &mut Gizmo,
    world: &mut World,
    selected_entities: &SelectedEntities,
) {
    let (cam_transform, projection) = world
        .query_filtered::<(&GlobalTransform, &Projection), With<Camera3d>>()
        .single(world)
        .expect("Camera not found");
    let view_matrix = Mat4::from(cam_transform.affine().inverse());
    let projection_matrix = projection.get_clip_from_view();

    if selected_entities.len() != 1 {
        return;
    }

    for selected in selected_entities.iter() {
        let Some(transform) = world.get::<Transform>(selected) else {
            continue;
        };

        gizmo.update_config(GizmoConfig {
            view_matrix: view_matrix.to_cols_array().map(|x| x as f64).into(),
            projection_matrix: projection_matrix.to_cols_array().map(|x| x as f64).into(),
            orientation: GizmoOrientation::Local,
            ..Default::default()
        });
        let transform = transform_gizmo_egui::math::Transform::from_scale_rotation_translation(
            mint::Vector3::from([
                transform.scale.x as f64,
                transform.scale.y as f64,
                transform.scale.z as f64,
            ]),
            mint::Quaternion::from(transform.rotation.to_array().map(|x| x as f64)),
            mint::Vector3::from([
                transform.translation.x as f64,
                transform.translation.y as f64,
                transform.translation.z as f64,
            ]),
        );
        let Some((_, transforms)) = gizmo.interact(ui, &[transform]) else {
            continue;
        };
        let new = transforms[0];

        let mut transform = world.get_mut::<Transform>(selected).unwrap();
        *transform = Transform {
            translation: DVec3::from([new.translation.x, new.translation.y, new.translation.z])
                .as_vec3(),
            rotation: DQuat::from_array(<[f64; 4]>::from(new.rotation)).as_quat(),
            scale: DVec3::from([new.scale.x, new.scale.y, new.scale.z]).as_vec3(),
        };
    }
}

fn select_resource(
    ui: &mut egui::Ui,
    type_registry: &TypeRegistry,
    selection: &mut InspectorSelection,
) {
    let mut resources: Vec<_> = type_registry
        .iter()
        .filter(|registration| registration.data::<ReflectResource>().is_some())
        .map(|registration| {
            (
                registration.type_info().type_path_table().short_path(),
                registration.type_id(),
            )
        })
        .collect();
    resources.sort_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));

    for (resource_name, type_id) in resources {
        let selected = match *selection {
            InspectorSelection::Resource(selected, _) => selected == type_id,
            _ => false,
        };

        if ui.selectable_label(selected, resource_name).clicked() {
            *selection = InspectorSelection::Resource(type_id, resource_name.to_string());
        }
    }
}

fn select_asset(
    ui: &mut egui::Ui,
    type_registry: &TypeRegistry,
    world: &World,
    selection: &mut InspectorSelection,
) {
    let mut assets: Vec<_> = type_registry
        .iter()
        .filter_map(|registration| {
            let reflect_asset = registration.data::<ReflectAsset>()?;
            Some((
                registration.type_info().type_path_table().short_path(),
                registration.type_id(),
                reflect_asset,
            ))
        })
        .collect();
    assets.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));

    for (asset_name, asset_type_id, reflect_asset) in assets {
        let handles: Vec<_> = reflect_asset.ids(world).collect();

        ui.collapsing(format!("{asset_name} ({})", handles.len()), |ui| {
            for handle in handles {
                let selected = match *selection {
                    InspectorSelection::Asset(_, _, selected_id) => selected_id == handle,
                    _ => false,
                };

                if ui
                    .selectable_label(selected, format!("{handle:?}"))
                    .clicked()
                {
                    *selection =
                        InspectorSelection::Asset(asset_type_id, asset_name.to_string(), handle);
                }
            }
        });
    }
}

fn states(ui: &mut egui::Ui, world: &mut World) {
    ui.horizontal(|ui| {
        ui.label("Server:");
        ui.push_id(1, |ui| {
            bevy_inspector::ui_for_state::<ServerState>(world, ui)
        });
    });

    ui.horizontal(|ui| {
        ui.label("Game:");
        ui.push_id(2, |ui| bevy_inspector::ui_for_state::<GameState>(world, ui));
    });

    ui.horizontal(|ui| {
        ui.label("Course:");
        ui.push_id(3, |ui| {
            bevy_inspector::ui_for_state::<CourseState>(world, ui)
        });
    });

    ui.horizontal(|ui| {
        ui.label("Hole:");
        ui.push_id(4, |ui| bevy_inspector::ui_for_state::<HoleState>(world, ui));
    });
}

#[derive(Resource, Reflect, Debug)]
struct SceneLoaderState {
    path: String,
}

impl Default for SceneLoaderState {
    fn default() -> Self {
        SceneLoaderState {
            path: "courses/0002".to_owned(),
        }
    }
}

fn scene_loader(ui: &mut egui::Ui, world: &mut World) {
    let mut state = world.resource_mut::<SceneLoaderState>();

    ui.horizontal(|ui| {
        ui.label("Scene File:");
        ui.text_edit_singleline(&mut state.path);
    });

    ui.horizontal(|ui| {
        if ui.button("Load file").clicked() {
            return;
        }

        if ui.button("Save file").clicked() {
            save_scene(world);
        }
    });

    ui.horizontal(|ui| {
        if ui.button("Save configuration").clicked() {
            save_configuration(world);
        }
    });
}

fn save_configuration(world: &mut World) {
    let app_type_registry = world.resource::<AppTypeRegistry>();
    let type_registry = app_type_registry.read();

    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .allow_resource::<Configuration>()
        .extract_resources()
        .build();

    let serialized_scene = scene.serialize(&type_registry).unwrap();
    IoTaskPool::get()
        .spawn(async move {
            File::create("assets/config.scn.ron".to_string())
                .and_then(|mut file| file.write(serialized_scene.as_bytes()))
                .expect("Could not write to file");
        })
        .detach();
}

fn save_scene(world: &mut World) {
    world
        .run_system_once(crate::course::setup::capture_course_state)
        .unwrap();

    let state = world.resource::<SceneLoaderState>();
    let path = state.path.clone();
    let app_type_registry = world.resource::<AppTypeRegistry>();
    let type_registry = app_type_registry.read();

    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .allow_resource::<CourseConfiguration>()
        .extract_resources()
        .build();

    let serialized_scene = scene.serialize(&type_registry).unwrap();
    IoTaskPool::get()
        .spawn(async move {
            File::create(format!("assets/{path}.scn.ron"))
                .and_then(|mut file| file.write(serialized_scene.as_bytes()))
                .expect("Could not write to file");
        })
        .detach();
}
