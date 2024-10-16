use std::{error::Error, fs, io, thread};
use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use chrono::{Local, NaiveDate, Utc};
use dotenv::dotenv;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::{
    app::{App, CurrentlyEditing, CurrentScreen},
    ui::ui,
};
use crate::HostToggleOption::{BLOCK, UNBLOCK};

mod app;
mod ui;

const CONFIG_FILE_PATH: &str = "config.toml";
const HOST_FILE_LOCAL_PREFIX: &str = "127.0.0.1\t";
const HOST_FILE_BLOCK_PREFIX: &str = "#";
const HOST_FILE_LOCAL_PREFIX_DISABLED: &str = "#127.0.0.1\t";
const HOST_FILE_COMMIT_BLOCK_BEGIN: &str = "### CommitBlock";
const HOST_FILE_COMMIT_BLOCK_END: &str = "### End CommitBlock";
const HOST_FILE_PATH: &str = "tmp/test";
const STATE_FILE_PATH: &str = "tmp/state_file.json";
const QUIT_KEY: char = 'q';
const INSERT_KEY: char = 'i';
const DATE_FORMATTER: &str = "%Y-%m-%d";
const GRAPHQL_QUERY: &str = r#"
        query($userName:String!) {
          user(login: $userName){
            contributionsCollection {
              contributionCalendar {
                totalContributions
                weeks {
                  contributionDays {
                    contributionCount
                    date
                  }
                }
              }
            }
          }
        }
       "#;

struct RequestModel {
    path: String,
    token: String,
    body: Value,
}

#[derive(Serialize, Deserialize)]
struct ContributionState {
    threshold_met_date: Option<String>,
    threshold_met_goal: Option<u32>,
}

#[derive(Deserialize)]
struct Config {
    github_username: String,
    contribution_goal: u32,
}

/// Used to signify whether to block or unblock the list of configured hosts
enum HostToggleOption {
    BLOCK,
    UNBLOCK,
}

