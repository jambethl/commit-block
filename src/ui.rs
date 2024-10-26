use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use ratatui::style::{Modifier, Stylize};
use ratatui::style::Color::Green;
use ratatui::widgets::Gauge;
use crate::app::{App, CurrentScreen, CurrentlyEditing, EditingField};
use crate::app::EditingField::{ContributionGoal, GithubUsername};

pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let title = Paragraph::new(Text::styled(
        "Commit Blocker",
        Style::default().fg(Color::Green),
    ))
        .block(title_block);

    frame.render_widget(title, chunks[0]);

    let mut list_items = Vec::<ListItem>::new();

    for (index, host) in app.hosts.iter().enumerate() {
        let style = if app.currently_editing.is_some() && index == app.selected_index {
            Style::default().fg(Green).bg(Color::LightBlue)
        } else {
            Style::default().fg(Color::Yellow)
        };
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!("{: <25}", host),
            style,
        ))));
    }

    // Include the new host input line in edit mode
    if app.currently_editing.is_some() {
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!("{: <25}", app.host_input),
            Style::default().fg(Color::Cyan),
        ))));
    }

    let left_block = List::new(
        list_items
    ).block(Block::default()
        .borders(Borders::ALL)
        .title("Blocked hosts"));

    frame.render_widget(left_block, middle_chunks[0]);

    let lines: Vec<Line> = vec![
        Line::from_iter([
            Span::styled("Configured contribution target", Style::default().fg(Color::Yellow)),
            Span::raw(" : "),
            Span::styled(app.contribution_goal.to_string(), Style::default().fg(Color::Green)),
        ]),
        Line::from_iter([
            Span::styled("Current contribution count for today", Style::default().fg(Color::Yellow)),
            Span::raw(" : "),
            Span::styled(app.progress.to_string(), Style::default().fg(Color::Green)),
        ]),
        Line::from_iter([
            Span::styled("Username", Style::default().fg(Color::Yellow)),
            Span::raw(" : "),
            Span::styled(app.username.to_string(), Style::default().fg(Color::Green)),
        ]),
        Line::from_iter([
            Span::styled("Previous date contribution goal met", Style::default().fg(Color::Yellow)),
            Span::raw(" : "),
            Span::styled(app.threshold_met_date.clone().unwrap_or("None".parse().unwrap()), Style::default().fg(Color::Green)),
        ]),
        Line::from_iter([
            Span::styled("Previous contribution goal met", Style::default().fg(Color::Yellow)),
            Span::raw(" : "),
            Span::styled(app.threshold_met_goal.clone().unwrap_or(0).to_string(), Style::default().fg(Color::Green)),
        ]),
    ].into_iter().collect();

    let right_block = Paragraph::new(lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Configuration")
        );
    frame.render_widget(right_block, middle_chunks[1]);

    let current_navigation_text = vec![
        // The first half of the text
        match app.current_screen {
            CurrentScreen::Main => Span::styled("Normal Mode", Style::default().fg(Color::Green)),
            CurrentScreen::Editing => Span::styled("Editing Mode", Style::default().fg(Color::Yellow)),
            CurrentScreen::Configuration => Span::styled("Editing Mode", Style::default().fg(Color::Yellow)),
            CurrentScreen::Exiting => Span::styled("Exiting", Style::default().fg(Color::LightRed)),
            CurrentScreen::Help => Span::styled("Help", Style::default().fg(Color::Green)),
        }
            .to_owned(),
        // A white divider bar to separate the two sections
        Span::styled(" | ", Style::default().fg(Color::White)),
        // The final section of the text, with hints on what the user is editing
        {
            if let Some(editing) = &app.currently_editing {
                match editing {
                    CurrentlyEditing::Key => {
                        Span::styled("Editing Host List", Style::default().fg(Color::Green))
                    }
                    CurrentlyEditing::Value => {
                        Span::styled("Editing Host Configuration", Style::default().fg(Color::LightGreen))
                    }
                }
            } else if let Some(editing) = &app.editing_field {
                match editing {
                    EditingField::ContributionGoal => {
                        Span::styled("Editing Contribution Goal", Style::default().fg(Color::Green))
                    }
                    EditingField::GithubUsername => {
                        Span::styled("Editing Username", Style::default().fg(Color::LightGreen))
                    }
                }
            } else {
                Span::styled("Not Editing Anything", Style::default().fg(Color::DarkGray))
            }
        },
    ];

    let mode_footer = Paragraph::new(Line::from(current_navigation_text))
        .block(Block::default().borders(Borders::ALL));

    let current_keys_hint = {
        match app.current_screen {
            CurrentScreen::Main => Span::styled(
                "(i) modify hosts / (c) edit configuration / (q) quit / (h) help",
                Style::default().fg(Color::Red),
            ),
            CurrentScreen::Editing => Span::styled(
                "(ESC) to cancel/(Tab) to switch boxes/enter to complete",
                Style::default().fg(Color::Red),
            ),
            CurrentScreen::Exiting => Span::styled(
                "(q) to quit / (i) to insert new host / (h) for help",
                Style::default().fg(Color::Red),
            ),
            CurrentScreen::Configuration => Span::styled(
                "(ESC) to cancel / (Tab) to switch boxes / enter to complete",
                Style::default().fg(Color::Red),
            ),
            CurrentScreen::Help => Span::styled(
                "Press any key to return",
                Style::default().fg(Color::Red),
            )
        }
    };

    let key_notes_footer =
        Paragraph::new(Line::from(current_keys_hint)).block(Block::default().borders(Borders::ALL));

    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)])
        .split(chunks[2]);

    let progress_bar_fg_color = if app.progress < app.contribution_goal / 2 {
        Color::Red
    } else if app.progress < app.contribution_goal {
        Color::Yellow
    } else {
        Color::Green
    };

    let progress_label = Span::styled(
        format!("{:.1}/{:.1}", app.progress, app.contribution_goal),
        Style::new().italic().bold().fg(progress_bar_fg_color),
    );

    let contribution_ratio = if app.progress > app.contribution_goal {
        1.0
    } else {
        app.progress as f64 / app.contribution_goal as f64
    };
    let progress_bar = Gauge::default()
        .block(Block::bordered().title("Progress"))
        .gauge_style(
            Style::default()
                .fg(progress_bar_fg_color)
                .bg(Color::Black)
                .add_modifier(Modifier::ITALIC),
        )
        .label(progress_label)
        .ratio(contribution_ratio);

    frame.render_widget(mode_footer, footer_chunks[0]);
    frame.render_widget(key_notes_footer, footer_chunks[1]);
    frame.render_widget(progress_bar, footer_chunks[2]);

    if let Some(_editing_config) = &app.editing_field {
        let size = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(size);

        let goal_input = Paragraph::new(app.contribution_goal_input.clone())
            .block(Block::default().borders(Borders::ALL).title("Contribution Goal"))
            .style(get_input_field_style(app, ContributionGoal));

        let username_input = Paragraph::new(app.github_username_input.clone())
            .block(Block::default().borders(Borders::ALL).title("GitHub Username"))
            .style(get_input_field_style(app, GithubUsername));

        frame.render_widget(Clear, chunks[1]);
        frame.render_widget(Clear, chunks[2]);
        frame.render_widget(goal_input, chunks[1]);
        frame.render_widget(username_input, chunks[2]);
    }

    if let CurrentScreen::Help = app.current_screen {
        frame.render_widget(Clear, frame.area()); //this clears the entire screen and anything already drawn
        let popup_block = Block::default()
            .title("Help")
            .borders(Borders::NONE)
            .style(Style::default().bg(Color::Yellow));

        let help_text = Text::styled(
            "Balh",
            Style::default().fg(Color::Black),
        );
        let help_paragraph = Paragraph::new(help_text)
            .block(popup_block)
            .wrap(Wrap { trim: false });
        let area = centered_rect(60, 25, frame.area());
        frame.render_widget(help_paragraph, area);
    }

    if let CurrentScreen::Exiting = app.current_screen {
        frame.render_widget(Clear, frame.area()); //this clears the entire screen and anything already drawn
        let popup_block = Block::default()
            .title("Y/N")
            .borders(Borders::NONE)
            .style(Style::default().bg(Color::DarkGray));

        let exit_text = Text::styled(
            "Exit?",
            Style::default().fg(Color::Red),
        );
        // the `trim: false` will stop the text from being cut off when over the edge of the block
        let exit_paragraph = Paragraph::new(exit_text)
            .block(popup_block)
            .wrap(Wrap { trim: false });

        let area = centered_rect(60, 25, frame.area());
        frame.render_widget(exit_paragraph, area);
    }
}

fn get_input_field_style(app: &App, field: EditingField) -> Style {
    if app.editing_field == Some(field) {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    }
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