use crate::apps::SubApp;
use crate::helpers::*;
use crate::models::*;
use clap::Parser;
use grim::Platform;
use grim::SystemInfo;
use grim::dta::DataArray;
use grim::dta::DataString;
use grim::io::*;
use grim::midi::{MidiFile, MidiTrack, MidiEvent, MidiTextType, MidiText};
use grim::scene::*;
use log::{debug, error, info, warn};
use serde::Deserialize;
use serde_json::Deserializer;
use std::collections::HashMap;
use std::error::Error;
use std::fs::OpenOptions;
use std::fs::{copy, create_dir_all, File, read, remove_dir_all, write};
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

// TODO: Rename to something like 'compile' or 'build'
#[derive(Parser, Debug)]
pub struct Project2MiloApp {
    #[clap(name = "dir_path", help = "Path to input project directory", required = true)]
    pub input_path: String,
    #[clap(name = "output_path", help = "Path to build output", required = true)]
    pub output_path: String,
    #[clap(short, long, help = "Enable to leave output milo archive(s) uncompressed", required = false)]
    pub uncompressed: bool,
    #[clap(short, long, help = "Platform (ps3, wii, x360)", required = false, default_value = "x360")]
    pub platform: String
}

impl SubApp for Project2MiloApp {
    fn process(&mut self) -> Result<(), Box<dyn Error>> {
        let input_dir = PathBuf::from(&self.input_path);
        if !input_dir.exists() {
            // TODO: Throw proper error
            error!("Input directory {:?} doesn't exist", input_dir);
            return Ok(())
        }

        // Open song file
        let song_json_path = input_dir.join("song.json");
        let song_json = read(song_json_path)?;
        let song = serde_json::from_slice::<P9Song>(song_json.as_slice())?;

        let game_abbr = if song.preferences.is_gdrb() { "GDRB" } else { "TBRB" };
        info!("Loading song project for {game_abbr}...");

        //dbg!(&song);

        // Get lipsync file(s)
        let mut lipsyncs = get_lipsync(&input_dir.join("lipsync").as_path(), song.preferences.is_gdrb());

        // Load venue midi
        let mut prop_anim = load_midi(&input_dir, song.preferences.is_gdrb());

        // Create song prefs file
        let song_pref = create_song_pref(&song);

        // TODO: Is "LightPreset.pst" needed?

        // Create milos files...

        // Write everything?
        let sys_info = SystemInfo {
            version: 25,
            platform: Platform::PS3,
            endian: IOEndian::Big,
        };

        let output_dir = PathBuf::from(&self.output_path);
        if !output_dir.exists() {
            // Create outout path if it doesn't exist
            create_dir_all(&output_dir).expect("Failed to create output directory");
        }

        // Get platform ext
        let platform_ext = match self.platform.to_ascii_lowercase().as_str() {
            "ps3" => "ps3",
            "wii" => "wii",
            _ => "xbox"
        };

        // Name, object dir init
        let object_dirs = [
            (format!("{}_ap.milo_{platform_ext}", &song.name), {
                let mut obj_dir = create_object_dir_for_song(&song.name, &sys_info);

                let entries = obj_dir.get_entries_mut();
                entries.push(song_pref);

                if let Some(prop_anim) = prop_anim.take() {
                    entries.push(prop_anim);
                }

                obj_dir
            }),
            (format!("{}.milo_{platform_ext}", &song.name), {
                let mut obj_dir = create_object_dir_for_lipsync(&sys_info);

                // Add lipsync files
                obj_dir.get_entries_mut().append(&mut lipsyncs);

                obj_dir
            }),
        ];

        for (file_name, object_dir) in object_dirs {
            let block_type = match self.uncompressed {
                true => BlockType::TypeA,
                _ => BlockType::TypeB
            };

            let archive = MiloArchive::from_object_dir(&object_dir, &sys_info, Some(block_type))?;

            // Write to file
            let milo_path = output_dir.join(&file_name);
            let mut stream = FileStream::from_path_as_read_write_create(&milo_path)?;
            archive.write_to_stream(&mut stream)?;

            info!("Wrote \"{file_name}\"")
        }

        Ok(())
    }
}