// #[tokio::main]
fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let existing_pairs = initialise_host_pairs();
    let app = Arc::new(Mutex::new(App::new(existing_pairs)));
    let (tx, rx): (mpsc::Sender<u32>, Receiver<u32>) = mpsc::channel();

    thread::spawn(move || {
        loop {
            let configuration = load_config(CONFIG_FILE_PATH);
            let mut state = load_contribution_state(STATE_FILE_PATH).unwrap_or_else(|| ContributionState {
                threshold_met_date: None,
                threshold_met_goal: None,
            });

            let today = Local::now().date_naive();
            if let Some(stored_date) = &state.threshold_met_date {
                let stored_date = NaiveDate::parse_from_str(stored_date, DATE_FORMATTER).unwrap();

                // * If the goal has been met earlier than today, reset the state
                // * If the goal has been met today, but the configuration has been updated to increase
                // the contribution target, reset the state
                if stored_date < today || state.threshold_met_goal.unwrap() < configuration.contribution_goal {
                    state.threshold_met_date = None;
                    state.threshold_met_goal = None;
                    modify_hosts(BLOCK).expect("TODO: panic message");

                } else {
                    if tx.send(100).is_err() { // Mark the progress bar as complete
                        break;
                    }
                    thread::sleep(Duration::from_secs(30));
                    continue;
                }
            }

            let contribution_progress = check_contribution_progress(state, today, configuration);

            if tx.send(contribution_progress).is_err() {
                break; // Exit if the receiver has been dropped
            }

            thread::sleep(Duration::from_secs(5));
        }
    });
    let mut app = app.lock().unwrap();
    run_app(&mut terminal, &mut app, rx).expect("TODO: panic message");

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    terminal.show_cursor()?;

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App, rx: Receiver<u32>) -> io::Result<bool> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        match rx.try_recv() {
            Ok(contribution_progress) => {
                app.progress = contribution_progress;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Ok(false);
            }
        }

        // TODO clean this up
        if event::poll(Duration::from_millis(10))? {
            if let Ok(evt) = event::read() {
                match evt {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Release {
                            continue;
                        }

                        match app.current_screen {
                            CurrentScreen::Main => match key.code {
                                KeyCode::Char(INSERT_KEY) => {
                                    app.current_screen = CurrentScreen::Editing;
                                    app.currently_editing = Some(CurrentlyEditing::Key);
                                }
                                KeyCode::Char(QUIT_KEY) => {
                                    app.current_screen = CurrentScreen::Exiting;
                                }
                                KeyCode::Char('h') => {
                                    app.current_screen = CurrentScreen::Help;
                                }
                                _ => {}
                            },
                            CurrentScreen::Exiting => match key.code {
                                KeyCode::Char('y') | KeyCode::Char(QUIT_KEY) => {
                                    return Ok(true); // Exit the app
                                }
                                KeyCode::Char('n') => {
                                    app.current_screen = CurrentScreen::Main; // Return to main
                                }
                                _ => {}
                            },
                            CurrentScreen::Editing => {
                                match key.kind {
                                    KeyEventKind::Press => {
                                        match key.code {
                                            KeyCode::Enter => {
                                                if let Some(editing) = &app.currently_editing {
                                                    match editing {
                                                        CurrentlyEditing::Key => {
                                                            app.currently_editing = Some(CurrentlyEditing::Value);
                                                        }
                                                        CurrentlyEditing::Value => {
                                                            app.save_key_value();
                                                            if let Err(e) = save_to_host(app.pairs.clone()) {
                                                                panic!("{}", e.to_string());
                                                            }
                                                            app.current_screen = CurrentScreen::Main;
                                                        }
                                                    }
                                                }
                                            }
                                            KeyCode::Backspace => {
                                                if let Some(editing) = &app.currently_editing {
                                                    match editing {
                                                        CurrentlyEditing::Key => {
                                                            app.key_input.pop();
                                                        }
                                                        CurrentlyEditing::Value => {
                                                            app.value_input = Some(false);
                                                        }
                                                    }
                                                }
                                            }
                                            KeyCode::Esc => {
                                                app.current_screen = CurrentScreen::Main;
                                                app.currently_editing = None;
                                            }
                                            KeyCode::Tab => {
                                                app.toggle_editing();
                                            }
                                            KeyCode::Char(value) => {
                                                if let Some(editing) = &app.currently_editing {
                                                    match editing {
                                                        CurrentlyEditing::Key => {
                                                            app.key_input.push(value);
                                                        }
                                                        CurrentlyEditing::Value => {
                                                            match value {
                                                                't' => app.value_input = Some(true),
                                                                'f' => app.value_input = Some(false),
                                                                _ => {}
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            CurrentScreen::Help => {
                                match key.code {
                                    _ => {
                                        app.current_screen = CurrentScreen::Main
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn check_contribution_progress(mut state: ContributionState, date: NaiveDate, configuration: Config) -> u32 {
    let client = reqwest::blocking::Client::new();

    let request_info = build_request_model(configuration.github_username);

    let response = client
        .post(&request_info.path)
        .header(header::USER_AGENT, "AppName/0.1")
        .bearer_auth(&request_info.token)
        .json(&request_info.body)
        .send()
        .unwrap().text().unwrap();

    let contribution_count = find_contribution_count_today(response).unwrap();

    if contribution_count >= configuration.contribution_goal {
        state.threshold_met_date = Some(date.format(DATE_FORMATTER).to_string());
        modify_hosts(UNBLOCK).expect("TODO: panic message");
        persist_contribution_state(&state).expect("TODO: panic message");
    }

    ((contribution_count as f32 / configuration.contribution_goal as f32) * 100.0).round() as u32
}

fn load_contribution_state(file_path: &str) -> Option<ContributionState> {
    let file = File::open(file_path).ok()?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).ok()
}

fn persist_contribution_state(state: &ContributionState) -> io::Result<()> {
    let file = OpenOptions::new().write(true).create(true).truncate(true).open(STATE_FILE_PATH)?;
    serde_json::to_writer_pretty(file, state).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

fn load_config(file_path: &str) -> Config {
    let config_str = fs::read_to_string(file_path)
        .expect("Failed to read config file.");
    toml::from_str(&config_str)
        .expect("Failed to parse config file.")
}

fn build_request_model(username: String) -> RequestModel {
    dotenv().ok();
    let token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN not set");

    let body = serde_json::json!({
        "query": GRAPHQL_QUERY,
        "variables": {
            "userName": username
        }
    });
    let path = String::from("https://api.github.com/graphql");

    RequestModel {
        path,
        token,
        body,
    }
}

fn find_contribution_count_today(api_response: String) -> Result<u32, ()> {
    let json_response: Value = serde_json::from_str(&api_response).unwrap();

    let today = Utc::now().format(DATE_FORMATTER).to_string();

    if let Some(contributions) = json_response["data"]["user"]["contributionsCollection"]
        ["contributionCalendar"]["weeks"]
        .as_array()
    {
        for week in contributions {
            if let Some(days) = week["contributionDays"].as_array() {
                for day in days {
                    if day["date"] == today {
                        let contribution_count = day["contributionCount"]
                            .as_i64()
                            .unwrap_or(0); // Default to 0 if not found
                        return Ok(contribution_count as u32);
                    }
                }
            }
        }
    }
    Ok(0)  // Return 0 if no contribution for today
}

fn initialise_host_pairs() -> HashMap<String, bool> {
    // TODO better handling
    let hosts = File::open(HOST_FILE_PATH).unwrap();
    let reader = BufReader::new(hosts);

    let mut inside_commit_block = false;
    let mut pairs: HashMap<String, bool> = HashMap::new();

    for line in reader.lines() {
        let line = line.unwrap();

        if line == HOST_FILE_COMMIT_BLOCK_BEGIN {
            inside_commit_block = true;
            continue;
        } else if line == HOST_FILE_COMMIT_BLOCK_END {
            break;
        }

        if inside_commit_block {
            if line.starts_with(HOST_FILE_BLOCK_PREFIX) {
                let trimmed = line.strip_prefix(HOST_FILE_LOCAL_PREFIX_DISABLED).unwrap_or(&line).parse().unwrap();
                pairs.insert(trimmed, false);
            } else {
                let trimmed = line.strip_prefix(HOST_FILE_LOCAL_PREFIX).unwrap_or(&line).parse().unwrap();
                pairs.insert(trimmed, true);
            }
        }
    }

    pairs
}

/// Saves the current host configuration to the host files
fn save_to_host(pairs: HashMap<String, bool>) -> Result<(), io::Error> {
    let mut hosts = File::open(HOST_FILE_PATH)?;
    let mut hosts_content = String::new();
    hosts.read_to_string(&mut hosts_content)?;

    let before_block = hosts_content.lines()
        .take_while(|s| !s.starts_with(HOST_FILE_COMMIT_BLOCK_BEGIN));
    let after_block = hosts_content.lines()
        .skip_while(|s| !s.starts_with(HOST_FILE_COMMIT_BLOCK_END))
        .skip(1);

    let mut new_hosts = String::new();
    for line in before_block.chain(after_block) {
        new_hosts.push_str(line);
        new_hosts.push_str("\n")
    };

    new_hosts.push_str("### CommitBlock\n");
    for domain in pairs {
        let block_marker = match domain.1 {
            true => "",
            false => HOST_FILE_BLOCK_PREFIX,
        };
        new_hosts.push_str(block_marker);
        new_hosts.push_str(HOST_FILE_LOCAL_PREFIX);
        new_hosts.push_str(&*domain.0);
        new_hosts.push_str("\n");
    };
    new_hosts.push_str("### End CommitBlock\n");

    let mut file = File::create(HOST_FILE_PATH)?;
    file.write_all(new_hosts.as_bytes())?;
    Ok(())
}

fn modify_hosts(toggle_option: HostToggleOption) -> Result<(), io::Error> {
    let file = File::open(HOST_FILE_PATH)?;
    let reader = BufReader::new(file);

    let mut in_commitblock = false;
    let mut output = Vec::new();

    for line in reader.lines() {
        let line = line?;

        if line.trim() == HOST_FILE_COMMIT_BLOCK_BEGIN {
            in_commitblock = true;
        } else if line.trim() == HOST_FILE_COMMIT_BLOCK_END {
            in_commitblock = false;
        }

        if in_commitblock {
            match toggle_option {
                BLOCK => {
                    if line != HOST_FILE_COMMIT_BLOCK_BEGIN {
                        output.push(line.strip_prefix(HOST_FILE_BLOCK_PREFIX).unwrap_or(&line).to_string())
                    } else {
                        output.push(line.clone());
                    }
                }
                UNBLOCK => {
                    if !line.trim().starts_with(HOST_FILE_BLOCK_PREFIX) {
                        output.push(format!("#{}", line));
                    } else {
                        output.push(line.clone());
                    }
                }
            }
        } else {
            output.push(line.clone());
        }
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(HOST_FILE_PATH)?;

    for line in output {
        writeln!(file, "{}", line)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Failed to read config file.")]
    fn load_config_file_not_found_panic() {
        load_config("doesNotExist.toml");
    }

    #[test]
    #[should_panic(expected = "Failed to parse config file.")]
    fn load_config_file_not_toml_panic() {
        load_config(".gitignore");
    }
}