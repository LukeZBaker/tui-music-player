mod player;

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use player::{PlaybackEvent, Player};

const APP_ID: &str = "io.github.tempo.Player";

fn main() -> glib::ExitCode {
    if let Err(error) = gstreamer::init() {
        eprintln!("Could not initialize GStreamer: {error}");
        return glib::ExitCode::FAILURE;
    }

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

#[derive(Clone)]
struct Browser {
    directory: Rc<RefCell<PathBuf>>,
    tracks: Rc<RefCell<Vec<PathBuf>>>,
    list: gtk::ListBox,
    path_label: gtk::Label,
    status: gtk::Label,
    player: Rc<Player>,
}

impl Browser {
    fn open(&self, directory: PathBuf) {
        match read_directory(&directory) {
            Ok(entries) => {
                while let Some(child) = self.list.first_child() {
                    self.list.remove(&child);
                }
                self.path_label.set_label(&directory.to_string_lossy());
                *self.directory.borrow_mut() = directory.clone();

                if let Some(parent) = directory.parent() {
                    self.add_row("..", "go-up-symbolic", Some(parent.to_owned()), None);
                }

                let mut tracks = Vec::new();
                for entry in entries {
                    let name = entry
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Invalid filename");
                    if entry.is_dir() {
                        self.add_row(name, "folder-symbolic", Some(entry), None);
                    } else {
                        let index = tracks.len();
                        self.add_row(name, "audio-x-generic-symbolic", None, Some(index));
                        tracks.push(entry);
                    }
                }
                *self.tracks.borrow_mut() = tracks;
                self.status.set_label("Choose a track to play");
            }
            Err(error) => self.show_error(&format!("Could not open folder: {error}")),
        }
    }

    fn add_row(&self, title: &str, icon: &str, directory: Option<PathBuf>, track: Option<usize>) {
        let row = adw::ActionRow::builder()
            .title(title)
            .activatable(true)
            .build();
        row.add_prefix(&gtk::Image::from_icon_name(icon));
        if directory.is_some() {
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        }
        row.set_data("directory", directory);
        row.set_data("track", track);
        self.list.append(&row);
    }

    fn activate(&self, row: &gtk::ListBoxRow) {
        let Some(row) = row.downcast_ref::<adw::ActionRow>() else {
            return;
        };
        if let Some(Some(path)) =
            unsafe { row.data::<Option<PathBuf>>("directory") }.map(|p| p.as_ref())
        {
            self.open(path.clone());
            return;
        }
        if let Some(Some(index)) = unsafe { row.data::<Option<usize>>("track") }.map(|p| *p) {
            let tracks = self.tracks.borrow().clone();
            match self.player.play_from(tracks, index) {
                Ok(()) => self.status.set_label(&format!("Playing {}", row.title())),
                Err(error) => self.show_error(&error),
            }
        }
    }

    fn show_error(&self, message: &str) {
        self.status.set_label(message);
        self.status.add_css_class("error");
    }
}

fn build_ui(app: &adw::Application) {
    let player = match Player::new() {
        Ok(player) => Rc::new(player),
        Err(error) => {
            eprintln!("{error}");
            app.quit();
            return;
        }
    };
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    let path_label = gtk::Label::builder()
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .hexpand(true)
        .build();
    let status = gtk::Label::builder()
        .label("Choose a folder")
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .hexpand(true)
        .build();
    let play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    let next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
    let previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
    let open = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Open folder")
        .build();

    let header = adw::HeaderBar::new();
    header.pack_start(&open);
    header.set_title_widget(Some(&path_label));
    let controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Center)
        .margin_top(9)
        .margin_bottom(9)
        .build();
    controls.append(&previous);
    controls.append(&play);
    controls.append(&next);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&header);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    content.append(&scroll);
    content.append(&status);
    content.append(&controls);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Tempo")
        .default_width(620)
        .default_height(700)
        .content(&content)
        .build();
    let initial = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let browser = Browser {
        directory: Rc::new(RefCell::new(initial.clone())),
        tracks: Rc::new(RefCell::new(Vec::new())),
        list,
        path_label,
        status: status.clone(),
        player: player.clone(),
    };
    browser.open(initial);

    browser.list.connect_row_activated(glib::clone!(
        #[strong]
        browser,
        move |_, row| browser.activate(row)
    ));
    open.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[strong]
        browser,
        move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Choose a music folder")
                .modal(true)
                .build();
            dialog.select_folder(
                Some(&window),
                gio::Cancellable::NONE,
                glib::clone!(
                    #[strong]
                    browser,
                    move |result| {
                        if let Ok(folder) = result {
                            if let Some(path) = folder.path() {
                                browser.open(path);
                            }
                        }
                    }
                ),
            );
        }
    ));
    play.connect_clicked(glib::clone!(
        #[strong]
        player,
        move |button| {
            let playing = player.toggle();
            button.set_icon_name(if playing {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            });
        }
    ));
    next.connect_clicked(glib::clone!(
        #[strong]
        player,
        move |_| {
            let _ = player.next();
        }
    ));
    previous.connect_clicked(glib::clone!(
        #[strong]
        player,
        move |_| {
            let _ = player.previous();
        }
    ));
    player.attach_events(move |event| match event {
        PlaybackEvent::Error(error) => status.set_label(&format!("Playback error: {error}")),
        PlaybackEvent::Track(path) => status.set_label(&format!(
            "Playing {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )),
    });
    window.present();
}

fn read_directory(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let iterator = std::fs::read_dir(directory).map_err(|e| e.to_string())?;
    let mut directories = Vec::new();
    let mut music = Vec::new();
    for item in iterator {
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        let path = item.path();
        let kind = match item.file_type() {
            Ok(kind) => kind,
            Err(_) => continue,
        };
        if kind.is_dir() {
            directories.push(path);
        } else if kind.is_file() && is_music_path(&path) {
            music.push(path);
        }
    }
    directories.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
    music.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
    directories.extend(music);
    Ok(directories)
}

fn is_music_path(path: &Path) -> bool {
    const AUDIO_EXTENSIONS: &[&str] = &[
        "aac", "aiff", "alac", "flac", "m4a", "mp3", "oga", "ogg", "opus", "wav", "wma",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn music_extension_filter_is_case_insensitive() {
        assert!(is_music_path(Path::new("song.FLAC")));
        assert!(is_music_path(Path::new("song.mp3")));
        assert!(!is_music_path(Path::new("cover.jpg")));
        assert!(!is_music_path(Path::new("notes.txt")));
    }
}
