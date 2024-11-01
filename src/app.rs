#[derive(PartialEq, Debug)]
pub enum CurrentScreen {
    Main,
    Editing,
    Exiting,
    Configuration,
    Help,
}

#[derive(PartialEq, Debug)]
pub enum CurrentlyEditing {
    Key,
    Value, // TODO remove
}

#[derive(PartialEq, Debug)]
pub enum EditingField {
    ContributionGoal,
    GithubUsername,
}

pub struct App {
    pub host_input: String,
    pub selected_index: usize,
    pub hosts: Vec<String>,
    pub current_screen: CurrentScreen,
    pub currently_editing: Option<CurrentlyEditing>,
    pub contribution_goal_input: String,
    pub github_username_input: String,
    pub editing_field: Option<EditingField>,
    pub progress: u32,
    pub contribution_goal: u32,
    pub threshold_met_date: Option<String>,
    pub threshold_met_goal: Option<u32>,
    pub username: String,
}

impl App {
    pub fn new(hosts: Vec<String>, current_contributions: u32, contribution_goal: u32, username: String, threshold_met_date: Option<String>, threshold_met_goal: Option<u32>) -> App {
        App {
            host_input: String::new(),
            selected_index: 0,
            hosts,
            current_screen: CurrentScreen::Main,
            currently_editing: None,
            contribution_goal_input: contribution_goal.to_string(),
            github_username_input: username.clone(),
            editing_field: None,
            progress: current_contributions,
            contribution_goal,
            threshold_met_goal,
            threshold_met_date,
            username,
        }
    }

    pub fn save_new_host(&mut self) {
        // This prevents a blank entry appearing if you press Enter without typing any hosts
        if !self.host_input.is_empty() {
            self.hosts.push(self.host_input.clone());
        }
        self.host_input = String::new();
        self.currently_editing = None;
    }

    pub fn toggle_editing_config(&mut self) {
        if let Some(edit_mode) = &self.editing_field {
            match edit_mode {
                EditingField::GithubUsername => self.editing_field = Some(EditingField::ContributionGoal),
                EditingField::ContributionGoal => self.editing_field = Some(EditingField::GithubUsername),
            };
        } else {
            self.editing_field = Some(EditingField::ContributionGoal);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use crate::app::CurrentScreen::Main;
    use crate::app::EditingField::{ContributionGoal, GithubUsername};
    use super::*;

    #[test]
    fn can_instantiate_app() {
        let hosts: Vec<String> = vec!(String::from_str("Commit").unwrap(), String::from_str("Block").unwrap());
        let current_contributions = 4;
        let contribution_goal = 5;
        let username = String::from_str("BingBong").unwrap();
        let threshold_met_date = Some(String::from_str("01/11/2024").unwrap());
        let threshold_met_goal = Some(4);

        let app = App::new(hosts.clone(), current_contributions, contribution_goal, username.clone(), threshold_met_date.clone(), threshold_met_goal.clone());

        assert_eq!(app.host_input, String::new());
        assert_eq!(app.hosts, hosts);
        assert_eq!(app.progress, current_contributions);
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.current_screen, Main);
        assert_eq!(app.currently_editing, None);
        assert_eq!(app.contribution_goal_input, contribution_goal.to_string());
        assert_eq!(app.github_username_input, username);
        assert_eq!(app.editing_field, None);
        assert_eq!(app.contribution_goal, contribution_goal);
        assert_eq!(app.threshold_met_goal, threshold_met_goal);
        assert_eq!(app.threshold_met_date, threshold_met_date);
        assert_eq!(app.username, username);
    }

    #[test]
    fn can_save_new_host_input_empty() {
        let hosts: Vec<String> = vec!(String::from("Commit"), String::from("Block"));
        let current_contributions = 4;
        let contribution_goal = 5;
        let username = String::from("BingBong");
        let threshold_met_date = Some(String::from("01/11/2024"));
        let threshold_met_goal = Some(4);

        let mut app = App::new(hosts.clone(), current_contributions, contribution_goal, username.clone(), threshold_met_date.clone(), threshold_met_goal.clone());

        app.save_new_host();

        assert_eq!(app.host_input, String::new());
        assert_eq!(app.currently_editing, None);
        assert_eq!(app.hosts, hosts); // No new hosts saved since host_input is empty
    }

    #[test]
    fn can_save_new_host() {
        let hosts: Vec<String> = vec!(String::from("Commit"), String::from("Block"));
        let current_contributions = 4;
        let contribution_goal = 5;
        let username = String::from("BingBong");
        let threshold_met_date = Some(String::from("01/11/2024"));
        let threshold_met_goal = Some(4);

        let mut app = App::new(hosts.clone(), current_contributions, contribution_goal, username.clone(), threshold_met_date.clone(), threshold_met_goal.clone());
        app.host_input = String::from("New Host");

        app.save_new_host();

        assert_eq!(app.host_input, String::new());
        assert_eq!(app.currently_editing, None);
        assert_eq!(app.hosts, vec!(String::from("Commit"), String::from("Block"), String::from("New Host")));
    }

    #[test]
    fn can_toggle_editing_config() {
        let hosts: Vec<String> = vec!(String::from("Commit"), String::from("Block"));
        let current_contributions = 4;
        let contribution_goal = 5;
        let username = String::from("BingBong");
        let threshold_met_date = Some(String::from("01/11/2024"));
        let threshold_met_goal = Some(4);

        let mut app = App::new(hosts.clone(), current_contributions, contribution_goal, username.clone(), threshold_met_date.clone(), threshold_met_goal.clone());

        app.toggle_editing_config();

        assert_eq!(app.editing_field, Some(ContributionGoal));

        app.toggle_editing_config();

        assert_eq!(app.editing_field, Some(GithubUsername));
    }
}