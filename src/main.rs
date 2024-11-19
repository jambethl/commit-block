use std::{error::Error, fs, io, thread};
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
    app::{App, CurrentScreen},
    ui::ui,
};
use crate::app::EditingConfigField;
use crate::HostToggleOption::{BLOCK, UNBLOCK};

mod app;
mod ui;

const CONFIG_FILE_PATH: &str = "config.toml";
const HOST_FILE_LOCAL_PREFIX_IP4: &str = "127.0.0.1\t";
const HOST_FILE_LOCAL_PREFIX_IP6: &str = "::1\t\t";
const HOST_FILE_BLOCK_PREFIX: &str = "#";
const HOST_FILE_LOCAL_PREFIX_DISABLED_IP4: &str = "#127.0.0.1\t";
const HOST_FILE_LOCAL_PREFIX_DISABLED_IP6: &str = "#::1\t\t";
const HOST_FILE_COMMIT_BLOCK_BEGIN: &str = "### CommitBlock";
const HOST_FILE_COMMIT_BLOCK_END: &str = "### End CommitBlock";
const HOST_FILE_PATH: &str = "/etc/hosts";
const STATE_FILE_PATH: &str = "tmp/state_file.json";
const QUIT_KEY: char = 'q';
const INSERT_KEY: char = 'i';
const HELP_KEY: char = 'h';
const CONFIGURATION_KEY: char = 'c';
const DATE_FORMATTER: &str = "%Y-%m-%d";
const GH_API_PATH: &str = "https://api.github.com/graphql";
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
    token: String,
    body: Value,
}

#[derive(Serialize, Deserialize)]
struct ContributionThresholdStatus {
    threshold_met_date: Option<String>,
    threshold_met_goal: Option<u32>,
}

#[derive(Deserialize, Serialize)]
struct Config {
    github_username: String,
    contribution_goal: u32,
}

/// Used to signify whether to block or unblock the list of configured hosts
enum HostToggleOption {
    BLOCK,
    UNBLOCK,
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let app = init_app();
    let (tx, rx): (mpsc::Sender<u32>, Receiver<u32>) = mpsc::channel();

