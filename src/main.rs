// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use appstream::Collection;
use cosmic::{
    app::{Command, Core, Settings},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{subscription::Subscription, window, Alignment, Length},
    widget, Application, ApplicationExt, Element,
};
use std::{any::TypeId, env, process, sync::Arc};

use appstream_cache::AppstreamCache;
mod appstream_cache;

use backend::{Backend, Package};
mod backend;

use config::{AppTheme, Config, CONFIG_VERSION};
mod config;

mod localize;

/// Runs application with these settings
#[rustfmt::skip]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(unix, not(target_os = "redox")))]
    match fork::daemon(true, true) {
        Ok(fork::Fork::Child) => (),
        Ok(fork::Fork::Parent(_child_pid)) => process::exit(0),
        Err(err) => {
            eprintln!("failed to daemonize: {:?}", err);
            process::exit(1);
        }
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    localize::localize();

    let (config_handler, config) = match cosmic_config::Config::new(App::APP_ID, CONFIG_VERSION) {
        Ok(config_handler) => {
            let config = match Config::get_entry(&config_handler) {
                Ok(ok) => ok,
                Err((errs, config)) => {
                    log::info!("errors loading config: {:?}", errs);
                    config
                }
            };
            (Some(config_handler), config)
        }
        Err(err) => {
            log::error!("failed to create config handler: {}", err);
            (None, Config::default())
        }
    };

    let mut settings = Settings::default();
    settings = settings.theme(config.app_theme.theme());

    #[cfg(target_os = "redox")]
    {
        // Redox does not support resize if doing CSDs
        settings = settings.client_decorations(false);
    }

    //TODO: allow size limits on iced_winit
    //settings = settings.size_limits(Limits::NONE.min_width(400.0).min_height(200.0));

    let flags = Flags {
        config_handler,
        config,
    };
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

fn get_translatable<'a>(translatable: &'a appstream::TranslatableString, locale: &str) -> &'a str {
    match translatable.get_for_locale(locale) {
        Some(some) => some.as_str(),
        None => match translatable.get_default() {
            Some(some) => some.as_str(),
            None => "",
        },
    }
}

fn get_markup_translatable<'a>(
    translatable: &'a appstream::MarkupTranslatableString,
    locale: &str,
) -> &'a str {
    match translatable.get_for_locale(locale) {
        Some(some) => some.as_str(),
        None => match translatable.get_default() {
            Some(some) => some.as_str(),
            None => "",
        },
    }
}

#[derive(Clone, Debug)]
pub struct Flags {
    config_handler: Option<cosmic_config::Config>,
    config: Config,
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    Todo,
    AppTheme(AppTheme),
    Config(Config),
    SelectInstalled(usize),
    SelectNone,
    SystemThemeModeChange(cosmic_theme::ThemeMode),
    ToggleContextPage(ContextPage),
    WindowClose,
    WindowNew,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPage {
    Settings,
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            Self::Settings => fl!("settings"),
        }
    }
}

/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    locale: String,
    app_themes: Vec<String>,
    appstream_cache: Arc<AppstreamCache>,
    backends: Vec<Box<dyn Backend>>,
    context_page: ContextPage,
    installed: Vec<(usize, Package)>,
    current_package: Option<(usize, Package, Collection)>,
}

impl App {
    fn update_config(&mut self) -> Command<Message> {
        cosmic::app::command::set_theme(self.config.app_theme.theme())
    }

