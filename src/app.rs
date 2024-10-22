use std::collections::HashMap;

pub enum CurrentScreen {
    Main,
    Editing,
    Exiting,
    Configuration,
    Help,
}

pub enum CurrentlyEditing {
    Key,
    Value,
}

pub enum EditingField {
    ContributionGoal,
    GithubUsername,
}

pub struct App {
    pub key_input: String,
    pub value_input: Option<bool>,
    pub pairs: HashMap<String, bool>,
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
    pub fn new(pairs: HashMap<String, bool>, current_contributions: u32, contribution_goal: u32, username: String, threshold_met_date: Option<String>, threshold_met_goal: Option<u32>) -> App {
        App {
            key_input: String::new(),
            value_input: None,
            pairs,
            current_screen: CurrentScreen::Main,
            currently_editing: None,
            contribution_goal_input: "".to_string(),
            github_username_input: "".to_string(),
            editing_field: None,
            progress: current_contributions,
            contribution_goal,
            threshold_met_goal,
            threshold_met_date,
            username,
        }
    }

    pub fn save_key_value(&mut self) {
        self.pairs
            .insert(self.key_input.clone(), match self.value_input {
                None => false,
                Some(true) => self.value_input.unwrap().clone(),
                Some(false) => self.value_input.unwrap().clone(),
            });

        self.key_input = String::new();
        self.value_input = None;
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

    pub fn toggle_editing(&mut self) {
        if let Some(edit_mode) = &self.currently_editing {
            match edit_mode {
                CurrentlyEditing::Key => self.currently_editing = Some(CurrentlyEditing::Value),
                CurrentlyEditing::Value => self.currently_editing = Some(CurrentlyEditing::Key),
            };
        } else {
            self.currently_editing = Some(CurrentlyEditing::Key);
        }
    }
}