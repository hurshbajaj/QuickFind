use std::{io, env, fs, path::PathBuf, path::Path, time::Duration};
use crossterm::*;
use crossterm::event::{KeyCode, KeyEvent};
use tui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::{Block, Borders, Paragraph, List, ListItem, ListState},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color, Modifier},
};
use event::Event;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut change_dir = false;
    let mut break_now = false;
    let mut i = 0;
    let mut state = ListState::default();

    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    out.execute(terminal::Clear(terminal::ClearType::All))?;
    out.execute(cursor::Hide)?;

    let backend = CrosstermBackend::new(&mut out);
    let mut terminal = Terminal::new(backend)?;

    let mut focus_dir = env::current_dir().expect("Could not get current directory");
    let mut entries: Vec<String> = read_entries(&focus_dir)?;

    'outer: loop {
        if break_now {
            break 'outer;
        }

        let focus_dir_display = focus_dir.clone();

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
                .split(f.size());

            let list_items: Vec<ListItem> = entries.iter().map(|entry| {
                let entry_path = focus_dir.join(entry);
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

            let input_display = Paragraph::new(focus_dir_display.to_string_lossy())
                .style(Style::default().fg(border_color))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color)).title("Path"));

            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
                    match code {
                        KeyCode::Enter => {
                            if let Some(raw_entry) = entries.get(i) {
                                let path_candidate = focus_dir.join(raw_entry);
                                if path_candidate.is_dir() {
                                    focus_dir.push(Path::new(raw_entry));
                                    entries = read_entries(&focus_dir).unwrap_or_default();
                                    i = 0;
                                } else {
                                    break_now = true;
                                    change_dir = true;
                                }
                            }
                        }
                        KeyCode::Esc => break_now = true,
                        KeyCode::Right => {
                            if let Some(raw_entry) = entries.get(i) {
                                let path_candidate = focus_dir.join(raw_entry);
                                if path_candidate.is_dir() {
                                    focus_dir.push(Path::new(raw_entry));
                                    entries = read_entries(&focus_dir).unwrap_or_default();
                                    i = 0;
                                }
                            }
                        }
                        KeyCode::Left => {
                            focus_dir.pop();
                            entries = read_entries(&focus_dir).unwrap_or_default();
                            i = 0;
                        }
                        KeyCode::Up => {
                            if i > 0 { i -= 1; }
                        }
                        KeyCode::Down => {
                            if i + 1 < entries.len() { i += 1; }
                        }
                        _ => {}
                    }
                }
            }

            state.select(Some(i));
            f.render_widget(input_display, chunks[1]);
            f.render_stateful_widget(list, chunks[0], &mut state);
        })?;
    }

    let mut out_post = io::stdout();
    out_post.execute(terminal::Clear(terminal::ClearType::All))?;
    terminal::disable_raw_mode()?;
    out_post.execute(cursor::MoveTo(0, 0))?;
    out_post.execute(cursor::Show)?;

    if change_dir {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("echo cd '\"{}\"' | clip.exe", focus_dir.display()))
            .output()?;
    }

    Ok(())
}

fn read_entries(dir: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let entries = fs::read_dir(dir)?
        .into_iter()
        .filter_map(|x| x.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<String>>();
    Ok(entries)
}

