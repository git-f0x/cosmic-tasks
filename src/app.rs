use std::{env, process};
use std::any::TypeId;
use std::collections::{HashMap, VecDeque};
use std::error::Error;

use cosmic::{
    app, Application, ApplicationExt, Command, cosmic_config, cosmic_theme, Element, executor,
    theme, widget,
};
use cosmic::app::{Core, message, Message as CosmicMessage};
use cosmic::iced::{
    Alignment, event, Event, keyboard::Event as KeyEvent, Length, Subscription, window,
};
use cosmic::iced::keyboard::{Key, Modifiers};
use cosmic::widget::segmented_button;
use cosmic::widget::segmented_button::{Entity, EntityMut, SingleSelect};
use done_core::models::list::List;
use done_core::models::task::Task;
use done_core::service::Service;

use crate::{content, details, fl, menu};
use crate::config::{AppTheme, CONFIG_VERSION};
use crate::content::Content;
use crate::details::Details;
use crate::key_bind::{key_binds, KeyBind};

pub struct App {
    core: Core,
    nav_model: segmented_button::SingleSelectModel,
    content: Content,
    details: Details,
    config_handler: Option<cosmic_config::Config>,
    config: crate::config::Config,
    app_themes: Vec<String>,
    context_page: ContextPage,
    key_binds: HashMap<KeyBind, Action>,
    selected_list: Option<List>,
    modifiers: Modifiers,
    dialog_pages: VecDeque<DialogPage>,
    dialog_text_input: widget::Id,
}

