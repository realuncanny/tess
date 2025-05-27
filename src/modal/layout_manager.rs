use crate::layout::{Layout, configuration};
use crate::screen::dashboard::{Dashboard, pane};
use crate::style::icon_text;
use crate::widget::column_drag::{self, DragEvent};
use crate::widget::dragger_row;
use crate::{style, tooltip};
use data::layout::WindowSpec;

use iced::widget::{
    Space, button, center, column, container, pane_grid::Configuration, row, scrollable, text,
    text_input, tooltip::Position as TooltipPosition,
};
use iced::{Element, Theme, padding};
use std::{collections::HashMap, vec};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum Editing {
    ConfirmingDelete(Uuid),
    Renaming(Uuid, String),
    Preview,
    None,
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectActive(Layout),
    SetLayoutName(Uuid, String),
    Renaming(String),
    AddLayout,
    RemoveLayout(Uuid),
    ToggleEditMode(Editing),
    CloneLayout(Uuid),
    Reorder(DragEvent),
}

pub enum Action {
    Select(Layout),
}

pub struct LayoutManager {
    layouts: HashMap<Uuid, (Layout, Dashboard)>,
    active_layout: Layout,
    pub layout_order: Vec<Uuid>,
    pub(crate) edit_mode: Editing,
}

impl LayoutManager {
    pub fn new() -> Self {
        let mut layouts = HashMap::new();

        let layout1 = Layout {
            id: Uuid::new_v4(),
            name: "Layout 1".to_string(),
        };

        layouts.insert(layout1.id, (layout1.clone(), Dashboard::default()));

        LayoutManager {
            layouts,
            active_layout: layout1.clone(),
            layout_order: vec![layout1.id],
            edit_mode: Editing::None,
        }
    }

    pub fn from_config(
        layout_order: Vec<Uuid>,
        layouts: HashMap<Uuid, (Layout, Dashboard)>,
        active_layout: Layout,
    ) -> Self {
        LayoutManager {
            layouts,
            active_layout,
            layout_order,
            edit_mode: Editing::None,
        }
    }

    fn generate_unique_layout_name(&self) -> String {
        let mut counter = 1;
        loop {
            let candidate = format!("Layout {counter}");
            if !self
                .layouts
                .values()
                .any(|(layout, _)| layout.name == candidate)
            {
                return candidate;
            }
            counter += 1;
        }
    }

    fn ensure_unique_name(&self, proposed_name: &str, current_id: Uuid) -> String {
        let mut counter = 2;
        let mut final_name = proposed_name.to_string();

        while self
            .layouts
            .values()
            .any(|(layout, _)| layout.id != current_id && layout.name == final_name)
        {
            final_name = format!("{proposed_name} ({counter})");
            counter += 1;
        }

        final_name.chars().take(20).collect()
    }

    pub fn iter_dashboards_mut(&mut self) -> impl Iterator<Item = &mut Dashboard> {
        self.layouts.values_mut().map(|(_, d)| d)
    }

    pub fn mut_dashboard(&mut self, id: &Uuid) -> Option<&mut Dashboard> {
        self.layouts.get_mut(id).map(|(_, d)| d)
    }

    pub fn dashboard(&self, id: &Uuid) -> Option<&Dashboard> {
        self.layouts.get(id).map(|(_, d)| d)
    }

    pub fn active_dashboard(&self) -> Option<&Dashboard> {
        self.dashboard(&self.active_layout.id)
    }

    pub fn active_dashboard_mut(&mut self) -> Option<&mut Dashboard> {
        let id = self.active_layout.id;
        self.mut_dashboard(&id)
    }

    pub fn active_layout(&self) -> Layout {
        self.active_layout.clone()
    }

