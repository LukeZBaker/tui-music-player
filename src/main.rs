use std::{
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    DefaultTerminal, Frame,
};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

fn main() -> Result<()> {
    let mut app = App::new(std::env::current_dir()?)?;
    let terminal = init_terminal()?;
    let run_result = app.run(terminal);
    restore_terminal()?;
    run_result
}

fn init_terminal() -> Result<DefaultTerminal> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    ratatui::init()
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen).context("failed to leave alternate screen")?;
    ratatui::restore();
    Ok(())
}

#[derive(Clone)]
struct BrowserEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Sink,
    now_playing: Option<PathBuf>,
    queue: Vec<PathBuf>,
}

impl Player {
    fn new() -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("no output audio device available")?;
        let sink = Sink::try_new(&handle).context("failed to initialize playback sink")?;

        Ok(Self {
            _stream: stream,
            handle,
            sink,
            now_playing: None,
            queue: Vec::new(),
        })
    }

    fn stop(&mut self) -> Result<()> {
        self.sink.stop();
        self.sink = Sink::try_new(&self.handle).context("failed to reset playback sink")?;
        self.now_playing = None;
        self.queue.clear();
        Ok(())
    }

    fn toggle_pause(&self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    fn play_gapless_playlist(&mut self, playlist: Vec<PathBuf>) -> Result<()> {
        if playlist.is_empty() {
            return Ok(());
        }

        self.stop()?;
        self.now_playing = playlist.first().cloned();
        self.queue = playlist.clone();

        for path in playlist {
            self.enqueue(path)?;
        }

        Ok(())
    }

    fn enqueue(&mut self, path: PathBuf) -> Result<()> {
        let file = File::open(&path)
            .with_context(|| format!("failed to open audio file: {}", path.display()))?;
        let source = Decoder::new(BufReader::new(file))
            .with_context(|| format!("failed to decode audio file: {}", path.display()))?;
        self.sink.append(source);
        Ok(())
    }

    fn status_line(&self) -> String {
        let now = self
            .now_playing
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Nothing playing".to_string());

        format!(
            "Now: {now} | queued tracks: {} | paused: {}",
            self.sink.len(),
            self.sink.is_paused()
        )
    }
}

struct App {
    current_dir: PathBuf,
    entries: Vec<BrowserEntry>,
    selected: usize,
    player: Player,
    status: String,
}

impl App {
    fn new(start_dir: PathBuf) -> Result<Self> {
        let mut app = Self {
            current_dir: start_dir,
            entries: Vec::new(),
            selected: 0,
            player: Player::new()?,
            status: String::new(),
        };
        app.reload_entries()?;
        Ok(app)
    }

    fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(120))? {
                let ev = event::read()?;
                if let Event::Key(key) = ev {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                        KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                        KeyCode::Enter | KeyCode::Char('l') => self.enter_selected()?,
                        KeyCode::Backspace | KeyCode::Char('h') => self.go_up()?,
                        KeyCode::Char(' ') => self.player.toggle_pause(),
                        KeyCode::Char('s') => {
                            self.player.stop()?;
                            self.status = "Playback stopped".to_string();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let header = Paragraph::new(format!("Directory: {}", self.current_dir.display()))
            .block(Block::default().title("Browser").borders(Borders::ALL));
        frame.render_widget(header, layout[0]);

        self.render_entries(frame, layout[1]);

        let now_playing = Paragraph::new(self.player.status_line())
            .block(Block::default().title("Playback").borders(Borders::ALL));
        frame.render_widget(now_playing, layout[2]);

        let footer = Paragraph::new(Line::from(vec![
            Span::raw("↑/↓ or j/k: move   "),
            Span::raw("Enter/l: open/play   "),
            Span::raw("Backspace/h: up   "),
            Span::raw("Space: pause/resume   "),
            Span::raw("s: stop   q: quit"),
        ]))
        .block(
            Block::default()
                .title(self.status.as_str())
                .borders(Borders::ALL),
        );
        frame.render_widget(footer, layout[3]);
    }

    fn render_entries(&self, frame: &mut Frame<'_>, area: Rect) {
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|entry| {
                if entry.is_dir {
                    ListItem::new(Line::from(vec![Span::styled(
                        format!("📁 {}", entry.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    )]))
                } else {
                    ListItem::new(format!("🎵 {}", entry.name))
                }
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().title("Files").borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut list_state = ListState::default();
        if !self.entries.is_empty() {
            list_state.select(Some(self.selected));
        }
        frame.render_stateful_widget(list, area, &mut list_state);
    }

    fn reload_entries(&mut self) -> Result<()> {
        let mut entries: Vec<BrowserEntry> = std::fs::read_dir(&self.current_dir)
            .with_context(|| format!("failed to read directory: {}", self.current_dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = path.is_dir();
                if is_dir || is_audio_file(&path) {
                    Some(BrowserEntry { name, path, is_dir })
                } else {
                    None
                }
            })
            .collect();

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.status = format!("{} entries", self.entries.len());
        Ok(())
    }

    fn enter_selected(&mut self) -> Result<()> {
        let Some(selected) = self.entries.get(self.selected).cloned() else {
            return Ok(());
        };

        if selected.is_dir {
            self.current_dir = selected.path;
            self.selected = 0;
            self.reload_entries()?;
            return Ok(());
        }

        let playlist = self.collect_audio_playlist_from_current(selected.path.as_path());
        if playlist.is_empty() {
            self.status = "No playable files in this folder".to_string();
            return Ok(());
        }

        self.player.play_gapless_playlist(playlist)?;
        self.status = "Playing folder as gapless playlist".to_string();
        Ok(())
    }

    fn collect_audio_playlist_from_current(&self, start_at: &Path) -> Vec<PathBuf> {
        let files: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.path.clone())
            .collect();

        let idx = files.iter().position(|p| p == start_at).unwrap_or(0);
        let (head, tail) = files.split_at(idx);
        tail.iter().chain(head.iter()).cloned().collect()
    }

    fn go_up(&mut self) -> Result<()> {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.selected = 0;
            self.reload_entries()?;
        }
        Ok(())
    }

    fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn move_selection_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }
}

fn is_audio_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };

    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp3" | "flac" | "wav" | "ogg" | "m4a"
    )
}
