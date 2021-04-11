// Hide console if release build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::{prelude::*, render::camera::PerspectiveProjection};
use bevy_egui::{egui, EguiContext, EguiPlugin};
use bevy_fly_camera::{FlyCamera, FlyCameraPlugin};

fn main() {
    App::build()
        .insert_resource(Msaa { samples: 8 })
        .insert_resource(WindowDescriptor {
            title: String::from("Preview"),
            width: 1920.0,
            height: 1080.0,
            vsync: true,
            resizable: true,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_plugin(FlyCameraPlugin)
        .add_system(ui_example.system())
        .add_system(control_camera.system())
        .add_startup_system(setup.system())
        .run();
}

fn ui_example(mut egui_ctx: ResMut<EguiContext>, mut event_writer: EventWriter<bevy::app::AppExit>) {
    let ctx = &mut egui_ctx.ctx();

    // Toolbar
    egui::TopPanel::top("top_panel").show(ctx, |ui| {
        // ui.heading("Main");

        egui::menu::bar(ui, |ui| {
            // File dropdown
            egui::menu::menu(ui, "File", |ui| {
                ui.set_min_width(80.0);

                ui.button("Open");
                ui.separator();
                ui.button("Save");
                ui.button("Save As...");
                ui.separator();
                ui.button("Close");
                ui.separator();
                if ui.button("Exit").clicked() {
                    // Close app
                    event_writer.send(bevy::app::AppExit);
                }
            });

            // Edit dropdown
            egui::menu::menu(ui, "Edit", |ui| {
                ui.set_min_width(80.0);

                ui.button("Undo");
                ui.button("Redo");
            });

            // Tools dropdown
            egui::menu::menu(ui, "Tools", |ui| {
                ui.set_min_width(80.0);

                ui.button("Options");
            });

            // Help dropdown
            egui::menu::menu(ui, "Help", |ui| {
                ui.set_min_width(120.0);

                ui.button("About");
                ui.separator();
                ui.button("Check for Updates");
            });
        });
    });

    // Side panel
    /* egui::SidePanel::left("side_panel", 500.0).show(ctx, |ui| {
        ui.set_min_width(400.0);
        ui.heading("Options");
    });*/

    // Hide menu shadow
    let mut style: egui::Style = (*ctx.style()).clone();
    let shadow_color = style.visuals.window_shadow.color.clone();
    style.visuals.window_shadow.color = shadow_color.linear_multiply(0.0);
    ctx.set_style(style);

    egui::Window::new("Hello").show(ctx, |ui| {
        // let mut style = ui.style_mut();
        // style.visuals.code_bg_color = style.visuals.code_bg_color.linear_multiply(0.1);

        ui.label("world");
    });
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 5.0 })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..Default::default()
    });
    // cube
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..Default::default()
    });
    // light
    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });
    // camera
    let mut camera = PerspectiveCameraBundle::new_3d();
    camera.transform = Transform::from_xyz(-2.0, 2.5, 5.0)
        .looking_at(Vec3::ZERO, Vec3::Y);

    commands.spawn_bundle(camera).insert(FlyCamera::default());
}

fn control_camera(
    mouse_input: Res<Input<MouseButton>>,
    egui_ctx: Res<bevy_egui::EguiContext>,
    mut cam_query: Query<&mut FlyCamera>,
) {
    for mut cam in cam_query.iter_mut() {
        cam.enabled = !egui_ctx.ctx().wants_pointer_input() && mouse_input.pressed(MouseButton::Middle);
    }
}