use crate::style::{self, Icon, icon_text};
use data::chart::kline::FootprintStudy;
use iced::{
    Element, padding,
    widget::{button, column, container, horizontal_rule, horizontal_space, row, slider, text},
};

#[derive(Debug, Clone, Copy)]
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
            let (is_selected, study_config) = {
                let mut is_selected = false;
                let mut study_config = None;

                for s in studies {
                    if s.is_same_type(&available_study) {
                        is_selected = true;
                        study_config = Some(*s);
                        break;
                    }
                }
                (is_selected, study_config)
            };

            content =
                content.push(self.create_study_row(available_study, is_selected, study_config));
        }

        content.into()
    }

    fn create_study_row(
        &self,
        study: FootprintStudy,
        is_selected: bool,
        study_config: Option<FootprintStudy>,
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
            );
        }

        let mut column = column![checkbox_row].padding(padding::left(4));

        if is_expanded && study_config.is_some() {
            let config = study_config.unwrap();

            match config {
                FootprintStudy::NPoC { lookback } => {
                    let slider_ui = slider(10.0..=400.0, lookback as f32, move |new_value| {
                        let updated_study = FootprintStudy::NPoC {
                            lookback: new_value as usize,
                        };
                        Message::StudyValueChanged(updated_study)
                    })
                    .step(10.0);

                    column = column.push(
                        column![text(format!("Lookback: {lookback} datapoints")), slider_ui]
                            .padding(8)
                            .spacing(4),
                    );
                }
                FootprintStudy::Imbalance {
                    threshold,
                    color_scale,
                    ignore_zeros,
                } => {
                    let qty_threshold = {
                        let info_text = text(format!("Ask:Bid threshold: {threshold}%"));

                        let threshold_slider =
                            slider(100.0..=800.0, threshold as f32, move |new_value| {
                                let updated_study = FootprintStudy::Imbalance {
                                    threshold: new_value as usize,
                                    color_scale,
                                    ignore_zeros,
                                };
                                Message::StudyValueChanged(updated_study)
                            })
                            .step(25.0);

                        column![info_text, threshold_slider,].padding(8).spacing(4)
                    };

                    let color_scaling = {
                        let color_scale_enabled = color_scale.is_some();
                        let color_scale_value = color_scale.unwrap_or(100);

                        let color_scale_checkbox =
                            iced::widget::checkbox("Dynamic color scaling", color_scale_enabled)
                                .on_toggle(move |is_enabled| {
                                    let updated_study = FootprintStudy::Imbalance {
                                        threshold,
                                        color_scale: if is_enabled {
                                            Some(color_scale_value)
                                        } else {
                                            None
                                        },
                                        ignore_zeros,
                                    };
                                    Message::StudyValueChanged(updated_study)
                                });

                        if color_scale_enabled {
                            let scaling_slider = column![
                                text(format!("Opaque color at: {color_scale_value}x")),
                                slider(50.0..=2000.0, color_scale_value as f32, move |new_value| {
                                    let updated_study = FootprintStudy::Imbalance {
                                        threshold,
                                        color_scale: Some(new_value as usize),
                                        ignore_zeros,
                                    };
                                    Message::StudyValueChanged(updated_study)
                                })
                                .step(50.0)
                            ]
                            .spacing(2);

                            column![color_scale_checkbox, scaling_slider]
                                .padding(8)
                                .spacing(8)
                        } else {
                            column![color_scale_checkbox].padding(8)
                        }
                    };

                    let ignore_zeros_checkbox = {
                        let cbox = iced::widget::checkbox("Ignore zeros", ignore_zeros).on_toggle(
                            move |is_checked| {
                                let updated_study = FootprintStudy::Imbalance {
                                    threshold,
                                    color_scale,
                                    ignore_zeros: is_checked,
                                };
                                Message::StudyValueChanged(updated_study)
                            },
                        );

                        column![cbox].padding(8).spacing(4)
                    };

                    column = column.push(
                        column![
                            qty_threshold,
                            horizontal_rule(1),
                            color_scaling,
                            horizontal_rule(1),
                            ignore_zeros_checkbox,
                        ]
                        .padding(8)
                        .spacing(4),
                    );
                }
            }
        }

        container(column).style(style::modal_container).into()
    }
}