fn create_song_pref(song: &P9Song) -> Object {
    let song_pref = match &song.preferences {
        SongPreferences::GDRB(prefs) => P9SongPref {
            name: String::from("P9SongPref"),
            venue: prefs.venue.to_string(),
            instruments: [
                prefs.mike_instruments.to_owned(),
                prefs.billie_instruments.to_owned(),
                Vec::new(),
                prefs.tre_instruments.to_owned()
            ],
            tempo: prefs.tempo.to_owned(),
            song_clips: prefs.song_clips.to_owned(),
            normal_outfit: prefs.normal_outfit.to_string(),
            bonus_outfit: prefs.bonus_outfit.to_string(),
            drum_set: prefs.drum_set.to_owned(),
            era: prefs.era.to_owned(),
            song_intro_cam: prefs.song_intro_cam.to_owned(),
            win_cam: prefs.win_cam.to_owned(),
            ..Default::default()
        },
        _ => todo!()
    };

    Object::P9SongPref(song_pref)
}

fn get_lipsync(lipsync_dir: &Path, is_gdrb: bool) -> Vec<Object> {
    const GDRB_LIPSYNC_NAMES: [&str; 4] = [
        "song.lipsync",
        "billiejoe.lipsync",
        "mikedirnt.lipsync",
        "trecool.lipsync"
    ];

    const TBRB_LIPSYNC_NAMES: [&str; 4] = [
        "george.lipsync",
        "john.lipsync",
        "paul.lipsync",
        "ringo.lipsync"
    ];

    let lipsyncs = lipsync_dir
        .find_files_with_depth(FileSearchDepth::Immediate)
        .unwrap_or_default()
        .into_iter()
        .filter(|lip| lip
            .file_name()
            .and_then(|f| f.to_str())
            .map(|p| p.ends_with(".lipsync"))
            .unwrap_or_default())
        .collect::<Vec<_>>();

    if lipsyncs.is_empty() {
        warn!("No lipsync files found in {:?}", lipsync_dir);
        return Vec::new();
    }

    // Validate lipsync file names
    let lipsync_names = if is_gdrb { &GDRB_LIPSYNC_NAMES } else { &TBRB_LIPSYNC_NAMES };

    for lipsync_file in lipsyncs.iter() {
        let file_name = lipsync_file.file_name().and_then(|f| f.to_str()).unwrap();

        info!("Found \"{}\"", &file_name);

        let mut is_valid = false;

        for name in lipsync_names.iter() {
            if file_name.eq(*name) {
                is_valid = true;
                break;
            }
        }

        if !is_valid {
            warn!("Lipsync with file name \"{file_name}\" is invalid. Expected: {lipsync_names:?}");
        }
    }

    // Get byte data for lipsync files
    lipsyncs
        .iter()
        .map(|lip_path| {
            let mut buffer = Vec::new();

            let mut file = File::open(lip_path).expect(format!("Can't open {:?}", lip_path).as_str());
            file.read_to_end(&mut buffer).expect(format!("Can't read data from {:?}", lip_path).as_str());

            let file_name = lip_path.file_name().and_then(|f| f.to_str()).unwrap();

            Object::Packed(PackedObject {
                name: file_name.to_string(),
                object_type: String::from("CharLipSync"),
                data: buffer,
            })
        })
        .collect()
}