    pub fn set_active_layout(&mut self, layout: Layout) -> Result<&mut Dashboard, String> {
        if let Some((_, dashboard)) = self.layouts.get_mut(&layout.id) {
            self.active_layout = layout;
            Ok(dashboard)
        } else {
            Err(format!("Layout with id {} does not exist", layout.id))
        }
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::SelectActive(layout) => {
                return Some(Action::Select(layout));
            }
            Message::ToggleEditMode(new_mode) => match (&new_mode, &self.edit_mode) {
                (Editing::Preview, Editing::Preview) => {
                    self.edit_mode = Editing::None;
                }
                (Editing::Renaming(id, _), Editing::Renaming(renaming_id, _))
                    if id == renaming_id =>
                {
                    self.edit_mode = Editing::None;
                }
                _ => {
                    self.edit_mode = new_mode;
                }
            },
            Message::AddLayout => {
                let new_layout = Layout {
                    id: Uuid::new_v4(),
                    name: self.generate_unique_layout_name(),
                };

                self.layout_order.push(new_layout.id);
                self.layouts
                    .insert(new_layout.id, (new_layout.clone(), Dashboard::default()));

                return Some(Action::Select(new_layout));
            }
            Message::RemoveLayout(id) => {
                self.layouts.remove(&id);
                self.layout_order.retain(|layout_id| *layout_id != id);

                self.edit_mode = Editing::Preview;
            }
            Message::SetLayoutName(id, new_name) => {
                let unique_name = self.ensure_unique_name(&new_name, id);
                let updated_layout = Layout {
                    id,
                    name: unique_name,
                };

                if let Some((_, dashboard)) = self.layouts.remove(&id) {
                    self.layouts
                        .insert(updated_layout.id, (updated_layout.clone(), dashboard));

                    if self.active_layout.id == id {
                        self.active_layout = updated_layout;
                    }
                }

                self.edit_mode = Editing::Preview;
            }
            Message::Renaming(name) => {
                self.edit_mode = match self.edit_mode {
                    Editing::Renaming(id, _) => {
                        let truncated = name.chars().take(20).collect();
                        Editing::Renaming(id, truncated)
                    }
                    _ => Editing::None,
                };
            }
            Message::CloneLayout(id) => {
                if let Some((layout, dashboard)) = self.layouts.get(&id) {
                    let new_id = Uuid::new_v4();
                    let new_layout = Layout {
                        id: new_id,
                        name: self.ensure_unique_name(&layout.name, new_id),
                    };

                    let ser_dashboard = data::Dashboard::from(dashboard);

                    let mut popout_windows: Vec<(Configuration<pane::State>, WindowSpec)> =
                        Vec::new();

                    for (pane, window_spec) in &ser_dashboard.popout {
                        let configuration = configuration(pane.clone());
                        popout_windows.push((configuration, *window_spec));
                    }

                    let dashboard = Dashboard::from_config(
                        configuration(ser_dashboard.pane.clone()),
                        popout_windows,
                        ser_dashboard.trade_fetch_enabled,
                    );

                    self.layout_order.push(new_layout.id);
                    self.layouts
                        .insert(new_layout.id, (new_layout.clone(), dashboard));
                }
            }
            Message::Reorder(event) => column_drag::reorder_vec(&mut self.layout_order, &event),
        }

        None
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut content = column![].spacing(8);

        let edit_btn = {
            match &self.edit_mode {
                Editing::None => {
                    button(text("Edit")).on_press(Message::ToggleEditMode(Editing::Preview))
                }
                _ => button(icon_text(style::Icon::Return, 12))
                    .on_press(Message::ToggleEditMode(Editing::Preview)),
            }
        };

        content = content.push(row![
            Space::with_width(iced::Length::Fill),
            row![
                tooltip(
                    button("i")
                        .style(move |theme, status| style::button::modifier(theme, status, true)),
                    Some("Layouts won't be saved if app exits abruptly"),
                    TooltipPosition::Top,
                ),
                edit_btn,
            ]
            .spacing(4),
        ]);

        let layout_btn =
            |layout: &Layout, on_press: Option<Message>| create_layout_button(layout, on_press);

        let mut layouts_column = column_drag::Column::new()
            .on_drag(Message::Reorder)
            .spacing(4);

