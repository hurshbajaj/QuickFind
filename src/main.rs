use std::{io, env, fs, path::PathBuf, path::Path, time::Duration};
use crossterm::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui::{
    backend::CrosstermBackend,
    Terminal,

    widgets::{Block, Borders, Paragraph, List, ListItem, ListState, Clear},
    layout::{Layout, Constraint, Direction, Alignment, Rect},
    
    style::{Style, Color, Modifier},
    text::{Spans, Span},
};
use event::Event;

#[derive(Clone, PartialEq)]
enum PopupMode {
    None,
    CreateFile,
    CreateDir,
    Delete,

    Rename,
}

struct AppState {
    focus_dir: PathBuf,
    entries: Vec<String>,
    selected_index: usize,

    list_state: ListState,
    popup_mode: PopupMode,

    input_buffer: String,

    break_now: bool,
}

impl AppState {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let focus_dir = env::current_dir()?;
        let entries = read_entries(&focus_dir)?;
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Ok(AppState {
            focus_dir,
            entries,

            selected_index: 0,
            list_state,
            popup_mode: PopupMode::None,
            input_buffer: String::new(),
            break_now: false,
        })
    }

    fn refresh_entries(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.entries = read_entries(&self.focus_dir)?;
        if self.selected_index >= self.entries.len() && !self.entries.is_empty() {
            self.selected_index = self.entries.len() - 1;
        }
        self.list_state.select(Some(self.selected_index));
        Ok(())
    }

    fn get_selected_path(&self) -> Option<PathBuf> {
        self.entries.get(self.selected_index)
            .map(|entry| self.focus_dir.join(entry))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app_state = AppState::new()?;

    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    out.execute(terminal::Clear(terminal::ClearType::All))?;
    out.execute(cursor::Hide)?;

    let backend = CrosstermBackend::new(&mut out);
    let mut terminal = Terminal::new(backend)?;

    'outer: loop {
        if app_state.break_now {
            break 'outer;
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(85), Constraint::Percentage(15)].as_ref())
                .split(f.size());

            let list_items: Vec<ListItem> = app_state.entries.iter().map(|entry| {
                let entry_path = app_state.focus_dir.join(entry);
                let style = if entry_path.is_dir() {
                    Style::default().fg(Color::Rgb(144, 238, 144))
                } else {
                    Style::default().fg(Color::Green)
                };
                ListItem::new(entry.clone()).style(style)
            }).collect();

            let border_color = Color::Green;

            let list = List::new(list_items)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color)).title("CLI Navigation"))
                .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .highlight_symbol(" #  ");

            let help_text = vec![
                Spans::from(vec![
                    Span::styled("Navigation: ", Style::default().fg(Color::Yellow)),
                    Span::raw("↑/↓ Select | ←/→ Navigate | Enter Exit")
                ]),
                Spans::from(vec![
                    Span::styled("File Ops: ", Style::default().fg(Color::Cyan)),
                    Span::raw("N New File | Shift+N New Dir | D Delete"),
                ]),
                Spans::from(vec![
                    Span::raw("R Rename | Esc Cancel"),
                ]),
            ];

            let help_display = Paragraph::new(help_text)
                .style(Style::default().fg(border_color))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color)).title("Controls"))
                .alignment(Alignment::Left);

            let path_display = Paragraph::new(app_state.focus_dir.to_string_lossy())
                .style(Style::default().fg(border_color))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color)).title("Current Path"));

            let help_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .split(chunks[1]);

            f.render_stateful_widget(list, chunks[0], &mut app_state.list_state);
            f.render_widget(path_display, help_chunks[0]);
            f.render_widget(help_display, help_chunks[1]);

            if app_state.popup_mode != PopupMode::None {
                render_popup(f, &app_state);
            }
        })?;

        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
                handle_input(&mut app_state, code, modifiers)?;
            }
        }
    }

    let mut out_post = io::stdout();
    out_post.execute(terminal::Clear(terminal::ClearType::All))?;
    terminal::disable_raw_mode()?;
    out_post.execute(cursor::MoveTo(0, 0))?;
    out_post.execute(cursor::Show)?;

    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("echo cd '\"{}\"' | clip.exe", app_state.focus_dir.display()))
        .output()?;

    Ok(())
}

fn handle_input(app_state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) -> Result<(), Box<dyn std::error::Error>> {
    if app_state.popup_mode != PopupMode::None {
        handle_popup_input(app_state, code, modifiers)?;
    } else {
        handle_main_input(app_state, code, modifiers)?;
    }
    Ok(())
}

