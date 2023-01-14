#![allow(dead_code)]
#![allow(unused_imports)]

// Hide console if release build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod events;

use bevy::{
    asset::{AssetIo, AssetIoError, AssetPath, Metadata},
    prelude::*,
    utils::BoxedFuture,
};
use bevy_asset_loader::prelude::*;
use bevy_fly_camera::{FlyCamera, FlyCameraPlugin};
use bevy_infinite_grid::{GridShadowCamera, InfiniteGridBundle, InfiniteGrid, InfiniteGridPlugin};
use events::*;
use std::path::{Path, PathBuf};

fn main() {
    App::new()
        //.insert_resource(ClearColor(Color::BLACK))
        .add_event::<AppEvent>()
        .add_event::<AppFileEvent>()
        .insert_resource(Msaa { samples: 4 })
        .add_startup_system(setup)
        .add_system(drop_files)
        .add_system(model_system)
        .add_plugins(
            DefaultPlugins
                .build()
                .add_before::<bevy::asset::AssetPlugin, _>(CustomAssetIoPlugin),
        )
        .add_plugin(FlyCameraPlugin)
        .add_plugin(InfiniteGridPlugin)
        .add_state(MyStates::Next)
        .add_loading_state(
            LoadingState::new(MyStates::AssetLoading)
                .continue_to_state(MyStates::Next)
                .with_collection::<ModelAsset>()
        )
        .add_system_set(
            SystemSet::on_enter(MyStates::Next)
                .with_system(asset_loaded),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    let camera = Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        ..Camera3dBundle::default()
    };

    commands.spawn(camera).insert(FlyCamera {
        enabled: true,
        sensitivity: 3.0,
        ..Default::default()
    }).insert(GridShadowCamera);

    // Infinite grid
    commands.spawn(InfiniteGridBundle {
        grid: InfiniteGrid {
            fadeout_distance: 300.,
            shadow_color: None, // No shadow
            ..InfiniteGrid::default()
        },
        visibility: Visibility {
            is_visible: true,
        },
        ..InfiniteGridBundle::default()
    });
}

fn drop_files(
    mut commands: Commands,
    mut drag_drop_events: EventReader<FileDragAndDrop>,
    mut _file_event_writer: EventWriter<AppFileEvent>,
    mut dynamic_assets: ResMut<DynamicAssets>,
    mut asset_server: ResMut<AssetServer>,
) {
    for d in drag_drop_events.iter() {
        if let FileDragAndDrop::DroppedFile { id: _, path_buf } = d {
            println!("Dropped \"{}\"", path_buf.to_str().unwrap());

            //file_event_writer.send(AppFileEvent::Open(path_buf.to_owned()));

            //let path_buf = PathBuf::from(format!("{}#Scene0", path_buf.to_str().unwrap()));

            commands.spawn(SceneBundle {
                scene: asset_server.load(AssetPath::new(path_buf.to_owned(), None)),
                ..Default::default()
            });
        }
    }
}

fn model_system(
    mut commands: Commands,
    models: Res<Assets<bevy::gltf::Gltf>>,
    models_query: Query<(Entity, &Handle<bevy::gltf::Gltf>)>,
    //mut materials: Query<&mut StandardMaterial>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {

    for (id, gl) in models.iter() {
        //println!("Loaded {} meshes", gl.meshes.len());

        commands.spawn(SceneBundle {
            scene: gl.scenes[0].clone(),
            ..Default::default()
        });
    }

    /*for visibility in mesh_entities.iter_mut() {
        //let mesh = meshes.get(id).unwrap();

        if let Some(mut vis) = visibility {
            vis.is_visible = true;
        }
    }*/

    //if let Some(gltf)

    for (_, model) in models.iter() {
        for id in model.materials.iter() {
            let mat = materials.get_mut(id).unwrap();
            mat.unlit = true;
        }
    }
}

fn asset_loaded() {

}

#[derive(AssetCollection, Resource)]
struct ModelAsset {
    #[asset(key = "model")]
    model: Handle<bevy::gltf::Gltf>,
}


#[derive(Clone, Eq, PartialEq, Debug, Hash)]
enum MyStates {
    AssetLoading,
    Next,
}

struct CustomAssetIo(Box<dyn AssetIo>);

impl AssetIo for CustomAssetIo {
    fn load_path<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<Vec<u8>, AssetIoError>> {
        info!("load_path({:?})", path);
        self.0.load_path(path)
    }

    fn read_directory(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>, AssetIoError> {
        info!("read_directory({:?})", path);
        self.0.read_directory(path)
    }

    fn watch_path_for_changes(&self, path: &Path) -> Result<(), AssetIoError> {
        info!("watch_path_for_changes({:?})", path);
        self.0.watch_path_for_changes(path)
    }

    fn watch_for_changes(&self) -> Result<(), AssetIoError> {
        info!("watch_for_changes()");
        self.0.watch_for_changes()
    }

    fn get_metadata(&self, path: &Path) -> Result<Metadata, AssetIoError> {
        info!("get_metadata({:?})", path);
        self.0.get_metadata(path)
    }
}

struct CustomAssetIoPlugin;

impl Plugin for CustomAssetIoPlugin {
    fn build(&self, app: &mut App) {
        let default_io = AssetPlugin::default().create_platform_default_asset_io();

        // create the custom asset io instance
        let asset_io = CustomAssetIo(default_io);

        // the asset server is constructed and added the resource manager
        app.insert_resource(AssetServer::new(asset_io));
    }
}