        for id in &self.layout_order {
            if let Some((layout, _)) = self.layouts.get(id) {
                let mut layout_row = row![].height(iced::Length::Fixed(32.0));

                if self.active_layout.id == layout.id {
                    let color_column = container(column![])
                        .height(iced::Length::Fill)
                        .width(iced::Length::Fixed(2.0))
                        .style(style::layout_card_bar);

                    layout_row = layout_row
                        .push(container(color_column).padding(padding::left(8).bottom(4).top(4)));
                }

                match &self.edit_mode {
                    Editing::ConfirmingDelete(delete_id) => {
                        if *delete_id == layout.id {
                            let (confirm_btn, cancel_btn) = create_confirm_delete_buttons(layout);

                            layout_row = layout_row
                                .push(center(text(format!("Delete {}?", layout.name)).size(12)))
                                .push(confirm_btn)
                                .push(cancel_btn);
                        } else {
                            layout_row = layout_row.push(layout_btn(layout, None));
                        }
                    }
                    Editing::Renaming(id, name) => {
                        if *id == layout.id {
                            let input_box = text_input("New layout name", name)
                                .on_input(|new_name| Message::Renaming(new_name.clone()))
                                .on_submit(Message::SetLayoutName(*id, name.clone()));

                            let (_, cancel_btn) = create_confirm_delete_buttons(layout);

                            layout_row = layout_row
                                .push(center(input_box).padding(padding::left(4)))
                                .push(cancel_btn);
                        } else {
                            layout_row = layout_row.push(layout_btn(layout, None));
                        }
                    }
                    Editing::Preview => {
                        layout_row = layout_row
                            .push(layout_btn(layout, None))
                            .push(tooltip(
                                button(icon_text(style::Icon::Clone, 12))
                                    .on_press(Message::CloneLayout(layout.id))
                                    .style(move |t, s| style::button::transparent(t, s, true)),
                                Some("Clone layout"),
                                TooltipPosition::Top,
                            ))
                            .push(create_rename_button(layout));

                        if self.active_layout.id != layout.id {
                            layout_row = layout_row.push(self.create_delete_button(layout.id));
                        }
                    }
                    Editing::None => {
                        layout_row = layout_row.push(layout_btn(
                            layout,
                            if self.active_layout.id == layout.id {
                                None
                            } else {
                                Some(Message::SelectActive(layout.clone()))
                            },
                        ));
                    }
                }

                layouts_column = layouts_column.push(dragger_row(
                    container(layout_row.align_y(iced::Alignment::Center))
                        .style(style::dragger_row_container)
                        .into(),
                ));
            }
        }

        content = content.push(layouts_column);

        if self.edit_mode != Editing::None {
            content = content.push(
                container(
                    button(text("Add layout"))
                        .style(move |t, s| style::button::transparent(t, s, false))
                        .width(iced::Length::Fill)
                        .on_press(Message::AddLayout),
                )
                .style(style::chart_modal),
            );
        };

        scrollable::Scrollable::with_direction(
            content.padding(padding::left(8).right(8)),
            scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(8).scroller_width(6),
            ),
        )
        .into()
    }

    pub fn get_layout(&self, layout_id: Uuid) -> Option<(&Layout, &Dashboard)> {
        self.layouts
            .get(&layout_id)
            .map(|(layout, dashboard)| (layout, dashboard))
    }

    fn create_delete_button<'a>(&self, layout_id: Uuid) -> Element<'a, Message> {
        if self.active_layout.id == layout_id {
            tooltip(
                create_icon_button(
                    style::Icon::TrashBin,
                    12,
                    |theme, status| style::button::layout_name(theme, *status),
                    None,
                ),
                Some("Can't delete active layout"),
                TooltipPosition::Right,
            )
        } else {
            create_icon_button(
                style::Icon::TrashBin,
                12,
                |theme, status| style::button::layout_name(theme, *status),
                Some(Message::ToggleEditMode(Editing::ConfirmingDelete(
                    layout_id,
                ))),
            )
            .into()
        }
    }
}

fn create_rename_button<'a>(layout: &Layout) -> button::Button<'a, Message> {
    create_icon_button(
        style::Icon::Edit,
        12,
        |theme, status| style::button::layout_name(theme, *status),
        Some(Message::ToggleEditMode(Editing::Renaming(
            layout.id,
            layout.name.clone(),
        ))),
    )
}

fn create_confirm_delete_buttons<'a>(
    layout: &Layout,
) -> (button::Button<'a, Message>, button::Button<'a, Message>) {
    let confirm = create_icon_button(
        style::Icon::Checkmark,
        12,
        |theme, status| style::button::confirm(theme, *status, true),
        Some(Message::RemoveLayout(layout.id)),
    );

    let cancel = create_icon_button(
        style::Icon::Close,
        12,
        |theme, status| style::button::cancel(theme, *status, true),
        Some(Message::ToggleEditMode(Editing::Preview)),
    );

    (confirm, cancel)
}

fn create_layout_button<'a>(layout: &Layout, on_press: Option<Message>) -> Element<'a, Message> {
    let mut layout_btn = button(text(layout.name.clone()))
        .width(iced::Length::Fill)
        .style(style::button::layout_name);

    if let Some(msg) = on_press {
        layout_btn = layout_btn.on_press(msg);
    }

    layout_btn.into()
}

fn create_icon_button<'a>(
    icon: style::Icon,
    size: u16,
    style_fn: impl Fn(&Theme, &button::Status) -> button::Style + 'static,
    on_press: Option<Message>,
) -> button::Button<'a, Message> {
    let mut btn =
        button(icon_text(icon, size)).style(move |theme, status| style_fn(theme, &status));

    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }

    btn
}