    thread::spawn(move || {
        loop {
            let configuration = load_config(CONFIG_FILE_PATH);
            let mut state = load_contribution_state(STATE_FILE_PATH).unwrap_or_else(|| ContributionThresholdStatus {
                threshold_met_date: None,
                threshold_met_goal: None,
            });

            let today = Local::now().date_naive();
            if let Some(stored_date) = &state.threshold_met_date {
                let stored_date = NaiveDate::parse_from_str(stored_date, DATE_FORMATTER).unwrap();

                // * If the goal has been met earlier than today, reset the state
                // * If the goal has been met today, but the configuration has been updated to increase
                // the contribution target, reset the state
                if stored_date < today || state.threshold_met_goal.unwrap_or(0) < configuration.contribution_goal {
                    state.threshold_met_date = None;
                    state.threshold_met_goal = None;
                    modify_hosts(BLOCK).expect("TODO: panic message");
                } else {
                    let contribution_count = check_contribution_progress(&configuration);
                    if contribution_count >= configuration.contribution_goal.clone() {
                        record_contribution_goal_met(today, state, &configuration);
                    }
                    if tx.send(contribution_count).is_err() { // Mark the progress bar as complete
                        break;
                    }
                    thread::sleep(Duration::from_secs(30));
                    continue;
                }
            }

            let contribution_count = check_contribution_progress(&configuration);
            if contribution_count >= configuration.contribution_goal.clone() {
                record_contribution_goal_met(today, state, &configuration);
            }

            if tx.send(contribution_count).is_err() {
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

    Ok(())
}

fn init_app() -> Arc<Mutex<App>> {
    let existing_hosts = initialise_hosts();

    let configuration = load_config(CONFIG_FILE_PATH);
    let contribution_goal = configuration.contribution_goal;
    let username = configuration.github_username.clone();

    let state = load_contribution_state(STATE_FILE_PATH).unwrap_or_else(|| ContributionThresholdStatus {
        threshold_met_date: None,
        threshold_met_goal: None,
    });
    let threshold_met_date = state.threshold_met_date.clone();
    let threshold_met_goal = state.threshold_met_goal;

    Arc::new(Mutex::new(App::new(
        existing_hosts,
        0, // This might not be accurate, but will be corrected by the other thread which is calling GH. Initialising to 0 allows the app to startup instantly instead of waiting for an external response
        contribution_goal,
        username,
        threshold_met_date,
        threshold_met_goal)))
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
                                    app.currently_editing = true;
                                }
                                KeyCode::Char(QUIT_KEY) => {
                                    app.current_screen = CurrentScreen::Exiting;
                                }
                                KeyCode::Char(HELP_KEY) => {
                                    app.current_screen = CurrentScreen::Help;
                                }
                                KeyCode::Char(CONFIGURATION_KEY) => {
                                    app.current_screen = CurrentScreen::Configuration;
                                    app.editing_config_field = Some(EditingConfigField::ContributionGoal);
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
                                            KeyCode::Up => {
                                                if app.selected_index > 0 {
                                                    app.selected_index -= 1;
                                                }
                                            }
                                            KeyCode::Down => {
                                                if app.selected_index < app.hosts.len() - 1 {
                                                    app.selected_index += 1;
                                                }
                                            }
                                            KeyCode::Enter => {
                                                if app.currently_editing {
                                                    app.save_new_host();
                                                    if let Err(e) = save_to_host(app.hosts.clone()) {
                                                        panic!("{}", e.to_string());
                                                    }
                                                    app.current_screen = CurrentScreen::Main;
                                                }
                                            }
                                            KeyCode::Backspace => {
                                                if app.currently_editing {
                                                    app.host_input.pop();
                                                }
                                            }
                                            KeyCode::Esc => {
                                                app.current_screen = CurrentScreen::Main;
                                                app.currently_editing = false;
                                            }
                                            KeyCode::Tab => {
                                                if app.selected_index < app.hosts.len() {
                                                    app.hosts.remove(app.selected_index);
                                                    if app.selected_index >= app.hosts.len() && app.selected_index > 0 {
                                                        app.selected_index -= 1;
                                                    }
                                                }
                                            }
                                            KeyCode::Char(value) => {
                                                if app.currently_editing {
                                                    app.host_input.push(value);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            CurrentScreen::Configuration => {
                                match app.editing_config_field {
                                    Some(EditingConfigField::ContributionGoal) => {
                                        if let KeyCode::Char(c) = key.code {
                                            app.contribution_goal_input.push(c);
                                        } else if key.code == KeyCode::Backspace {
                                            app.contribution_goal_input.pop();
                                        }
                                    }
                                    Some(EditingConfigField::GithubUsername) => {
                                        if let KeyCode::Char(c) = key.code {
                                            app.github_username_input.push(c);
                                        } else if key.code == KeyCode::Backspace {
                                            app.github_username_input.pop();
                                        }
                                    }
                                    None => {}  // Do nothing if no field is being edited
                                }
                                if key.code == KeyCode::Tab {
                                    app.toggle_editing_config();
                                }
                                match key.kind {
                                    KeyEventKind::Press => {
                                        match key.code {
                                            KeyCode::Esc => {
                                                app.current_screen = CurrentScreen::Main;
                                                app.editing_config_field = None;
                                            }
                                            KeyCode::Enter => {
                                                if let Ok(new_goal) = app.contribution_goal_input.parse::<u32>() {
                                                    app.contribution_goal = new_goal;
                                                }

                                                app.username = app.github_username_input.clone();

                                                // Save the configuration back to the file
                                                save_config(CONFIG_FILE_PATH, &Config {
                                                    github_username: app.username.clone(),
                                                    contribution_goal: app.contribution_goal,
                                                }).expect("Failed to save config.");

                                                // Reset the editing field
                                                app.editing_config_field = None;

                                                // Return to the main screen
                                                app.current_screen = CurrentScreen::Main;
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

fn check_contribution_progress(configuration: &Config) -> u32 {
    let client = reqwest::blocking::Client::new();

    let request_info = build_request_model(&configuration.github_username);

    let response = client
        .post(GH_API_PATH)
        .header(header::USER_AGENT, "AppName/0.1")
        .bearer_auth(&request_info.token)
        .json(&request_info.body)
        .send()
        .unwrap().text().unwrap();

     find_contribution_count_today(response).unwrap()
}

fn record_contribution_goal_met(date: NaiveDate, mut state: ContributionThresholdStatus, configuration: &Config) {
    state.threshold_met_date = Some(date.format(DATE_FORMATTER).to_string());
    state.threshold_met_goal = Some(configuration.contribution_goal);
    modify_hosts(UNBLOCK).expect("TODO: panic message");
    persist_contribution_state(&state).expect("TODO: panic message");
}

fn load_contribution_state(file_path: &str) -> Option<ContributionThresholdStatus> {
    let file = File::open(file_path).ok()?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).ok()
}

fn persist_contribution_state(state: &ContributionThresholdStatus) -> io::Result<()> {
    let file = OpenOptions::new().write(true).create(true).truncate(true).open(STATE_FILE_PATH)?;
    serde_json::to_writer_pretty(file, state).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

fn save_config(file_path: &str, config: &Config) -> Result<(), io::Error> {
    let toml_string = toml::to_string(config)
        .expect("Failed to serialize config to TOML");

    let mut file = File::create(file_path)?;
    file.write_all(toml_string.as_bytes())?;

    Ok(())
}

fn load_config(file_path: &str) -> Config {
    let read_result = fs::read_to_string(file_path);
    if read_result.is_ok() {
        toml::from_str(&read_result.unwrap())
            .expect("Failed to parse config file.")
    } else {
        Config {
            github_username: "".to_string(),
            contribution_goal: 1
        }
    }
}

fn build_request_model(username: &String) -> RequestModel {
    dotenv().ok();
    let token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN not set");

    let body = serde_json::json!({
        "query": GRAPHQL_QUERY,
        "variables": {
            "userName": username
        }
    });

    RequestModel {
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

fn initialise_hosts() -> Vec<String> {
    // TODO better handling
    let hosts = File::open(HOST_FILE_PATH).unwrap();
    let reader = BufReader::new(hosts);

    let mut inside_commit_block = false;
    let mut hosts: Vec<String> = Vec::new();

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
                let trimmed: String = get_trimmed_host_name(line, HOST_FILE_LOCAL_PREFIX_DISABLED_IP4);
                hosts.push(trimmed);
            } else {
                let trimmed: String = get_trimmed_host_name(line, HOST_FILE_LOCAL_PREFIX_IP4);
                hosts.push(trimmed);
            }
        }
    }

    hosts.dedup();
    hosts
}

fn get_trimmed_host_name(line: String, prefix_to_trim: &str) -> String {
    let trimmed: String = line.strip_prefix(prefix_to_trim).unwrap_or(&line).parse().unwrap();
    if trimmed.starts_with(HOST_FILE_LOCAL_PREFIX_IP6) {
        return trimmed.strip_prefix(HOST_FILE_LOCAL_PREFIX_IP6).unwrap_or(&trimmed).parse().unwrap();
    } else if trimmed.starts_with(HOST_FILE_LOCAL_PREFIX_DISABLED_IP6) {
        return trimmed.strip_prefix(HOST_FILE_LOCAL_PREFIX_DISABLED_IP6).unwrap_or(&trimmed).parse().unwrap();
    } else {
        trimmed
    }
}

/// Saves the current host configuration to the host files
fn save_to_host(hosts: Vec<String>) -> Result<(), io::Error> {
    let mut hosts_file = File::open(HOST_FILE_PATH)?;
    let mut hosts_content = String::new();
    hosts_file.read_to_string(&mut hosts_content)?;

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
    for domain in hosts {
        new_hosts.push_str(HOST_FILE_LOCAL_PREFIX_IP4);
        new_hosts.push_str(&*domain);
        new_hosts.push_str("\n");
        new_hosts.push_str(HOST_FILE_LOCAL_PREFIX_IP6);
        new_hosts.push_str(&*domain);
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
    fn load_config_file_not_found_return_default_config() {
        let config = load_config("doesNotExist.toml");

        assert_eq!(config.github_username, "".to_string());
        assert_eq!(config.contribution_goal, 1);
    }

    #[test]
    #[should_panic(expected = "Failed to parse config file.")]
    fn load_config_file_not_toml_panic() {
        load_config(".gitignore");
    }

    #[test]
    fn host_file_name() {
        assert_eq!(HOST_FILE_PATH, "/etc/hosts")
    }

    #[test]
    fn config_file_name() {
        assert_eq!(CONFIG_FILE_PATH, "config.toml")
    }

    #[test]
    fn gh_api_path() {
        assert_eq!(GH_API_PATH, "https://api.github.com/graphql")
    }

    #[test]
    fn commit_block_file_content() {
        assert_eq!(HOST_FILE_COMMIT_BLOCK_BEGIN, "### CommitBlock");
        assert_eq!(HOST_FILE_COMMIT_BLOCK_END, "### End CommitBlock");
        assert_eq!(HOST_FILE_LOCAL_PREFIX_IP4, "127.0.0.1\t");
        assert_eq!(HOST_FILE_BLOCK_PREFIX, "#");
        assert_eq!(HOST_FILE_LOCAL_PREFIX_DISABLED_IP4, "#127.0.0.1\t")
    }
}