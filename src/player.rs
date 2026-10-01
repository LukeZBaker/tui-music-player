use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

use gstreamer::{glib, prelude::*};
use gstreamer_pbutils::Discoverer;

pub enum PlaybackEvent {
    Error(String),
    Track(PathBuf),
}

pub struct Player {
    playbin: gstreamer::Element,
    playlist: Rc<RefCell<Vec<PathBuf>>>,
    current: Rc<Cell<usize>>,
    playing: Cell<bool>,
}

impl Player {
    pub fn new() -> Result<Self, String> {
        let playbin = gstreamer::ElementFactory::make("playbin")
            .build()
            .map_err(|_| {
                "GStreamer playbin is unavailable; install the playback plugins".to_string()
            })?;
        let playlist = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
        let current = Rc::new(Cell::new(0));
        playbin
            .connect(
                "about-to-finish",
                false,
                glib::clone!(
                    #[strong]
                    playlist,
                    #[strong]
                    current,
                    move |values| {
                        let playbin = values[0]
                            .get::<gstreamer::Element>()
                            .expect("signal has an element");
                        let next = current.get() + 1;
                        if let Some(path) = playlist.borrow().get(next) {
                            if let Ok(uri) = glib::filename_to_uri(path, None) {
                                current.set(next);
                                playbin.set_property("uri", uri);
                            }
                        }
                        None
                    }
                ),
            )
            .map_err(|e| format!("Could not enable gapless playback: {e}"))?;
        Ok(Self {
            playbin,
            playlist,
            current,
            playing: Cell::new(false),
        })
    }

    pub fn play_from(&self, tracks: Vec<PathBuf>, index: usize) -> Result<(), String> {
        let path = tracks
            .get(index)
            .ok_or_else(|| "That track is no longer available".to_string())?;
        validate_audio(path)?;
        let uri =
            glib::filename_to_uri(path, None).map_err(|e| format!("Invalid filename: {e}"))?;
        self.playbin
            .set_state(gstreamer::State::Null)
            .map_err(|e| format!("Could not stop previous track: {e}"))?;
        *self.playlist.borrow_mut() = tracks;
        self.current.set(index);
        self.playbin.set_property("uri", uri);
        self.playbin
            .set_state(gstreamer::State::Playing)
            .map_err(|e| format!("Could not start playback: {e}"))?;
        self.playing.set(true);
        Ok(())
    }

    pub fn toggle(&self) -> bool {
        let should_play = !self.playing.get();
        if self
            .playbin
            .set_state(if should_play {
                gstreamer::State::Playing
            } else {
                gstreamer::State::Paused
            })
            .is_ok()
        {
            self.playing.set(should_play);
        }
        self.playing.get()
    }

    pub fn next(&self) -> Result<(), String> {
        self.move_by(1)
    }
    pub fn previous(&self) -> Result<(), String> {
        self.move_by(-1)
    }

    fn move_by(&self, offset: isize) -> Result<(), String> {
        let index = self
            .current
            .get()
            .checked_add_signed(offset)
            .ok_or_else(|| "No more tracks".to_string())?;
        self.play_from(self.playlist.borrow().clone(), index)
    }

    pub fn attach_events(&self, notify: impl Fn(PlaybackEvent) + 'static) {
        let Some(bus) = self.playbin.bus() else {
            return;
        };
        let current = self.current.clone();
        let playlist = self.playlist.clone();
        let _ = bus.add_watch_local(move |_, message| {
            use gstreamer::MessageView;
            match message.view() {
                MessageView::Error(error) => {
                    notify(PlaybackEvent::Error(error.error().to_string()))
                }
                MessageView::StreamStart(_) => {
                    if let Some(path) = playlist.borrow().get(current.get()) {
                        notify(PlaybackEvent::Track(path.clone()));
                    }
                }
                _ => {}
            }
            glib::ControlFlow::Continue
        });
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.playbin.set_state(gstreamer::State::Null);
    }
}

fn validate_audio(path: &PathBuf) -> Result<(), String> {
    let uri = glib::filename_to_uri(path, None).map_err(|e| format!("Invalid filename: {e}"))?;
    let discoverer = Discoverer::new(gstreamer::ClockTime::from_seconds(5))
        .map_err(|e| format!("Could not create media validator: {e}"))?;
    let info = discoverer
        .discover_uri(&uri)
        .map_err(|e| format!("Not a playable audio file: {e}"))?;
    let has_audio = !info.audio_streams().is_empty();
    if has_audio {
        Ok(())
    } else {
        Err("The selected file contains no audio stream".to_string())
    }
}