#[derive(Debug, Clone)]
pub enum Message {
    ContentMessage(content::Message),
    DetailsMessage(details::Message),
    ToggleContextPage(ContextPage),
    LaunchUrl(String),
    PopulateLists(Vec<List>),
    WindowClose,
    WindowNew,
    DialogCancel,
    DialogComplete,
    DialogUpdate(DialogPage),
    Key(Modifiers, Key),
    Modifiers(Modifiers),
    AppTheme(AppTheme),
    SystemThemeModeChange(cosmic_theme::ThemeMode),
    OpenNewListDialog,
    OpenRenameListDialog,
    OpenDeleteListDialog,
    AddList(List),
    DeleteList,
    UpdateList(List),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextPage {
    About,
    TaskDetails,
    Settings,
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            Self::About => String::new(),
            Self::Settings => fl!("settings"),
            Self::TaskDetails => "Details".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogPage {
    NewItem { name: String },
    RenameItem { from: String, to: String },
    DeleteItem { name: String },
}

#[derive(Clone, Debug)]
pub struct Flags {
    pub config_handler: Option<cosmic_config::Config>,
    pub config: crate::config::Config,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    About,
    ItemDown,
    ItemUp,
    Settings,
    WindowClose,
    WindowNew,
    NewList,
    DeleteList,
    RenameList,
}

impl Action {
    pub fn message(self, _entity_opt: Option<Entity>) -> Message {
        match self {
            Action::About => Message::ToggleContextPage(ContextPage::About),
            Action::ItemDown => Message::ContentMessage(content::Message::ItemDown),
            Action::ItemUp => Message::ContentMessage(content::Message::ItemUp),
            Action::Settings => Message::ToggleContextPage(ContextPage::Settings),
            Action::WindowClose => Message::WindowClose,
            Action::WindowNew => Message::WindowNew,
            Action::NewList => Message::OpenNewListDialog,
            Action::RenameList => Message::OpenRenameListDialog,
            Action::DeleteList => Message::OpenDeleteListDialog,
        }
    }
}

impl App {
    fn update_config(&mut self) -> Command<CosmicMessage<Message>> {
        app::command::set_theme(self.config.app_theme.theme())
    }

    fn about(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;
        let repository = "https://github.com/edfloreshz/cosmic-todo";
        let hash = env!("VERGEN_GIT_SHA");
        let short_hash: String = hash.chars().take(7).collect();
        let date = env!("VERGEN_GIT_COMMIT_DATE");
        widget::column::with_children(vec![
            widget::svg(widget::svg::Handle::from_memory(
                &include_bytes!(
                    "../res/icons/hicolor/128x128/apps/com.system76.CosmicTodo.svg"
                )[..],
            ))
                .into(),
            widget::text::title3(fl!("cosmic-todo")).into(),
            widget::button::link(repository)
                .on_press(Message::LaunchUrl(repository.to_string()))
                .padding(0)
                .into(),
            widget::button::link(fl!(
                    "git-description",
                    hash = short_hash.as_str(),
                    date = date
                ))
                .on_press(Message::LaunchUrl(format!("{}/commits/{}", repository, hash)))
                .padding(0)
                .into(),
        ])
            .align_items(Alignment::Center)
            .spacing(space_xxs)
            .width(Length::Fill)
            .into()
    }

    fn settings(&self) -> Element<Message> {
        let app_theme_selected = match self.config.app_theme {
            AppTheme::Dark => 1,
            AppTheme::Light => 2,
            AppTheme::System => 0,
        };
        widget::settings::view_column(vec![widget::settings::view_section(fl!("appearance"))
            .add(
                widget::settings::item::builder(fl!("theme")).control(widget::dropdown(
                    &self.app_themes,
                    Some(app_theme_selected),
                    move |index| {
                        Message::AppTheme(match index {
                            1 => AppTheme::Dark,
                            2 => AppTheme::Light,
                            _ => AppTheme::System,
                        })
                    },
                )),
            )
            .into()])
            .into()
    }

    fn create_nav_item(&mut self, list: List) -> EntityMut<SingleSelect> {
        self.nav_model
            .insert()
            .text(list.name.clone())
            .icon(widget::icon::icon(
                widget::icon::from_name(list.clone().icon.unwrap())
                    .size(16)
                    .handle(),
            ))
            .data(list.clone())
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = Flags;
    type Message = Message;
    const APP_ID: &'static str = "com.system76.CosmicTodo";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, flags: Self::Flags) -> (Self, Command<CosmicMessage<Self::Message>>) {
        core.nav_bar_toggle_condensed();
        let nav_model = segmented_button::ModelBuilder::default();

        let app = App {
            core,
            nav_model: nav_model.build(),
            content: Content::new(),
            details: Details::new(),
            config_handler: flags.config_handler,
            config: flags.config,
            app_themes: vec![fl!("match-desktop"), fl!("dark"), fl!("light")],
            context_page: ContextPage::Settings,
            key_binds: key_binds(),
            modifiers: Modifiers::empty(),
            dialog_pages: VecDeque::new(),
            dialog_text_input: widget::Id::unique(),
            selected_list: None,
        };

        let commands = vec![Command::perform(fetch_lists(), |result| match result {
            Ok(data) => message::app(Message::PopulateLists(data)),
            Err(_) => message::none(),
        })];

        (app, Command::batch(commands))
    }

    fn on_escape(&mut self) -> Command<CosmicMessage<Self::Message>> {
        if self.dialog_pages.pop_front().is_some() {
            return Command::none();
        }

        self.core.window.show_context = false;

        Command::none()
    }

    fn context_drawer(&self) -> Option<Element<Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => self.about(),
            ContextPage::Settings => self.settings(),
            ContextPage::TaskDetails => self.details.view().map(Message::DetailsMessage),
        })
    }

    fn header_start(&self) -> Vec<Element<Self::Message>> {
        vec![menu::menu_bar(&self.key_binds)]
    }

    fn header_end(&self) -> Vec<Element<Self::Message>> {
        let add_list_button = widget::button::icon(
            widget::icon::from_name("list-add-symbolic")
                .size(16)
                .handle(),
        )
            .on_press(Message::OpenNewListDialog);
        vec![add_list_button.into()]
    }

    fn nav_model(&self) -> Option<&segmented_button::SingleSelectModel> {
        Some(&self.nav_model)
    }

    fn on_nav_select(&mut self, entity: Entity) -> Command<CosmicMessage<Self::Message>> {
        self.nav_model.activate(entity);
        let location_opt = self.nav_model.data::<List>(entity);

        if let Some(list) = location_opt {
            self.selected_list = Some(list.clone());
            let message = Message::ContentMessage(content::Message::List(Some(list.clone())));
            return self.update(message);
        }

        Command::none()
    }

    fn dialog(&self) -> Option<Element<Message>> {
        let dialog_page = match self.dialog_pages.front() {
            Some(some) => some,
            None => return None,
        };

        let cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;

        let dialog = match dialog_page {
            DialogPage::NewItem { name } => {
                let dialog = widget::dialog(fl!("new-list"));

                dialog
                    .primary_action(
                        widget::button::suggested(fl!("save"))
                            .on_press_maybe(Some(Message::DialogComplete)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
                    .control(
                        widget::column::with_children(vec![
                            widget::text::body(fl!("list-name")).into(),
                            widget::text_input("", name.as_str())
                                .id(self.dialog_text_input.clone())
                                .on_input(move |name| {
                                    Message::DialogUpdate(DialogPage::NewItem { name })
                                })
                                .into(),
                        ])
                            .spacing(space_xxs),
                    )
            }
            DialogPage::RenameItem {
                from: old_name,
                to: name,
            } => {
                let dialog = widget::dialog(fl!("rename-list"));

                dialog
                    .primary_action(
                        widget::button::suggested(fl!("save"))
                            .on_press_maybe(Some(Message::DialogComplete)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
                    .control(
                        widget::column::with_children(vec![
                            widget::text::body(fl!("list-name")).into(),
                            widget::text_input("", name.as_str())
                                .id(self.dialog_text_input.clone())
                                .on_input(move |name| {
                                    Message::DialogUpdate(DialogPage::RenameItem {
                                        from: old_name.clone(),
                                        to: name,
                                    })
                                })
                                .into(),
                        ])
                            .spacing(space_xxs),
                    )
            }
            DialogPage::DeleteItem { name } => {
                let dialog = widget::dialog(fl!("delete-list"));

                dialog
                    .primary_action(
                        widget::button::suggested(fl!("ok"))
                            .on_press_maybe(Some(Message::DialogComplete)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
            }
        };

        Some(dialog.into())
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct ThemeSubscription;

        let mut subscriptions = vec![
            event::listen_with(|event, status| match event {
                Event::Keyboard(KeyEvent::KeyPressed { key, modifiers, .. }) => match status {
                    event::Status::Ignored => Some(Message::Key(modifiers, key)),
                    event::Status::Captured => None,
                },
                Event::Keyboard(KeyEvent::ModifiersChanged(modifiers)) => {
                    Some(Message::Modifiers(modifiers))
                }
                _ => None,
            }),
            cosmic_config::config_subscription(
                TypeId::of::<ConfigSubscription>(),
                Self::APP_ID.into(),
                CONFIG_VERSION,
            )
                .map(|update| {
                    if !update.errors.is_empty() {
                        log::info!(
                        "errors loading config {:?}: {:?}",
                        update.keys,
                        update.errors
                    );
                    }
                    Message::SystemThemeModeChange(update.config)
                }),
            cosmic_config::config_subscription::<_, cosmic_theme::ThemeMode>(
                TypeId::of::<ThemeSubscription>(),
                cosmic_theme::THEME_MODE_ID.into(),
                cosmic_theme::ThemeMode::version(),
            )
                .map(|update| {
                    if !update.errors.is_empty() {
                        log::info!(
                        "errors loading theme mode {:?}: {:?}",
                        update.keys,
                        update.errors
                    );
                    }
                    Message::SystemThemeModeChange(update.config)
                }),
        ];

        subscriptions.push(
            self.content
                .subscription()
                .map(Message::ContentMessage),
        );

        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Self::Message) -> Command<CosmicMessage<Self::Message>> {
        // Helper for updating config values efficiently
        macro_rules! config_set {
            ($name: ident, $value: expr) => {
                match &self.config_handler {
                    Some(config_handler) => {
                        match paste::paste! { self.config.[<set_ $name>](config_handler, $value) } {
                            Ok(_) => {}
                            Err(err) => {
                                log::warn!(
                                    "failed to save config {:?}: {}",
                                    stringify!($name),
                                    err
                                );
                            }
                        }
                    }
                    None => {
                        self.config.$name = $value;
                        log::warn!(
                            "failed to save config {:?}: no config handler",
                            stringify!($name)
                        );
                    }
                }
            };
        }

        let mut commands = vec![];

        match message {
            Message::ContentMessage(message) => {
                let content_commands = self.content.update(message);
                for content_command in content_commands {
                    match content_command {
                        content::Command::GetTasks(list_id) => {
                            commands.push(Command::perform(fetch_tasks(list_id), |result| {
                                match result {
                                    Ok(data) => message::app(Message::ContentMessage(
                                        content::Message::SetItems(data),
                                    )),
                                    Err(_) => message::none(),
                                }
                            }));
                        }
                        content::Command::DisplayTask(task) => {
                            let entity =
                                self.details.priority_model.entity_at(task.priority as u16);
                            if let Some(entity) = entity {
                                self.details.priority_model.activate(entity);
                            }

                            self.details.task = Some(task.clone());
                            commands.push(
                                self.update(Message::ToggleContextPage(ContextPage::TaskDetails)),
                            );
                        }
                        content::Command::UpdateTask(task) => {
                            self.details.task = Some(task.clone());
                            let command =
                                Command::perform(update_task(task), |result| match result {
                                    Ok(_) => message::none(),
                                    Err(_) => message::none(),
                                });
                            commands.push(command);
                        }
                        content::Command::Delete(id) => {
                            if let Some(list) = &self.selected_list {
                                let command = Command::perform(
                                    delete_task(list.id.clone(), id.clone()),
                                    |result| match result {
                                        Ok(_) => message::none(),
                                        Err(_) => message::none(),
                                    },
                                );
                                commands.push(command);
                            }
                        }
                        content::Command::CreateTask(task) => {
                            let command =
                                Command::perform(create_task(task), |result| match result {
                                    Ok(_) => message::none(),
                                    Err(_) => message::none(),
                                });
                            commands.push(command);
                        }
                    }
                }
            }
            Message::DetailsMessage(message) => {
                let details_commands = self.details.update(message);
                for details_command in details_commands {
                    match details_command {
                        details::Command::Update(task) => {
                            commands.push(Command::perform(
                                update_task(task),
                                |result| match result {
                                    Ok(_) => message::none(),
                                    Err(_) => message::none(),
                                },
                            ));
                        }
                        details::Command::Rename(id, title) => {
                            commands.push(self.update(Message::ContentMessage(
                                content::Message::Rename(id.clone(), title.clone()),
                            )));
                        }
                        details::Command::Delete(_) => {}
                        details::Command::Complete(_, _) => {}
                        details::Command::Favorite(_, _) => {}
                        details::Command::PriorityActivate(id, priority) => {
                            commands.push(self.update(Message::ContentMessage(
                                content::Message::SetPriority(id.clone(), priority),
                            )));
                        }
                    }
                }
            }
            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page.clone();
                    self.core.window.show_context = true;
                }
                self.set_context_title(context_page.clone().title());
            }
            Message::WindowClose => {
                return window::close(window::Id::MAIN);
            }
            Message::WindowNew => match env::current_exe() {
                Ok(exe) => match process::Command::new(&exe).spawn() {
                    Ok(_child) => {}
                    Err(err) => {
                        eprintln!("failed to execute {:?}: {}", exe, err);
                    }
                },
                Err(err) => {
                    eprintln!("failed to get current executable path: {}", err);
                }
            },
            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    log::warn!("failed to open {:?}: {}", url, err);
                }
            },
            Message::AppTheme(app_theme) => {
                config_set!(app_theme, app_theme);
                return self.update_config();
            }
            Message::SystemThemeModeChange(_) => {
                return self.update_config();
            }
            Message::PopulateLists(lists) => {
                for list in lists {
                    self.create_nav_item(list);
                }
                let Some(entity) = self.nav_model.iter().next() else {
                    return Command::none();
                };
                self.nav_model.activate(entity);
                let command = self.on_nav_select(entity);
                commands.push(command);
            }
            Message::Key(modifiers, key) => {
                for (key_bind, action) in self.key_binds.iter() {
                    if key_bind.matches(modifiers, &key) {
                        return self.update(action.message(None));
                    }
                }
            }
            Message::Modifiers(modifiers) => {
                self.modifiers = modifiers;
            }
            Message::OpenNewListDialog => {
                self.dialog_pages.push_back(DialogPage::NewItem {
                    name: String::new(),
                });
                return widget::text_input::focus(self.dialog_text_input.clone());
            }
            Message::OpenRenameListDialog => {
                if let Some(list) = &self.selected_list {
                    self.dialog_pages.push_back(DialogPage::RenameItem {
                        from: list.name.clone(),
                        to: list.name.clone(),
                    });
                    return widget::text_input::focus(self.dialog_text_input.clone());
                }
            }
            Message::OpenDeleteListDialog => {
                if let Some(list) = &self.selected_list {
                    self.dialog_pages.push_back(DialogPage::DeleteItem {
                        name: list.name.clone(),
                    });
                }
            }
            Message::AddList(list) => {
                self.selected_list = Some(list.clone());
                self.create_nav_item(list);
                let Some(entity) = self.nav_model.iter().last() else {
                    return Command::none();
                };
                let command = self.on_nav_select(entity);
                commands.push(command);
            }
            Message::DeleteList => {
                if let Some(list) = &self.selected_list {
                    let entity = self.nav_model.iter().find(|entity| {
                        if let Some(data) = self.nav_model.data::<List>(*entity) {
                            return data.id == list.id;
                        }
                        false
                    });

                    if let Some(entity) = entity {
                        self.nav_model.remove(entity);
                    }

                    let command =
                        Command::perform(delete_list(list.id.clone()), |result| match result {
                            Ok(_) => message::none(),
                            Err(_) => message::none(),
                        });

                    commands
                        .push(self.update(Message::ContentMessage(content::Message::List(None))));

                    commands.push(command);

                    self.selected_list = None;
                }
            }
            Message::UpdateList(list) => {
                let entity = self.nav_model.active();
                self.nav_model.data_set(entity, list);
            }
            Message::DialogCancel => {
                self.dialog_pages.pop_front();
            }
            Message::DialogComplete => {
                if let Some(dialog_page) = self.dialog_pages.pop_front() {
                    match dialog_page {
                        DialogPage::NewItem { name } => {
                            let list = List::new(&name, Service::Computer);
                            commands.push(Command::perform(
                                create_list(list),
                                |result| match result {
                                    Ok(list) => message::app(Message::AddList(list)),
                                    Err(_) => message::none(),
                                },
                            ));
                        }
                        DialogPage::RenameItem {
                            from: old_name,
                            to: name,
                        } => {
                            if let Some(list) = &self.selected_list {
                                let mut list = list.clone();
                                list.name = name;
                                // TODO: Update model.
                                let command =
                                    Command::perform(update_list(list.clone()), move |result| {
                                        match result {
                                            Ok(_) => {
                                                message::app(Message::UpdateList(list.clone()))
                                            }
                                            Err(_) => message::none(),
                                        }
                                    });
                                commands.push(command);
                            }
                        }
                        DialogPage::DeleteItem { .. } => {
                            if let Some(list) = &self.selected_list {
                                commands.push(self.update(Message::DeleteList));
                            }
                        }
                    }
                }
            }
            Message::DialogUpdate(dialog_page) => {
                //TODO: panicless way to do this?
                self.dialog_pages[0] = dialog_page;
            }
        }

        if !commands.is_empty() {
            return Command::batch(commands);
        }

        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        let mut content_column = widget::column::with_capacity(1);
        let content_view = self
            .content
            .view()
            .map(Message::ContentMessage);
        content_column = content_column.push(content_view);

        let content: Element<_> = content_column.into();

        content
    }
}

async fn update_list(list: List) -> Result<(), Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service.update_list(list).await?)
}

async fn delete_list(id: String) -> Result<(), Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service.delete_list(id).await?)
}

async fn create_list(list: List) -> Result<List, Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service.create_list(list).await?)
}

async fn create_task(task: Task) -> Result<(), Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service.create_task(task).await?)
}

async fn fetch_lists() -> Result<Vec<List>, Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service.read_lists().await.unwrap_or(vec![]))
}

async fn fetch_tasks(list_id: String) -> Result<Vec<Task>, Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service
        .read_tasks_from_list(list_id)
        .await
        .unwrap_or(vec![]))
}

async fn update_task(task: Task) -> Result<Task, Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service.update_task(task).await?)
}

async fn delete_task(list_id: String, task_id: String) -> Result<(), Box<dyn Error>> {
    let mut service = Service::Computer.get_service();
    Ok(service.delete_task(list_id, task_id).await?)
}
