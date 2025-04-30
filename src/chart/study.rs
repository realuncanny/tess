use data::chart::kline::FootprintStudy;
use iced::{
    Element, padding,
    widget::{button, column, container, horizontal_space, row, slider, text},
};

use crate::style::{self, Icon, icon_text};

#[derive(Debug, Clone)]
pub enum Message {
    CardToggled(FootprintStudy),
    StudyToggled(FootprintStudy, bool),
    StudyValueChanged(FootprintStudy),
}

pub enum Action {
    None,
    ToggleStudy(FootprintStudy, bool),
    ConfigureStudy(FootprintStudy),
}

#[derive(Default)]
pub struct ChartStudy {
    expanded_card: Option<FootprintStudy>,
}

impl ChartStudy {
    pub fn new() -> Self {
        Self {
            expanded_card: None,
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::CardToggled(study) => {
                let should_collapse = self
                    .expanded_card
                    .as_ref()
                    .is_some_and(|expanded| expanded.is_same_type(&study));

                if should_collapse {
                    self.expanded_card = None;
                } else {
                    self.expanded_card = Some(study);
                }
            }
            Message::StudyToggled(study, is_checked) => {
                return Action::ToggleStudy(study, is_checked);
            }
            Message::StudyValueChanged(study) => {
                return Action::ConfigureStudy(study);
            }
        }

        Action::None
    }

    pub fn view(&self, studies: &Vec<FootprintStudy>) -> Element<Message> {
        let mut content = column![].spacing(4);

        for available_study in FootprintStudy::ALL {
            let (is_selected, value) = {
                let mut is_selected = false;
                let mut value = None;

                for s in studies {
                    if s.is_same_type(&available_study) {
                        is_selected = true;
                        value = match s {
                            FootprintStudy::Imbalance { threshold } => Some(threshold),
                            FootprintStudy::NPoC { lookback } => Some(lookback),
                        };
                    }
                }
                (is_selected, value)
            };

            content = content.push(self.create_study_row(available_study, is_selected, value));
        }

        content.into()
    }

    fn create_study_row(
        &self,
        study: FootprintStudy,
        is_selected: bool,
        value: Option<&usize>,
    ) -> Element<Message> {
        let checkbox = iced::widget::checkbox(study.to_string(), is_selected)
            .on_toggle(move |checked| Message::StudyToggled(study, checked));

        let mut checkbox_row = row![checkbox, horizontal_space()]
            .height(36)
            .align_y(iced::Alignment::Center)
            .padding(4)
            .spacing(4);

        let is_expanded = self
            .expanded_card
            .as_ref()
            .is_some_and(|expanded| expanded.is_same_type(&study));

        if is_selected {
            checkbox_row = checkbox_row.push(
                button(icon_text(Icon::Cog, 12))
                    .on_press(Message::CardToggled(study))
                    .style(move |theme, status| {
                        style::button::transparent(theme, status, is_expanded)
                    }),
            )
        }

        let mut column = column![checkbox_row].padding(padding::left(4));

        if is_expanded && value.is_some() {
            let value = *value.unwrap();

            match study {
                FootprintStudy::NPoC { .. } => {
                    let slider_ui = slider(10.0..=400.0, value as f32, move |new_value| {
                        let updated_study = FootprintStudy::NPoC {
                            lookback: new_value as usize,
                        };
                        Message::StudyValueChanged(updated_study)
                    })
                    .step(10.0);

                    column = column.push(
                        column![text(format!("Lookback: {value} datapoints")), slider_ui]
                            .padding(8)
                            .spacing(4),
                    );
                }
                FootprintStudy::Imbalance { .. } => {
                    let slider_ui = slider(100.0..=800.0, value as f32, move |new_value| {
                        let updated_study = FootprintStudy::Imbalance {
                            threshold: new_value as usize,
                        };
                        Message::StudyValueChanged(updated_study)
                    })
                    .step(25.0);

                    column = column.push(
                        column![text(format!("Ask:Bid threshold: {value}%")), slider_ui]
                            .padding(8)
                            .spacing(4),
                    );
                }
            }
        }

        container(column).style(style::modal_container).into()
    }
}
