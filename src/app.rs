use std::collections::HashMap;

pub enum CurrentScreen {
    Main,
    Editing,
    Exiting,
}

pub enum CurrentlyEditing {
    Key,
    Value
}

pub struct App {
    pub key_input: String,
    pub value_input: Option<bool>,
    pub pairs: HashMap<String, bool>,
    pub current_screen: CurrentScreen,
    pub currently_editing: Option<CurrentlyEditing>
}

impl App {

    pub fn new() -> App {
        App {
            key_input: String::new(),
            value_input: None,
            pairs: HashMap::new(),
            current_screen: CurrentScreen::Main,
            currently_editing: None,
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