    fn update_title(&mut self) -> Command<Message> {
        let title = "COSMIC App Store";
        self.set_header_title(title.to_string());
        self.set_window_title(title.to_string())
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
}

/// Implement [`Application`] to integrate with COSMIC.
impl Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received
    type Flags = Flags;

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicStore";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(core: Core, flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let locale = sys_locale::get_locale().unwrap_or_else(|| {
            log::warn!("failed to get system locale, falling back to en-US");
            String::from("en-US")
        });
        let app_themes = vec![fl!("match-desktop"), fl!("dark"), fl!("light")];
        let appstream_cache = Arc::new(AppstreamCache::new());
        let backends = backend::backends(&appstream_cache, &locale);
        let mut app = App {
            core,
            config_handler: flags.config_handler,
            config: flags.config,
            locale,
            app_themes,
            appstream_cache,
            backends,
            context_page: ContextPage::Settings,
            installed: Vec::new(),
            current_package: None,
        };

        //TODO: move to command, ability to refresh
        for (backend_i, backend) in app.backends.iter().enumerate() {
            match backend.installed() {
                Ok(installed) => {
                    for package in installed {
                        app.installed.push((backend_i, package));
                    }
                }
                Err(err) => {
                    log::error!("failed to list installed: {}", err);
                }
            }
        }
        app.installed
            .sort_by(|a, b| lexical_sort::natural_lexical_cmp(&a.1.name, &b.1.name));

        let command = app.update_title();
        (app, command)
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
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

        match message {
            Message::Todo => {
                log::warn!("TODO");
            }
            Message::AppTheme(app_theme) => {
                config_set!(app_theme, app_theme);
                return self.update_config();
            }
            Message::Config(config) => {
                if config != self.config {
                    log::info!("update config");
                    //TODO: update syntax theme by clearing tabs, only if needed
                    self.config = config;
                    return self.update_config();
                }
            }
            Message::SelectInstalled(installed_i) => {
                if let Some((backend_i, package)) = self.installed.get(installed_i) {
                    if let Some(backend) = self.backends.get(*backend_i) {
                        //TODO: do async
                        match backend.appstream(&package) {
                            Ok(appstream) => {
                                self.current_package =
                                    Some((*backend_i, package.clone(), appstream));
                            }
                            Err(err) => {
                                log::error!(
                                    "failed to get appstream data for {}: {}",
                                    package.id,
                                    err
                                );
                            }
                        }
                    }
                }
            }
            Message::SelectNone => {
                self.current_package = None;
            }
            Message::SystemThemeModeChange(_theme_mode) => {
                return self.update_config();
            }
            Message::ToggleContextPage(context_page) => {
                //TODO: ensure context menus are closed
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
                self.set_context_title(context_page.title());
            }
            Message::WindowClose => {
                return window::close(window::Id::MAIN);
            }
            Message::WindowNew => match env::current_exe() {
                Ok(exe) => match process::Command::new(&exe).spawn() {
                    Ok(_child) => {}
                    Err(err) => {
                        log::error!("failed to execute {:?}: {}", exe, err);
                    }
                },
                Err(err) => {
                    log::error!("failed to get current executable path: {}", err);
                }
            },
        }

        Command::none()
    }

    fn context_drawer(&self) -> Option<Element<Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::Settings => self.settings(),
        })
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message> {
        let cosmic_theme::Spacing {
            space_xs,
            space_xxs,
            ..
        } = self.core().system_theme().cosmic().spacing;

        let content: Element<_> = match &self.current_package {
            Some((backend_i, package, appstream)) => {
                //TODO: capacity may go over due to summary
                let mut column = widget::column::with_capacity(appstream.components.len() + 2)
                    // Hack to make room for scroll bar
                    .padding([0, space_xs, 0, 0])
                    .spacing(space_xxs)
                    .width(Length::Fill);
                column = column.push(widget::button("Back").on_press(Message::SelectNone));
                column = column.push(
                    widget::row::with_children(vec![
                        package.icon.clone().size(32).into(),
                        widget::text(&package.name).into(),
                        widget::horizontal_space(Length::Fill).into(),
                        widget::text(&package.version).into(),
                    ])
                    .align_items(Alignment::Center)
                    .spacing(space_xxs),
                );
                for component in appstream.components.iter() {
                    column = column.push(widget::text(get_translatable(
                        &component.name,
                        &self.locale,
                    )));
                    if let Some(summary) = &component.summary {
                        column = column.push(widget::text(get_translatable(summary, &self.locale)));
                    }
                    /*TODO: MarkupTranslatableString doesn't properly filter p tag with xml:lang
                    if let Some(description) = &component.description {
                        column = column.push(widget::text(get_markup_translatable(
                            description,
                            &self.locale,
                        )));
                    }
                    */
                }
                widget::scrollable(column).into()
            }
            None => {
                let mut column = widget::column::with_capacity(self.installed.len() + 1)
                    .padding([0, space_xs, 0, 0])
                    .spacing(space_xxs)
                    .width(Length::Fill);
                column = column.push(widget::text("Installed:"));
                for (installed_i, (_backend_i, package)) in self.installed.iter().enumerate() {
                    column = column.push(
                        widget::mouse_area(
                            widget::row::with_children(vec![
                                package.icon.clone().size(32).into(),
                                widget::text(&package.name).into(),
                                widget::horizontal_space(Length::Fill).into(),
                                widget::text(&package.version).into(),
                            ])
                            .align_items(Alignment::Center)
                            .spacing(space_xxs),
                        )
                        .on_press(Message::SelectInstalled(installed_i)),
                    );
                }
                widget::scrollable(column).into()
            }
        };

        // Uncomment to debug layout:
        //content.explain(cosmic::iced::Color::WHITE)
        content
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct ThemeSubscription;

        Subscription::batch([
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
        ])
    }
}
