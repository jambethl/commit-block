use std::{error::Error, io, thread};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::time::Duration;
use chrono::Utc;
use std::env;
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
use serde_json::Value;

mod app;
mod ui;

use crate::{
    app::{App, CurrentScreen, CurrentlyEditing},
    ui::ui,
};

const HOST_FILE_LOCAL_PREFIX: &str = "127.0.0.1\t";
const HOST_FILE_LOCAL_PREFIX_DISABLED: &str = "#127.0.0.1\t";
const HOST_FILE_COMMIT_BLOCK_BEGIN: &str = "### CommitBlock";
const HOST_FILE_COMMIT_BLOCK_END: &str = "### End CommitBlock";
const HOST_FILE_PATH: &str = "tmp/test";
const QUIT_KEY: char = 'q';
const INSERT_KEY: char = 'i';
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let existing_pairs = initialise_host_pairs();
    let mut app = App::new(existing_pairs);

    thread::spawn(move || {
        check_commit_count();
    });
    run_app(&mut terminal, &mut app).expect("TODO: panic message");

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    terminal.show_cursor()?;

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    loop {
        terminal.draw(|f| ui(f, app))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            // TODO handle configuration screen
            match app.current_screen {
                CurrentScreen::Main => match key.code {
                    KeyCode::Char(INSERT_KEY) => {
                        app.current_screen = CurrentScreen::Editing;
                        app.currently_editing = Some(CurrentlyEditing::Key);
                    }
                    KeyCode::Char(QUIT_KEY) => {
                        app.current_screen = CurrentScreen::Exiting;
                    }
                    _ => {}
                },
                CurrentScreen::Exiting => match key.code {
                    KeyCode::Char('y') | KeyCode::Char(QUIT_KEY)=> {
                        return Ok(true);
                    }
                    KeyCode::Char('n') => {
                        app.current_screen = CurrentScreen::Main
                    }
                    _ => {}
                },
                CurrentScreen::Editing if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Enter => {
                            if let Some(editing) = &app.currently_editing {
                                match editing {
                                    CurrentlyEditing::Key => {
                                        app.currently_editing = Some(CurrentlyEditing::Value);
                                    }
                                    CurrentlyEditing::Value => {
                                        app.save_key_value();
                                        match save_to_host(app.pairs.clone()) {
                                            Ok(_) => {},
                                            Err(e) => panic!("{}", e.to_string()),
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
                                            _ => {},
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
    }
}

fn check_commit_count() {

    // TODO store goal_met var; lets us return early if true and day == today
    // avoids us hitting the API unnecessarily until day advances
    let client = reqwest::blocking::Client::new();
    let request_info = build_request_model();
    let mut goal_met = false;
    loop {

        if goal_met {
            thread::sleep(Duration::from_secs(30));
            continue;
        }

        let response = client
            .post(&request_info.path)
            .header(header::USER_AGENT, "AppName/0.1")
            .bearer_auth(&request_info.token)
            .json(&request_info.body)
            .send()
            .unwrap().text().unwrap();

        let contribution_count = find_contribution_count_today(response).unwrap();

        println!("{}", contribution_count);

        let contribution_goal = fetch_configured_contribution_goal();

        // TODO reset contribution count
        if contribution_count >= contribution_goal {
            unblock_hosts().expect("TODO: panic message");
            goal_met = true;
        }
        // TODO draw progress bar

        thread::sleep(Duration::from_secs(5));
    }
}

// todo -- fetch from configuration file or else default
fn fetch_configured_contribution_goal() -> i32 {
    0
}

fn build_request_model() -> RequestModel {
    dotenv().ok();
    let token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN not set");

    let body = serde_json::json!({
        "query": GRAPHQL_QUERY,
        "variables": {
            "userName": "jambethl"
        }
    });
    let path = String::from("https://api.github.com/graphql");

    RequestModel {
        path,
        token,
        body
    }
}

fn find_contribution_count_today(api_response: String) -> Result<i32, ()> {
    let json_response: Value = serde_json::from_str(&api_response).unwrap();

    let today = Utc::now().format("%Y-%m-%d").to_string();

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
                        return Ok(contribution_count as i32);
                    }
                }
            }
        }
    }
    println!("No contributions found for today.");
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
            if line.starts_with("#") {
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
            false => "#",
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

fn unblock_hosts() -> Result<(), io::Error> {
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

        if in_commitblock && !line.trim().starts_with("#") {
            output.push(format!("#{}", line));
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