fn load_midi(project_dir: &Path, is_gdrb: bool) -> Option<Object> {
    const GDRB_CHARACTERS: [(&str, &str); 3] = [
        ("BILLIE", "billiejoe"),
        ("MIKE", "mikedirnt"),
        ("TRE", "trecool"),
    ];

    const TBRB_CHARACTERS: [(&str, &str); 4] = [
        ("GEORGE", "george"),
        ("JOHN", "john"),
        ("PAUL", "paul"),
        ("RINGO", "ringo"),
    ];

    // Open midi
    let mid_path = project_dir.join("venue.mid"); // TODO: Check if midi exists first?
    if !mid_path.exists() {
        // TODO: Throw proper error. Not sure if should halt process though...
        error!("Can't find \"venue.mid\"");
        return None;
    }

    let mid = MidiFile::from_path(mid_path).unwrap();
    let mut prop_keys = Vec::new();

    // Parse venue track
    let venue_track_name = if is_gdrb { "VENUE GDRB" } else { "VENUE" };
    let venue_track = mid.get_track_with_name(venue_track_name);
    if let Some(track) = venue_track {
        let mut keys = load_venue_track(track, is_gdrb);
        prop_keys.append(&mut keys);
    } else {
        warn!("Track \"{venue_track_name}\" not found in midi");
    }

    // Parse each character
    let mut char_loaded = false;
    let char_track_names = if is_gdrb { GDRB_CHARACTERS.as_slice() } else { TBRB_CHARACTERS.as_slice() };
    for (char_track_name, long_name) in char_track_names.iter() {
        let char_track = mid.get_track_with_name(char_track_name);

        if let Some(track) = char_track {
            let mut keys = load_char_track(track, long_name, is_gdrb);
            prop_keys.append(&mut keys);

            char_loaded = true;
        }
    }

    if !char_loaded {
        let char_track_names = char_track_names.iter().map(|(n, _)| n).collect::<Vec<_>>();
        warn!("No character anim tracks found in midi. Expected: {char_track_names:?}");
    }

    Some(Object::PropAnim(PropAnim {
        name: String::from("song.anim"),
        type2: String::from("song_anim"),
        note: format!("Generated by {} v{}", super::PKG_NAME, super::VERSION),
        keys: prop_keys,
        ..Default::default()
    }))
}