fn handle_main_input(app_state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) -> Result<(), Box<dyn std::error::Error>> {
    match code {
        KeyCode::Enter => {
            app_state.break_now = true;
        }
        KeyCode::Esc => app_state.break_now = true,
        KeyCode::Right => {
            if let Some(entry) = app_state.entries.get(app_state.selected_index) {
                let path_candidate = app_state.focus_dir.join(entry);
                if path_candidate.is_dir() {
                    app_state.focus_dir.push(Path::new(entry));
                    app_state.refresh_entries()?;
                    app_state.selected_index = 0;
                    app_state.list_state.select(Some(0));
                }
            }
        }
        KeyCode::Left => {
            app_state.focus_dir.pop();
            app_state.refresh_entries()?;
            app_state.selected_index = 0;
            app_state.list_state.select(Some(0));
        }
        KeyCode::Up => {
            if app_state.selected_index > 0 {
                app_state.selected_index -= 1;
                app_state.list_state.select(Some(app_state.selected_index));
            }
        }
        KeyCode::Down => {
            if app_state.selected_index + 1 < app_state.entries.len() {
                app_state.selected_index += 1;
                app_state.list_state.select(Some(app_state.selected_index));
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if modifiers.contains(KeyModifiers::SHIFT) {
                app_state.popup_mode = PopupMode::CreateDir;
            } else {
                app_state.popup_mode = PopupMode::CreateFile;
            }
            app_state.input_buffer.clear();
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if !app_state.entries.is_empty() {
                app_state.popup_mode = PopupMode::Delete;
                app_state.input_buffer.clear();
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if !app_state.entries.is_empty() {
                app_state.popup_mode = PopupMode::Rename;
                if let Some(current_name) = app_state.entries.get(app_state.selected_index) {
                    app_state.input_buffer = current_name.clone();
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_popup_input(app_state: &mut AppState, code: KeyCode, _modifiers: KeyModifiers) -> Result<(), Box<dyn std::error::Error>> {
    match code {
        KeyCode::Esc => {
            app_state.popup_mode = PopupMode::None;
            app_state.input_buffer.clear();
        }
        KeyCode::Enter => {
            execute_popup_action(app_state)?;
        }
        KeyCode::Backspace => {
            app_state.input_buffer.pop();
        }
        KeyCode::Char(c) => {
            app_state.input_buffer.push(c);
        }
        _ => {}
    }
    Ok(())
}

fn execute_popup_action(app_state: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
    match app_state.popup_mode {
        PopupMode::CreateFile => {
            if !app_state.input_buffer.trim().is_empty() {
                let file_path = app_state.focus_dir.join(&app_state.input_buffer);
                if !file_path.exists() {
                    fs::write(&file_path, "")?;
                }
            }
        }
        PopupMode::CreateDir => {
            if !app_state.input_buffer.trim().is_empty() {
                let dir_path = app_state.focus_dir.join(&app_state.input_buffer);
                if !dir_path.exists() {
                    fs::create_dir(&dir_path)?;
                }
            }
        }
        PopupMode::Delete => {
            if app_state.input_buffer.to_lowercase() == "y" || app_state.input_buffer.to_lowercase() == "yes" {
                if let Some(entry) = app_state.entries.get(app_state.selected_index) {
                    let target_path = app_state.focus_dir.join(entry);
                    if target_path.is_dir() {
                        fs::remove_dir_all(&target_path)?;
                    } else {
                        fs::remove_file(&target_path)?;
                    }
                }
            }
        }
        PopupMode::Rename => {
            if !app_state.input_buffer.trim().is_empty() {
                if let Some(old_name) = app_state.entries.get(app_state.selected_index) {
                    let old_path = app_state.focus_dir.join(old_name);
                    let new_path = app_state.focus_dir.join(&app_state.input_buffer);
                    if old_path != new_path && !new_path.exists() {
                        fs::rename(&old_path, &new_path)?;
                    }
                }
            }
        }
        PopupMode::None => {}
    }

    app_state.popup_mode = PopupMode::None;
    app_state.input_buffer.clear();
    app_state.refresh_entries()?;
    Ok(())
}

fn render_popup(f: &mut tui::Frame<CrosstermBackend<&mut io::Stdout>>, app_state: &AppState) {
    let size = f.size();
    let popup_area = centered_rect(50, 30, size);

    f.render_widget(Clear, popup_area);

    let (title, prompt) = match app_state.popup_mode {
        PopupMode::CreateFile => ("Create New File", "Enter filename:"),
        PopupMode::CreateDir => ("Create New Directory", "Enter directory name:"),
        PopupMode::Delete => {
            let empty_string = String::new();
            let selected_name = app_state.entries.get(app_state.selected_index).unwrap_or(&empty_string);
            return render_delete_popup(f, popup_area, selected_name, &app_state.input_buffer);
        },
        PopupMode::Rename => ("Rename Item", "Enter new name:"),
        PopupMode::None => ("", ""),
    };

    let popup_text = vec![
        Spans::from(vec![Span::raw(prompt)]),
        Spans::from(vec![Span::styled(&app_state.input_buffer, Style::default().fg(Color::Yellow))]),
        Spans::from(vec![]),
        Spans::from(vec![Span::styled("Press Enter to confirm, Esc to cancel", Style::default().fg(Color::Gray))]),
    ];

    let popup = Paragraph::new(popup_text)
        .block(Block::default().borders(Borders::ALL).title(title).style(Style::default().fg(Color::Cyan)))
        .alignment(Alignment::Left);

    f.render_widget(popup, popup_area);
}

fn render_delete_popup(f: &mut tui::Frame<CrosstermBackend<&mut io::Stdout>>, popup_area: Rect, selected_name: &str, input_buffer: &str) {
    let popup_text = vec![
        Spans::from(vec![Span::styled("WARNING: Delete item?", Style::default().fg(Color::Red))]),
        Spans::from(vec![]),
        Spans::from(vec![Span::raw("Item: "), Span::styled(selected_name, Style::default().fg(Color::Yellow))]),
        Spans::from(vec![]),
        Spans::from(vec![Span::raw("Type 'y' or 'yes' to confirm:")]),
        Spans::from(vec![Span::styled(">> ", Style::default().fg(Color::Red)), Span::styled(input_buffer, Style::default().fg(Color::Yellow))]),
        Spans::from(vec![]),
        Spans::from(vec![Span::styled("Press Esc to cancel", Style::default().fg(Color::Gray))]),
    ];

    let popup = Paragraph::new(popup_text)
        .block(Block::default().borders(Borders::ALL).title("Delete Confirmation").style(Style::default().fg(Color::Red)))
        .alignment(Alignment::Left);

    f.render_widget(popup, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn read_entries(dir: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut entries = fs::read_dir(dir)?
        .into_iter()
        .filter_map(|x| x.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<String>>();
    
    entries.sort();
    Ok(entries)
}

