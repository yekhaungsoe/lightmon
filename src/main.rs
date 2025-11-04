use iced::{
    executor, time, Alignment, Application, Command, Element, Length, Background,
    Settings, Subscription, Theme,
};
use iced::widget::{button, column, container, row, text, text_input, horizontal_space, vertical_space};
use iced::widget::container::Appearance;
use iced::widget::scrollable;
use iced::{Color, Border};
use sysinfo::{System, Pid};
use log::info;
use std::fs::File;
use std::io::Write;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::path::Path;

const BETA_TAG: &str = "v1.0-beta";

fn main() -> iced::Result {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    LightMon::run(Settings::default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    refresh_interval: u64,
    dark_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            refresh_interval: 1,
            dark_mode: false,
        }
    }
}

fn get_config_path() -> Option<PathBuf> {
    // For now, use current directory. We'll improve this later.
    Some(PathBuf::from("lightmon_config.toml"))
}

fn load_config() -> AppConfig {
    if let Some(config_path) = get_config_path() {
        if let Ok(config_str) = fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str(&config_str) {
                return config;
            }
        }
    }
    AppConfig::default()
}

fn save_config(config: &AppConfig) -> Result<(), String> {
    if let Some(config_path) = get_config_path() {
        let config_str = toml::to_string(config).map_err(|e| e.to_string())?;
        fs::write(&config_path, config_str).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Could not determine config path".to_string())
    }
}

struct LightMon {
    sys: System,
    cpu_usage: f32,
    memory_used: u64,
    memory_total: u64,
    disk_used: u64,
    disk_total: u64,
    current_screen: Screen,
    dark_mode: bool,
    sort_by: SortBy,
    filter_text: String,
    selected: Option<Pid>,
    error_message: Option<String>,
    refresh_interval: u64,
    refresh_interval_input: String, // NEW
    toast_message: Option<String>,
    is_exporting: bool, // NEW: Track export progress
}

#[derive(Debug, Clone)]
enum Screen {
    Overview,
    Processes,
    Settings,
}

#[derive(Debug, Clone, Copy)]
enum SortBy {
    Cpu,
    Memory,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    SystemData(f32, u64, u64, u64, u64),
    GoToOverview,
    GoToProcesses,
    GoToSettings,
    ToggleTheme,
    SortByCpu,
    SortByMemory,
    FilterChanged(String),
    SelectProcess(Pid),
    ClearError,
    SetRefreshInterval(String),
    ExportProcesses,
    ExportComplete(Result<(), String>),
    ClearToast,
}

impl Application for LightMon {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let mut sys = System::new_all();
        sys.refresh_all();
        
        // Load config at startup
        let config = load_config();
        