fn load_track(track: &MidiTrack, properties: &[(&str, u32, Option<&str>, u32, fn() -> PropKeysEvents)]) -> Vec<PropKeys> {
    let mut prop_keys = HashMap::new(); // property -> keys
    let track_name = track.name.as_ref().map(|n| n.as_str()).unwrap_or("???");

    for ev in track.events.iter() {
        let (_pos, _pos_realtime, text) = match ev {
            MidiEvent::Meta(MidiText { pos, pos_realtime, text: MidiTextType::Event(text), .. }) => (*pos, pos_realtime.unwrap(), std::str::from_utf8(text).ok()),
            _ => continue,
        };

        let text = if let Some(txt) = text {
            txt
        } else {
            // TODO: Output warning and midi timestamp/realtime pos
            continue;
        };

        let parsed_text = if let Some(parsed) = FormattedAnimEvent::try_from_str(text) { parsed } else { continue; };
        let property = parsed_text.get_property();

        if !prop_keys.contains_key(property) {
            // Validate property
            match properties.iter().find(|(p, ..)| p.eq(&parsed_text.get_property())) {
                Some((property, interpolation, interp_handler, unk_enum, init_events)) => {
                    // Create and insert new prop key
                    prop_keys.insert(*property, PropKeys {
                        target: String::from("P9Director"), // Note: Implicitly P9Director
                        property: vec![
                            DataArray::Symbol(DataString::from_string(property.to_string()))
                        ],
                        interpolation: *interpolation,
                        interp_handler: interp_handler
                            .map(|h| h.to_string())
                            .unwrap_or_default(),
                            unknown_enum: *unk_enum,
                            events: init_events()
                    });
                },
                _ => {
                    // Property not supported
                    // TODO: Show time in log
                    warn!("Event for property \"{property}\" is not supported");
                    continue;
                }
            };
        }

        let key = prop_keys.get_mut(property).unwrap();
        let pos = ((ev.get_pos_realtime().unwrap() * 30.) / 1000.) as f32; // TODO: Probably make fps a variable

        match &mut key.events {
            PropKeysEvents::Float(evs) => {
                let anim_ev = AnimEventFloat {
                    pos,
                    value: match parsed_text.try_parse_values::<1, f32>() {
                        [Some(f)] => f,
                        _ => {
                            // TODO: Show position
                            warn!("Unable to parse \"{}\"", parsed_text.get_text());
                            continue;
                        }
                    }
                };

                evs.push(anim_ev);
            },
            PropKeysEvents::Color(evs) => {
                let color = match parsed_text.try_parse_values::<4, f32>() {
                    [Some(r), Some(g), Some(b), Some(a)] => Color4 { r, g, b, a },
                    _ => {
                        // TODO: Show position
                        warn!("Unable to parse \"{}\"", parsed_text.get_text());
                        continue;
                    }
                };

                let anim_ev = AnimEventColor {
                    pos,
                    value: color
                };

                evs.push(anim_ev);
            },
            PropKeysEvents::Object(evs) => {
                let values = parsed_text.get_values();
                let parsed_values = [ values.get(0), values.get(1) ];

                let mut anim_ev = AnimEventObject {
                    pos,
                    ..Default::default()
                };

                match parsed_values {
                    [Some(v1), Some(v2)] => {
                        // First value is usually reserved for milo directory name
                        anim_ev.text1 = v1.to_string();
                        anim_ev.text2 = v2.to_string();
                    },
                    [Some(v1), ..] => {
                        anim_ev.text2 = v1.to_string();
                    },
                    _ => {
                        // Treat empty array as null symbol
                        // So do nothing. Maybe further validate symbol syntax?
                    }
                }

                evs.push(anim_ev);
            },
            PropKeysEvents::Bool(evs) => {
                let anim_ev = AnimEventBool {
                    pos,
                    value: match parsed_text.get_values().get(0) {
                        Some(&"TRUE") => true,
                        Some(&"FALSE") => false,
                        _ => {
                            // TODO: Show position
                            warn!("Unable to parse \"{}\"", parsed_text.get_text());
                            continue;
                        }
                    }
                };

                evs.push(anim_ev);
            },
            PropKeysEvents::Quat(evs) => {
                let quat = match parsed_text.try_parse_values::<4, f32>() {
                    [Some(x), Some(y), Some(z), Some(w)] => Quat { x, y, z, w },
                    _ => {
                        // TODO: Show position
                        warn!("Unable to parse \"{}\"", parsed_text.get_text());
                        continue;
                    }
                };

                let anim_ev = AnimEventQuat {
                    pos,
                    value: quat
                };

                evs.push(anim_ev);
            },
            PropKeysEvents::Vector3(evs) => {
                let parsed_values = parsed_text.try_parse_values::<3, f32>();

                let vector3 = match parsed_values {
                    [Some(x), Some(y), Some(z)] => Vector3 { x, y, z },
                    _ => {
                        // TODO: Show position
                        warn!("Unable to parse \"{}\"", parsed_text.get_text());
                        continue;
                    }
                };

                let anim_ev = AnimEventVector3 {
                    pos,
                    value: vector3
                };

                evs.push(anim_ev);
            },
            PropKeysEvents::Symbol(evs) => {
                let anim_ev = AnimEventSymbol {
                    pos,
                    text: parsed_text // Treat empty array as null symbol
                        .get_values()
                        .get(0)
                        .map(|s| s.to_string())
                        .unwrap_or_default()
                };

                evs.push(anim_ev);
            },
        }
    }

    let keys = prop_keys.into_values().collect::<Vec<_>>();

    let (property_count, event_count) = keys
        .iter()
        .fold(
            (0, 0),
            |(pc, ec), key| (pc + 1, ec + key.events.len())
        );

    info!("[{track_name:>10}] Loaded {event_count:>4} events for {property_count:>2} properties");

    keys
}

