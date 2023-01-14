use std::path::PathBuf;

pub enum AppEvent {
    Exit,
}

pub enum AppFileEvent {
    Open(PathBuf),
}