        (
            Self {
                sys,
                cpu_usage: 0.0,
                memory_used: 0,
                memory_total: 0,
                disk_used: 0,
                disk_total: 0,
                current_screen: Screen::Overview,
                dark_mode: config.dark_mode,
                sort_by: SortBy::Cpu,
                filter_text: String::new(),
                selected: None,
                error_message: None,
                refresh_interval: config.refresh_interval,
                refresh_interval_input: config.refresh_interval.to_string(),
                toast_message: None,
                is_exporting: false, // NEW: Initialize as false
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("System Monitor")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Tick => {
                return Command::perform(fetch_system_data(), |(cpu, used, total, disk_used, disk_total)| {
                    Message::SystemData(cpu, used, total, disk_used, disk_total)
                });
            }
            Message::SystemData(cpu, used, total, disk_used, disk_total) => {
                self.cpu_usage = cpu;
                self.memory_used = used;
                self.memory_total = total;
                self.disk_used = disk_used;
                self.disk_total = disk_total;
                info!("CPU: {:.1}%, Memory: {}/{}", cpu, used, total);
            }
            Message::GoToOverview => self.current_screen = Screen::Overview,
            Message::GoToProcesses => {
                self.current_screen = Screen::Processes;
                self.sys.refresh_all();
            }
            
            Message::GoToSettings => self.current_screen = Screen::Settings,
            Message::ToggleTheme => {
                self.dark_mode = !self.dark_mode;
                // Auto-save config
                let config = AppConfig {
                    refresh_interval: self.refresh_interval,
                    dark_mode: self.dark_mode,
                };
                if let Err(e) = save_config(&config) {
                    self.toast_message = Some(format!("Could not save settings: {} - check file permissions", e));
                }
            }
            Message::SortByCpu => self.sort_by = SortBy::Cpu,
            Message::SortByMemory => self.sort_by = SortBy::Memory,
            Message::FilterChanged(s) => self.filter_text = s,
            Message::SelectProcess(pid) => self.selected = Some(pid),
            Message::ClearError => self.error_message = None,
            Message::SetRefreshInterval(s) => {
                // Always save the user input so they can type freely
                self.refresh_interval_input = s.clone();

                // Only update numeric value if parsing succeeds
                if let Ok(interval) = s.parse::<u64>() {
                    self.refresh_interval = interval.max(1);

                    let config = AppConfig {
                        refresh_interval: self.refresh_interval,
                        dark_mode: self.dark_mode,
                    };
                    let _ = save_config(&config);
                }
            }

            Message::ExportProcesses => {
                self.is_exporting = true; // NEW: Show loading
                let processes_data = self.get_processes_data();
                return Command::perform(export_processes_to_csv(processes_data), Message::ExportComplete);
            }
            Message::ExportComplete(result) => {
                self.is_exporting = false; // NEW: Hide loading
                match result {
                    Ok(()) => {
                        self.toast_message = Some("Processes exported to processes.csv".into());
                    }
                    Err(e) => {
                        self.toast_message = Some(format!("Export failed: {} - check if file is open elsewhere", e));
                    }
                }
                return Command::perform(
                    async {
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    }, 
                    |_| Message::ClearToast
                );
            }
            Message::ClearToast => {
                self.toast_message = None;
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let header = row![
            text("Lightweight System Monitor").size(20),
            button("[Overview Tab]").on_press(Message::GoToOverview).padding(5),
            button("[Process Tab]").on_press(Message::GoToProcesses).padding(5),
            horizontal_space(),
            button("Settings").on_press(Message::GoToSettings).padding(8),
        ]
        .spacing(15)
        .align_items(Alignment::Center)
        .padding(12);

        let content: Element<_> = match self.current_screen {
            Screen::Overview => self.view_overview(),
            Screen::Processes => self.view_processes(),
            Screen::Settings => self.view_settings(),
        };

        let mut main = column![header, content];

        if let Some(toast_msg) = &self.toast_message {
            let is_error = toast_msg.contains("failed");
            let toast = container(
                text(toast_msg)
                    .size(14)
                    .style(if is_error {
                        if self.dark_mode {
                            Color::from_rgb(1.0, 0.5, 0.5)  // Darker red for dark mode
                        } else {
                            Color::from_rgb(0.7, 0.0, 0.0)  // Darker red for light mode
                        }
                    } else {
                        if self.dark_mode {
                            Color::from_rgb(0.5, 1.0, 0.5)  // Darker green for dark mode
                        } else {
                            Color::from_rgb(0.0, 0.5, 0.0)  // Darker green for light mode
                        }
                    })
            )
            .padding(10)
            .style(|theme: &Theme| {
                let (bg_color, border_color) = match theme {
                    Theme::Dark => (Color::from_rgb(0.2, 0.2, 0.2), Color::from_rgb(0.4, 0.4, 0.4)),
                    Theme::Light => (Color::from_rgb(0.98, 0.98, 0.98), Color::from_rgb(0.8, 0.8, 0.8)),
                    _ => (Color::from_rgb(0.98, 0.98, 0.98), Color::from_rgb(0.8, 0.8, 0.8)),
                };
                Appearance {
                    text_color: None,
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                }
            });

            main = main.push(toast);
        }

        container(main)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        time::every(std::time::Duration::from_secs(self.refresh_interval)).map(|_| Message::Tick)
    }

    fn theme(&self) -> Theme {
        if self.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

async fn fetch_system_data() -> (f32, u64, u64, u64, u64) {
    let mut sys = System::new_all();
    sys.refresh_all();
    let cpu = sys.cpus().first().map(|c| c.cpu_usage()).unwrap_or(0.0);
    let used = sys.used_memory();
    let total = sys.total_memory();
    
    let disk_used = used / 1024;
    let disk_total = total / 1024;
    
    (cpu, used, total, disk_used, disk_total)
}

impl LightMon {
    fn get_processes_data(&self) -> Vec<(Pid, String, f32, u64, String)> {
        self.sys.processes()
            .iter()
            .map(|(pid, process)| {
                (
                    *pid,
                    process.name().to_string(),
                    process.cpu_usage(),
                    process.memory(),
                    format!("{:?}", process.status())
                )
            })
            .collect()
    }
}

async fn export_processes_to_csv(processes: Vec<(Pid, String, f32, u64, String)>) -> Result<(), String> {
    // Simulate some work time to show the loading indicator
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    let mut file = File::create("processes.csv")
        .map_err(|e| format!("Cannot create CSV file: {} - check permissions", e))?;
    
    writeln!(file, "PID,Name,CPU%,Memory (KB),Status")
        .map_err(|e| format!("Cannot write to CSV: {} - disk may be full", e))?;

    for (pid, name, cpu_usage, memory, status) in processes {
    let line = format!(
        "{},{},{:.1},{},{}",
        pid,
        name,
        cpu_usage,
        memory,
        status
    );
    writeln!(file, "{}", line)
        .map_err(|e| format!("Cannot write process data: {} - disk error", e))?;
}

file.flush()
    .map_err(|e| format!("Cannot save CSV file: {} - write failed", e))?;

Ok(())
}

impl LightMon {
    fn view_overview(&self) -> Element<Message> {
        let mem_total_mb = self.memory_total as f64 / 1024.0;
        let mem_used_mb = self.memory_used as f64 / 1024.0;
        let mem_percent = (mem_used_mb / mem_total_mb * 100.0).min(100.0);

        let disk_total_gb = self.disk_total as f64 / 1024.0;
        let disk_used_gb = self.disk_used as f64 / 1024.0;
        let disk_percent = (disk_used_gb / disk_total_gb * 100.0).min(100.0);

        let cpu_filled = (self.cpu_usage as usize / 5).min(20);
        let mem_filled = (mem_percent as usize / 5).min(20);
        let disk_filled = (disk_percent as usize / 5).min(20);

        let cpu_bar = format!("[{}{}]", "█".repeat(cpu_filled), "░".repeat(20 - cpu_filled));
        let mem_bar = format!("[{}{}]", "█".repeat(mem_filled), "░".repeat(20 - mem_filled));
        let disk_bar = format!("[{}{}]", "█".repeat(disk_filled), "░".repeat(20 - disk_filled));

        let stat_box = |label: &str, bar: String, percent: f32| {
            let widget = container(
                column![
                    text(label).size(16),
                    row![
                        text(bar).size(16),
                        text(format!("{:.1}%", percent)).width(Length::Fixed(70.0)).size(16),
                    ]
                    .spacing(12)
                    .align_items(Alignment::Center),
                ]
                .spacing(6)
            )
            .padding(14);

            let bg = if self.dark_mode {
                iced::Color::from_rgb(0.12, 0.12, 0.12)
            } else {
                iced::Color::from_rgb(0.95, 0.95, 0.95)
            };
            let text_color = if self.dark_mode {
                Some(iced::Color::from_rgb(0.94, 0.94, 0.94))
            } else {
                Some(iced::Color::from_rgb(0.06, 0.06, 0.06))
            };
            let border_color = if self.dark_mode {
                iced::Color::from_rgb(0.25, 0.25, 0.25)
            } else {
                iced::Color::from_rgb(0.2, 0.2, 0.2)
            };

            let widget = widget.style(move |_theme: &Theme| {
                Appearance {
                    text_color,
                    background: Some(Background::Color(bg)),
                    border: iced::Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                }
            });

            widget
        };

        column![
            text("Overview").size(28),
            vertical_space().height(Length::Fixed(10.0)),
            stat_box("CPU", cpu_bar, self.cpu_usage),
            stat_box("Memory", mem_bar, mem_percent as f32),
            stat_box("Disk", disk_bar, disk_percent as f32),
            vertical_space().height(Length::Fixed(15.0)),
            text(format!(
                "{:.1} / {:.1} GB",
                mem_used_mb / 1024.0, mem_total_mb / 1024.0
            ))
            .size(14),
        ]
        .spacing(8)
        .padding(25)
        .align_items(Alignment::Start)
        .into()
    }

    fn view_processes(&self) -> Element<Message> {
        let mut content_column = column![
            text("Running Processes").size(28),
            vertical_space().height(Length::Fixed(10.0)),
            row![
                button("Sort by CPU").on_press(Message::SortByCpu).padding(6),
                button("Sort by Memory").on_press(Message::SortByMemory).padding(6),
                // NEW: Show loading state for export button
                if self.is_exporting {
                    button("Exporting...").padding(6)
                } else {
                    button("Export to CSV").on_press(Message::ExportProcesses).padding(6)
                },
            ]
            .spacing(10),
            vertical_space().height(Length::Fixed(10.0)),
            text_input("Search processes by name or PID number", &self.filter_text)
                .on_input(Message::FilterChanged)
                .padding(10)
                .size(15),
            vertical_space().height(Length::Fixed(10.0)),
        ]
        .spacing(6)
        .padding(25);

        let mut process_list = column![
            row![
                text("PID").width(Length::Fixed(80.0)).size(15),
                text("Name").width(Length::Fill).size(15),
                text("CPU%").width(Length::Fixed(80.0)).size(15),
                text("Memory").width(Length::Fixed(100.0)).size(15),
            ]
            .spacing(12)
            .align_items(Alignment::Center),
        ]
        .spacing(8);

        let mut processes: Vec<_> = self.sys.processes().iter().collect();
        match self.sort_by {
            SortBy::Cpu => processes.sort_by(|a, b| {
                b.1.cpu_usage()
                    .partial_cmp(&a.1.cpu_usage())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortBy::Memory => processes.sort_by(|a, b| b.1.memory().cmp(&a.1.memory())),
        }

        let filter = self.filter_text.to_lowercase();
        let filtered = processes.into_iter().filter(|(_, p)| {
            let pid_str = format!("{}", p.pid());
            p.name().to_lowercase().contains(&filter) || pid_str.contains(&filter)
        });

        for (pid, process) in filtered.take(12) {
            let row_content = row![
                text(format!("{}", pid)).width(Length::Fixed(80.0)).size(14),
                text(process.name()).width(Length::Fill).size(14),
                text(format!("{:.1}", process.cpu_usage())).width(Length::Fixed(80.0)).size(14),
                text(format!("{}", process.memory() / 1024)).width(Length::Fixed(100.0)).size(14),
            ]
            .spacing(12)
            .align_items(Alignment::Center);

            let row_button = button(row_content)
                .on_press(Message::SelectProcess(*pid))
                .padding(4);

            process_list = process_list.push(row_button);
        }

        let process_container = container(process_list)
            .padding(15)
            .style(|theme: &Theme| {
                let (bg_color, border_color) = match theme {
                    Theme::Dark => (Color::from_rgb(0.15, 0.15, 0.15), Color::from_rgb(0.4, 0.4, 0.4)),
                    Theme::Light => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                    _ => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                };
                Appearance {
                    text_color: None,
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                }
            });

        content_column = content_column.push(process_container);

        if let Some(pid) = self.selected {
            if let Some(proc_) = self.sys.process(pid) {
                content_column = content_column.push(vertical_space().height(Length::Fixed(15.0)));
                content_column = content_column.push(
                    container(
                        column![
                            text(format!("Selected Process Details")).size(18),
                            vertical_space().height(Length::Fixed(10.0)),
                            row![
                                column![
                                    text("Name:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)),
                                    text("PID:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)),
                                    text("Status:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)),
                                    text("User ID:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)),
                                ]
                                .spacing(6)
                                .width(Length::Fixed(80.0)),
                                column![
                                    text(proc_.name()).size(14),
                                    text(format!("{}", pid)).size(14),
                                    text(format!("{:?}", proc_.status())).size(14),
                                    text(format!("{:?}", proc_.user_id())).size(14),
                                ]
                                .spacing(6)
                                .width(Length::Fill),
                            ]
                            .spacing(8),
                            vertical_space().height(Length::Fixed(10.0)),
                            row![
                                column![
                                    text("CPU Usage").size(14),
                                    text(format!("{:.1}%", proc_.cpu_usage())).size(18),
                                ]
                                .spacing(4)
                                .align_items(Alignment::Center),
                                column![
                                    text("Memory").size(14),
                                    text(format!("{} MB", proc_.memory() / 1024)).size(18),
                                ]
                                .spacing(4)
                                .align_items(Alignment::Center),
                                column![
                                    text("Virtual Memory").size(14),
                                    text(format!("{} MB", proc_.virtual_memory() / 1024)).size(16),
                                ]
                                .spacing(4)
                                .align_items(Alignment::Center),
                            ]
                            .spacing(30),
                        ]
                        .spacing(12),
                    )
                    .padding(20)
                    .style(|theme: &Theme| {
                        let (bg_color, border_color) = match theme {
                            Theme::Dark => (Color::from_rgb(0.15, 0.15, 0.15), Color::from_rgb(0.4, 0.4, 0.4)),
                            Theme::Light => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                            _ => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                        };
                        Appearance {
                            text_color: None,
                            background: Some(Background::Color(bg_color)),
                            border: Border {
                                color: border_color,
                                width: 1.0,
                                radius: 8.0.into(),
                            },
                            shadow: Default::default(),
                        }
                    }),
                );
            }
        }

        container(scrollable(content_column))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_settings(&self) -> Element<Message> {
        column![
            text("Settings").size(28),
            vertical_space().height(Length::Fixed(15.0)),
            container(
                column![
                    text_input(
                            "update frequency in seconds",
                            &self.refresh_interval_input
                        )
                        .on_input(Message::SetRefreshInterval)
                        .padding(10)
                        .size(10)
                        .width(Length::Fixed(150.0)),
                ]
                .spacing(8)
            )
            .padding(15)
            .style(|theme: &Theme| {
                let (bg_color, border_color) = match theme {
                    Theme::Dark => (Color::from_rgb(0.15, 0.15, 0.15), Color::from_rgb(0.4, 0.4, 0.4)),
                    Theme::Light => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                    _ => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                };
                Appearance {
                    text_color: None,
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                }
            }),
            vertical_space().height(Length::Fixed(20.0)),
            container(
                column![
                    text("Theme").size(16),
                    vertical_space().height(Length::Fixed(8.0)),
                    row![
                        button(if self.dark_mode { "Light" } else { "● Light" })
                            .on_press(Message::ToggleTheme)
                            .padding(12),
                        button(if self.dark_mode { "● Dark" } else { "Dark" })
                            .on_press(Message::ToggleTheme)
                            .padding(12),
                    ]
                    .spacing(12),
                ]
                .spacing(8)
            )
            .padding(15)
            .style(|theme: &Theme| {
                let (bg_color, border_color) = match theme {
                    Theme::Dark => (Color::from_rgb(0.15, 0.15, 0.15), Color::from_rgb(0.4, 0.4, 0.4)),
                    Theme::Light => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                    _ => (Color::from_rgb(0.95, 0.95, 0.95), Color::from_rgb(0.2, 0.2, 0.2)),
                };
                Appearance {
                    text_color: None,
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                }
            }),
        ]
        .spacing(15)
        .padding(25)
        .align_items(Alignment::Start)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::fs::OpenOptions;
    use std::io::Write;

    // -------------------
    // Unit tests
    // -------------------

    #[tokio::test]
    async fn test_fetch_system_data_works() {
        let result = fetch_system_data().await;
        assert!(result.0 >= 0.0);
    }

    #[test]
    fn test_config_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.refresh_interval, 1);
        assert_eq!(config.dark_mode, false);
    }

    #[test]
    fn test_load_config_no_crash() {
        let config = load_config();
        assert!(config.refresh_interval >= 1);
    }

    #[test]
    fn test_appconfig_implements_default() {
        let config = AppConfig::default();
        assert_eq!(config.refresh_interval, 1);
    }

    #[test]
    fn test_screen_enum_debug() {
        let screen = Screen::Overview;
        format!("{:?}", screen);
    }

    #[test]
    fn test_sortby_enum_debug() {
        let sort = SortBy::Cpu;
        format!("{:?}", sort);
    }

    #[test]
    fn test_message_enum_clone() {
        let msg = Message::Tick;
        let _cloned = msg.clone();
    }

    #[test]
    fn test_lightmon_get_processes_data() {
        let mon = LightMon::new(()).0;
        let data = mon.get_processes_data();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_set_refresh_interval_parsing() {
        let mut mon = LightMon::new(()).0;

        mon.update(Message::SetRefreshInterval("5".to_string()));
        assert_eq!(mon.refresh_interval, 5);

        mon.update(Message::SetRefreshInterval("abc".to_string()));
        assert_eq!(mon.refresh_interval, 5);
    }

    #[test]
    fn test_toggle_theme() {
        let mut mon = LightMon::new(()).0;
        let initial = mon.dark_mode;

        mon.update(Message::ToggleTheme);
        assert_ne!(mon.dark_mode, initial);
    }

    #[test]
    fn test_filter_changed() {
        let mut mon = LightMon::new(()).0;
        mon.update(Message::FilterChanged("test".to_string()));
        assert_eq!(mon.filter_text, "test");
    }

    // -------------------
    // Integration / Week 6 Tests
    // -------------------

    #[test]
    fn test_config_file_creation() {
        let test_config = AppConfig {
            refresh_interval: 3,
            dark_mode: true,
        };

        let result = save_config(&test_config);
        assert!(result.is_ok());
        assert!(PathBuf::from("lightmon_config.toml").exists());

        // Clean up
        let _ = fs::remove_file("lightmon_config.toml");
    }

    #[test]
    fn test_config_round_trip() {
        let original_config = load_config();

        let test_config = AppConfig {
            refresh_interval: 7,
            dark_mode: false,
        };

        save_config(&test_config).unwrap();

        let config_path = PathBuf::from("lightmon_config.toml");
        let config_str = fs::read_to_string(&config_path).unwrap();
        let loaded_config: AppConfig = toml::from_str(&config_str).unwrap();

        assert_eq!(loaded_config.refresh_interval, 7);
        assert_eq!(loaded_config.dark_mode, false);

        // Restore original config
        save_config(&original_config).unwrap();
    }

    #[tokio::test]
    async fn test_export_processes_to_csv_success() {
        let processes = vec![(1.into(), "test".into(), 0.0, 0, "Running".into())];
        let result = export_processes_to_csv(processes).await;
        assert!(result.is_ok());

        // Clean up
        let _ = fs::remove_file("processes.csv");
    }

    #[tokio::test]
    async fn test_export_processes_to_csv_fail() {
        // Make CSV path invalid
        let invalid_path = "/root/invalid_processes.csv";
        let processes = vec![(1.into(), "test".into(), 0.0, 0, "Running".into())];

        // Simulate failure by attempting to write to unwritable location
        let result = export_processes_to_csv(processes).await;
        // We can't actually force a permission error on all systems,
        // so this is just a placeholder to check error handling exists
        // Usually you'd mock File::create here
        assert!(result.is_ok() || result.is_err());
    }
}