fn load_venue_track(track: &MidiTrack, is_gdrb: bool) -> Vec<PropKeys> {
    // Property, interpolation, interp_handler, unknown_enum, type
    const GDRB_PROPERTIES_VENUE: [(&str, u32, Option<&str>, u32, fn() -> PropKeysEvents); 30] = [
        ("configuration",                 0, None,                    6, init_events_symbol),
        ("crash_ignore_triggers",         0, None,                    0, init_events_bool),
        ("crowd_anim_override",           0, None,                    0, init_events_symbol),
        ("crowd_extras_command",          0, None,                    0, init_events_symbol),
        ("crowd_preset",                  0, None,                    0, init_events_symbol),
        ("floortom_ignore_triggers",      0, None,                    0, init_events_bool),
        ("hihat_clip",                    0, None,                    0, init_events_symbol),
        ("hihat_ignore_triggers",         0, None,                    0, init_events_bool),
        ("jumbotron_post_proc",           0, None,                    0, init_events_symbol),
        ("jumbotron_shot",                0, None,                    0, init_events_symbol),
        ("kick_ignore_triggers",          0, None,                    0, init_events_bool),
        ("left_crash_clip",               0, None,                    0, init_events_symbol),
        ("left_crash_ignore_triggers",    0, None,                    0, init_events_bool),
        ("left_crash_weight",             1, None,                    0, init_events_float),
        ("left_floortom_ignore_triggers", 0, None,                    0, init_events_bool),
        ("left_foot_ignore_triggers",     0, None,                    0, init_events_bool),
        ("left_tom_ignore_triggers",      0, None,                    0, init_events_bool),
        ("lighting_preset",               0, None,                    0, init_events_symbol),
        ("lighting_preset_modifier",      0, None,                    0, init_events_symbol),
        ("mic_stand_visibility",          0, None,                    0, init_events_symbol),
        ("postproc",                      0, Some("postproc_interp"), 5, init_events_object),
        ("postproc_blending_enabled",     0, None,                    0, init_events_bool),
        ("ride_clip",                     0, None,                    0, init_events_symbol),
        ("ride_ignore_triggers",          0, None,                    0, init_events_bool),
        ("right_crash_clip",              0, None,                    0, init_events_symbol),
        ("right_crash_ignore_triggers",   0, None,                    0, init_events_bool),
        ("right_tom_ignore_triggers",     0, None,                    0, init_events_bool),
        ("shot",                          0, None,                    0, init_events_symbol),
        ("snare_ignore_triggers",         0, None,                    0, init_events_bool),
        ("trigger_group",                 0, None,                    0, init_events_symbol),
    ];

    let venue_properties = if is_gdrb { GDRB_PROPERTIES_VENUE.as_slice() } else { todo!("TBRB venue not supported right now") };

    load_track(track, venue_properties)
}

fn load_char_track(track: &MidiTrack, char_name: &str, is_gdrb: bool) -> Vec<PropKeys> {
    // Property, interpolation, interp_handler, unknown_enum, type
    const GDRB_PROPERTIES_CHARS: [(&str, u32, Option<&str>, u32, fn() -> PropKeysEvents); 20] = [
        ("add_face_clip",            0, None, 0, init_events_symbol),
        ("add_face_weight",          4, None, 0, init_events_float),
        ("attention",                0, None, 0, init_events_symbol),
        ("body",                     0, None, 0, init_events_symbol),
        ("brow_clip",                0, None, 0, init_events_symbol),
        ("brow_clip_b",              0, None, 0, init_events_symbol),
        ("brow_clip_balance",        1, None, 0, init_events_float),
        ("brow_weight",              1, None, 0, init_events_float),
        ("face_clip",                0, None, 0, init_events_symbol),
        ("face_clip_b",              0, None, 0, init_events_symbol),
        ("face_clip_balance",        4, None, 0, init_events_float),
        ("face_weight",              4, None, 0, init_events_float),
        ("hist_clip",                0, None, 0, init_events_symbol),
        ("lid_clip",                 0, None, 0, init_events_symbol),
        ("lid_clip_b",               0, None, 0, init_events_symbol),
        ("lid_clip_balance",         1, None, 0, init_events_float),
        ("lid_weight",               1, None, 0, init_events_float),
        ("lookat",                   1, None, 0, init_events_float),
        ("procedural_blink_enabled", 0, None, 0, init_events_bool),
        ("vox_clone_enabled",        0, None, 0, init_events_bool),
    ];

    let char_properties = if is_gdrb { GDRB_PROPERTIES_CHARS.as_slice() } else { todo!("TBRB characters not supported right now") };

    let mut prop_keys = load_track(track, char_properties);

    // Rename properties for specific char
    for prop_key in prop_keys.iter_mut() {
        let property = prop_key.property.first_mut().map(|p| match p {
            DataArray::Symbol(s) => s,
            _ => panic!("Shouldn't be hit")
        }).unwrap();

        let transformed_value = match property.as_utf8().unwrap() {
            "procedural_blink_enabled" => format!("procedural_blink_{}_enabled", char_name),
            "vox_clone_enabled" => format!("vox_clone_{}_enabled", char_name),
            default @ _ => format!("{}_{}", default, char_name)
        };

        *property = DataString::from_string(transformed_value);
    }

    prop_keys
}

fn init_events_bool() -> PropKeysEvents {
    PropKeysEvents::Bool(Vec::new())
}

fn init_events_float() -> PropKeysEvents {
    PropKeysEvents::Float(Vec::new())
}

fn init_events_object() -> PropKeysEvents {
    PropKeysEvents::Object(Vec::new())
}

fn init_events_symbol() -> PropKeysEvents {
    PropKeysEvents::Symbol(Vec::new())
}

fn create_object_dir_for_song(name: &str, info: &SystemInfo) -> ObjectDir {
    let mut obj_dir = ObjectDirBase {
        name: name.to_string(),
        ..ObjectDirBase::new()
    };

    let dir_entry = create_object_dir_entry(
        &format!("{name}_ap"),
        "song",
        &[
            "../../world/shared/director.milo",
            "../../world/shared/camera.milo",
            &format!("{name}.milo") // Lipsync milo
        ],
        info
    );

    obj_dir.entries.push(dir_entry.unwrap()); // Uhh... shouldn't fail. All this will be refactored anyways

    ObjectDir::ObjectDir(obj_dir)
}

fn create_object_dir_for_lipsync(info: &SystemInfo) -> ObjectDir {
    let mut obj_dir = ObjectDirBase {
        name: String::from("lipsync"),
        ..ObjectDirBase::new()
    };

    let dir_entry = create_object_dir_entry(
        "lipsync", // Great naming there, HMX
        "",
        &[],
        info
    );

    obj_dir.entries.push(dir_entry.unwrap()); // Uhh... shouldn't fail. All this will be refactored anyways

    ObjectDir::ObjectDir(obj_dir)
}

fn create_object_dir_entry(name: &str, obj_type: &str, subdir_paths: &[&str], info: &SystemInfo) -> Result<Object, Box<dyn Error>> {
    // Create stream
    let mut data = Vec::<u8>::new();
    let mut stream = MemoryStream::from_vector_as_read_write(&mut data);
    let mut writer = BinaryStream::from_stream_with_endian(&mut stream, info.endian);

    // Version, revision, type
    writer.write_int32(22)?;
    writer.write_int32(2)?;
    writer.write_prefixed_string(obj_type)?;

    // Viewports
    const VIEWPORT_COUNT: i32 = 7;
    let mat = Matrix::indentity();
    writer.write_int32(VIEWPORT_COUNT)?;

    for _ in 0..VIEWPORT_COUNT {
        writer.write_float32(mat.m11)?;
        writer.write_float32(mat.m12)?;
        writer.write_float32(mat.m13)?;

        writer.write_float32(mat.m21)?;
        writer.write_float32(mat.m22)?;
        writer.write_float32(mat.m23)?;

        writer.write_float32(mat.m31)?;
        writer.write_float32(mat.m32)?;
        writer.write_float32(mat.m33)?;

        writer.write_float32(mat.m41)?;
        writer.write_float32(mat.m42)?;
        writer.write_float32(mat.m43)?;
    }
    writer.write_int32(0)?; // Current viewport index

    // Inline proxy, proxy file
    writer.write_boolean(true)?;
    writer.write_prefixed_string("")?;

    // Subdir count, subdirs
    writer.write_int32(subdir_paths.len() as i32)?;
    for subdir_path in subdir_paths {
        writer.write_prefixed_string(subdir_path)?;
    }

    // Inline subdir, inline subdir count
    writer.write_boolean(false)?;
    writer.write_int32(0)?;

    // Unknown strings
    writer.write_prefixed_string("")?;
    writer.write_prefixed_string("")?;

    // Props (dta). Just ignore for now
    writer.write_boolean(false)?;

    // Note
    let note = format!("Generated by {} v{}", super::PKG_NAME, super::VERSION);
    writer.write_prefixed_string(&note)?;

    Ok(Object::Packed(PackedObject {
        name: name.to_string(),
        object_type: String::from("ObjectDir"),
        data
    }))
}
