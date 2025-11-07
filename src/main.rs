use iced::{
    executor, time, Alignment, Application, Command, Element, Length, Background,
    Settings, Subscription, Theme,
};
use iced::widget::{button, column, container, row, text, text_input, horizontal_space, vertical_space};
use iced::widget::container::Appearance;
use iced::widget::scrollable;
use iced::{Color, Border};
use sysinfo::{System, Disks, Pid};
use log::info;
use std::fs::File;
use std::io::Write;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

fn main() -> iced::Result {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    LightMon::run(Settings::default())
}

// Our app settings - gets saved to a config file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    refresh_interval: u64,  // How often to update stats (seconds)
    dark_mode: bool,        // Light or dark theme
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            refresh_interval: 1,  // Update every second by default
            dark_mode: false,     // Start with light mode
        }
    }
}

// Handle loading/saving settings to file
fn get_config_path() -> PathBuf {
    PathBuf::from("lightmon_config.toml")  // Config file in same directory
}

fn load_config() -> AppConfig {
    let config_path = get_config_path();
    if let Ok(config_str) = fs::read_to_string(&config_path) {
        if let Ok(config) = toml::from_str(&config_str) {
            return config;  // Return loaded config if file exists and is valid
        }
    }
    AppConfig::default()  // Otherwise use defaults
}

fn save_config(config: &AppConfig) -> Result<(), String> {
    let config_path = get_config_path();
    let config_str = toml::to_string(config).map_err(|e| e.to_string())?;
    fs::write(&config_path, config_str).map_err(|e| e.to_string())?;
    Ok(())
}

// Main app state - holds everything we need to track
struct LightMon {
    sys: System,           // System info library instance
    cpu_usage: f32,        // Current CPU usage percentage
    memory_used: u64,      // Memory being used right now
    memory_total: u64,     // Total available memory
    disk_used: u64,        // Disk space used (simplified)
    disk_total: u64,       // Total disk space (simplified)
    current_screen: Screen, // Which tab we're on
    dark_mode: bool,       // Current theme
    sort_by: SortBy,       // How to sort processes
    filter_text: String,   // Search filter for processes
    selected: Option<Pid>, // Currently selected process
    refresh_interval: u64, // How often to update (seconds)
    refresh_interval_input: String, // User input for refresh rate
    toast_message: Option<String>, // Popup messages
    is_exporting: bool,    // Whether we're exporting CSV
}

// Different tabs in our app
#[derive(Debug, Clone)]
enum Screen {
    Overview,   // System stats overview
    Processes,  // Process list and management  
    Settings,   // App settings
}

// How to sort the process list
#[derive(Debug, Clone, Copy)]
enum SortBy {
    Cpu,    // Sort by CPU usage
    Memory, // Sort by memory usage
}

// All the different things that can happen in our app
#[derive(Debug, Clone)]
enum Message {
    Tick,  // Timer tick - update system info
    SystemData(f32, u64, u64, u64, u64), // New system data received
    GoToOverview,    // Switch to overview tab
    GoToProcesses,   // Switch to processes tab  
    GoToSettings,    // Switch to settings tab
    ToggleTheme,     // Switch between light/dark mode
    SortByCpu,       // Sort processes by CPU
    SortByMemory,    // Sort processes by memory
    FilterChanged(String), // User typed in search box
    SelectProcess(Pid),    // User clicked a process
    SetRefreshInterval(String), // User changed refresh rate
    ExportProcesses,       // Export process list to CSV
    ExportComplete(Result<(), String>), // CSV export finished
    ClearToast,      // Clear popup message
    KillProcess,     // Kill the selected process
}

// Make our app work with the Iced framework
impl Application for LightMon {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let mut sys = System::new_all();
        sys.refresh_all();  // Load initial system data
        
        let config = load_config();  // Load saved settings
        
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
                refresh_interval: config.refresh_interval,
                refresh_interval_input: config.refresh_interval.to_string(),
                toast_message: None,
                is_exporting: false,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("System Monitor")
    }

    // Handle all the different messages/events
    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Tick => {
                // Get fresh system data in the background
                return Command::perform(fetch_system_data(), |(cpu, used, total, disk_used, disk_total)| {
                    Message::SystemData(cpu, used, total, disk_used, disk_total)
                });
            }
            Message::SystemData(cpu, used, total, disk_used, disk_total) => {
                // Update our display with new system stats
                self.cpu_usage = cpu;
                self.memory_used = used;
                self.memory_total = total;
                self.disk_used = disk_used;
                self.disk_total = disk_total;
                info!("CPU: {:.1}%, Memory: {}/{} MB", cpu, used / 1024, total / 1024);
            }
            Message::GoToOverview => self.current_screen = Screen::Overview,
            Message::GoToProcesses => {
                self.current_screen = Screen::Processes;
                self.sys.refresh_all();  // Refresh process list when switching to this tab
            }
            Message::GoToSettings => self.current_screen = Screen::Settings,
            Message::ToggleTheme => {
                self.dark_mode = !self.dark_mode;
                // Auto-save the theme preference
                let config = AppConfig {
                    refresh_interval: self.refresh_interval,
                    dark_mode: self.dark_mode,
                };
                if let Err(e) = save_config(&config) {
                    self.toast_message = Some(format!("Couldn't save settings: {}", e));
                }
            }
            Message::SortByCpu => self.sort_by = SortBy::Cpu,
            Message::SortByMemory => self.sort_by = SortBy::Memory,
            Message::FilterChanged(s) => self.filter_text = s,
            Message::SelectProcess(pid) => self.selected = Some(pid),
            Message::SetRefreshInterval(s) => {
                // Let user type freely in the input field
                self.refresh_interval_input = s.clone();

                // Only update the actual refresh rate if it's a valid number
                if let Ok(interval) = s.parse::<u64>() {
                    self.refresh_interval = interval.max(1);  // Minimum 1 second

                    // Save the new setting
                    let config = AppConfig {
                        refresh_interval: self.refresh_interval,
                        dark_mode: self.dark_mode,
                    };
                    let _ = save_config(&config);
                }
            }
            Message::ExportProcesses => {
                self.is_exporting = true;  // Show "Exporting..." on button
                let processes_data = self.get_processes_data();
                return Command::perform(export_processes_to_csv(processes_data), Message::ExportComplete);
            }
            Message::ExportComplete(result) => {
                self.is_exporting = false;
                match result {
                    Ok(()) => {
                        self.toast_message = Some("✅ Processes exported to processes.csv".into());
                    }
                    Err(e) => {
                        self.toast_message = Some(format!("❌ Export failed: {}", e));
                    }
                }
                // Auto-clear the toast message after 3 seconds
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
            Message::KillProcess => {
                if let Some(pid) = self.selected {
                    match self.kill_process(pid) {
                        Ok(()) => {
                            self.toast_message = Some(format!("✅ Killed process {}", pid));
                            self.selected = None;  // Clear selection after killing
                            self.sys.refresh_all();  // Refresh the process list
                        }
                        Err(e) => {
                            self.toast_message = Some(format!("❌ Failed to kill process {}: {}", pid, e));
                        }
                    }
                } else {
                    self.toast_message = Some("⚠️ No process selected".into());
                }
            }
        }
        Command::none()
    }

    // Build the UI based on current state
    fn view(&self) -> Element<Message> {
        // Top navigation bar
        let header = row![
            text("System Monitor").size(20),
            button("Overview").on_press(Message::GoToOverview).padding(5),
            button("Processes").on_press(Message::GoToProcesses).padding(5),
            horizontal_space(),
            button("Settings").on_press(Message::GoToSettings).padding(8),
        ]
        .spacing(15)
        .align_items(Alignment::Center)
        .padding(12);

        // Main content area - changes based on current tab
        let content: Element<_> = match self.current_screen {
            Screen::Overview => self.view_overview(),
            Screen::Processes => self.view_processes(),
            Screen::Settings => self.view_settings(),
        };

        let mut main = column![header, content];

        // Show toast messages if we have any (success/error popups)
        if let Some(toast_msg) = &self.toast_message {
            let is_error = toast_msg.contains('❌');
            let toast_color = if is_error { 
                // Red for errors
                if self.dark_mode { Color::from_rgb(1.0, 0.5, 0.5) } else { Color::from_rgb(0.8, 0.0, 0.0) }
            } else { 
                // Green for success
                if self.dark_mode { Color::from_rgb(0.5, 1.0, 0.5) } else { Color::from_rgb(0.0, 0.6, 0.0) }
            };
            
            let toast_bg = if self.dark_mode { 
                Color::from_rgb(0.2, 0.2, 0.2)  // Dark gray background
            } else { 
                Color::from_rgb(0.98, 0.98, 0.98)  // Light gray background
            };
            let toast_border = if self.dark_mode { 
                Color::from_rgb(0.4, 0.4, 0.4)  // Medium gray border
            } else { 
                Color::from_rgb(0.8, 0.8, 0.8)  // Light gray border
            };

            let toast = container(text(toast_msg).size(14).style(toast_color))
                .padding(10)
                .style(move |_theme: &Theme| Appearance {
                    text_color: None,
                    background: Some(Background::Color(toast_bg)),
                    border: Border { 
                        color: toast_border, 
                        width: 1.0, 
                        radius: 4.0.into() 
                    },
                    shadow: Default::default(),
                });

            main = main.push(toast);
        }

        container(main)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into()
    }

    // Set up periodic updates
    fn subscription(&self) -> Subscription<Message> {
        time::every(std::time::Duration::from_secs(self.refresh_interval))
            .map(|_| Message::Tick)
    }

    // Return current theme
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

    // CPU
    let cpu = sys.cpus().first().map(|c| c.cpu_usage()).unwrap_or(0.0);

    // Memory (KB)
    let used_mem_kb = sys.used_memory();
    let total_mem_kb = sys.total_memory();

    // Disks (new API: separate Disks struct)
    let mut disks = Disks::new_with_refreshed_list();
    disks.refresh();

    let mut total_disk_bytes: u64 = 0;
    let mut used_disk_bytes: u64 = 0;

    for disk in disks.list() {
        let t = disk.total_space();
        let a = disk.available_space();
        total_disk_bytes = total_disk_bytes.saturating_add(t);
        used_disk_bytes = used_disk_bytes.saturating_add(t.saturating_sub(a));
    }

    (cpu, used_mem_kb, total_mem_kb, used_disk_bytes, total_disk_bytes)
}



impl LightMon {
    // Kill a process using Windows taskkill command
    fn kill_process(&self, pid: Pid) -> Result<(), String> {
        let output = ProcessCommand::new("taskkill")
            .args(&["/PID", &pid.to_string(), "/F"])  // /F = force kill
            .output()
            .map_err(|e| format!("Failed to run taskkill: {}", e))?;
        
        if output.status.success() {
            info!("Successfully killed process {}", pid);
            Ok(())
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            Err(format!("Windows taskkill failed: {}", error_msg))
        }
    }

    // Get process data for CSV export
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

    // Overview tab - shows system resource usage
    fn view_overview(&self) -> Element<Message> {
        // Calculate percentages and convert units
        // convert memory KB -> GB
        let mem_total_gb = (self.memory_total as f64) / 1024.0 / 1024.0;
        let mem_used_gb  = (self.memory_used as f64)  / 1024.0 / 1024.0;
        let mem_percent = if mem_total_gb > 0.0 {
            (mem_used_gb / mem_total_gb * 100.0).min(100.0)
        } else { 0.0 };

        // convert disk bytes -> GB
        let disk_total_gb = (self.disk_total as f64) / 1024.0 / 1024.0 / 1024.0;
        let disk_used_gb  = (self.disk_used as f64)  / 1024.0 / 1024.0 / 1024.0;
        let disk_percent = if disk_total_gb > 0.0 {
            (disk_used_gb / disk_total_gb * 100.0).min(100.0)
        } else { 0.0 };


        // Create visual progress bars
        let cpu_filled = (self.cpu_usage as usize / 5).min(20);
        let mem_filled = (mem_percent as usize / 5).min(20);
        let disk_filled = (disk_percent as usize / 5).min(20);

        let cpu_bar = format!("[{}{}]", "█".repeat(cpu_filled), "░".repeat(20 - cpu_filled));
        let mem_bar = format!("[{}{}]", "█".repeat(mem_filled), "░".repeat(20 - mem_filled));
        let disk_bar = format!("[{}{}]", "█".repeat(disk_filled), "░".repeat(20 - disk_filled));

        // Helper to create consistent stat boxes
        let stat_box = |label: &str, bar: String, percent: f32| {
            let bg = if self.dark_mode { 
                Color::from_rgb(0.12, 0.12, 0.12)  // Very dark gray
            } else { 
                Color::from_rgb(0.95, 0.95, 0.95)  // Very light gray
            };
            let text_color = if self.dark_mode { 
                Color::from_rgb(0.94, 0.94, 0.94)  // Almost white
            } else { 
                Color::from_rgb(0.06, 0.06, 0.06)  // Almost black
            };
            let border_color = if self.dark_mode { 
                Color::from_rgb(0.25, 0.25, 0.25)  // Medium dark gray
            } else { 
                Color::from_rgb(0.2, 0.2, 0.2)     // Medium light gray
            };

            container(
                column![
                    text(label).size(16),
                    row![
                        text(bar).size(16),  // The progress bar
                        text(format!("{:.1}%", percent)).width(Length::Fixed(70.0)).size(16),  // Percentage
                    ].spacing(12).align_items(Alignment::Center),
                ].spacing(6)
            )
            .padding(14)
            .style(move |_theme: &Theme| Appearance {
                text_color: Some(text_color),
                background: Some(Background::Color(bg)),
                border: Border { 
                    color: border_color, 
                    width: 1.0, 
                    radius: 4.0.into() 
                },
                shadow: Default::default(),
            })
        };

        column![
            text("System Overview").size(28),
            vertical_space().height(Length::Fixed(10.0)),
            stat_box("CPU", cpu_bar, self.cpu_usage),
            stat_box("Memory", mem_bar, mem_percent as f32),
            stat_box("Disk", disk_bar, disk_percent as f32),
            vertical_space().height(Length::Fixed(15.0)),
            text(format!("Memory: {:.1} / {:.1} GB", mem_used_gb, mem_total_gb))
                .size(14),
        ]
        .spacing(8)
        .padding(25)
        .align_items(Alignment::Start)
        .into()
    }

    // Processes tab - list and manage running processes
    fn view_processes(&self) -> Element<Message> {
        let mut content_column = column![
            text("Running Processes").size(28),
            vertical_space().height(Length::Fixed(10.0)),
            // Action buttons
            row![
                button("Sort by CPU").on_press(Message::SortByCpu).padding(6),
                button("Sort by Memory").on_press(Message::SortByMemory).padding(6),
                if self.is_exporting {
                    button("Exporting...").padding(6)  // Show loading state
                } else {
                    button("Export to CSV").on_press(Message::ExportProcesses).padding(6)
                },
            ].spacing(10),
            vertical_space().height(Length::Fixed(10.0)),
            // Search box
            text_input("Search processes by name or PID", &self.filter_text)
                .on_input(Message::FilterChanged)
                .padding(10)
                .size(15),
            vertical_space().height(Length::Fixed(10.0)),
        ].spacing(6).padding(25);

        // Process list header
        let mut process_list = column![
            row![
                text("PID").width(Length::Fixed(80.0)).size(15),
                text("Name").width(Length::Fill).size(15),
                text("CPU%").width(Length::Fixed(80.0)).size(15),
                text("Memory").width(Length::Fixed(100.0)).size(15),
            ].spacing(12).align_items(Alignment::Center),
        ].spacing(8);

        // Sort processes based on current setting
        let mut processes: Vec<_> = self.sys.processes().iter().collect();
        match self.sort_by {
            SortBy::Cpu => processes.sort_by(|a, b| {
                b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortBy::Memory => processes.sort_by(|a, b| b.1.memory().cmp(&a.1.memory())),
        }

        // Filter processes based on search text
        let filter = self.filter_text.to_lowercase();
        let filtered = processes.into_iter().filter(|(_, p)| {
            let pid_str = format!("{}", p.pid());
            p.name().to_lowercase().contains(&filter) || pid_str.contains(&filter)
        });

        // Display processes (limit to 12 for performance)
        for (pid, process) in filtered.take(12) {
            let row_content = row![
                text(format!("{}", pid)).width(Length::Fixed(80.0)).size(14),
                text(process.name()).width(Length::Fill).size(14),
                text(format!("{:.1}", process.cpu_usage())).width(Length::Fixed(80.0)).size(14),
                text(format!("{} MB", process.memory() / 1024)).width(Length::Fixed(100.0)).size(14),
            ].spacing(12).align_items(Alignment::Center);

            // Make each row clickable
            let row_button = button(row_content)
                .on_press(Message::SelectProcess(*pid))
                .padding(4);

            process_list = process_list.push(row_button);
        }

        // Style the process list container
        let list_bg = if self.dark_mode { 
            Color::from_rgb(0.15, 0.15, 0.15) 
        } else { 
            Color::from_rgb(0.95, 0.95, 0.95) 
        };
        let list_border = if self.dark_mode { 
            Color::from_rgb(0.4, 0.4, 0.4) 
        } else { 
            Color::from_rgb(0.2, 0.2, 0.2) 
        };
        
        let process_container = container(process_list)
            .padding(15)
            .style(move |_theme: &Theme| Appearance {
                text_color: None,
                background: Some(Background::Color(list_bg)),
                border: Border { 
                    color: list_border, 
                    width: 1.0, 
                    radius: 4.0.into() 
                },
                shadow: Default::default(),
            });

        content_column = content_column.push(process_container);

        // Show detailed view when a process is selected
        if let Some(pid) = self.selected {
            if let Some(proc_) = self.sys.process(pid) {
                content_column = content_column.push(vertical_space().height(Length::Fixed(15.0)));
                
                // Extract the colors outside the closure to avoid lifetime issues
                let detail_bg = if self.dark_mode { 
                    Color::from_rgb(0.15, 0.15, 0.15) 
                } else { 
                    Color::from_rgb(0.95, 0.95, 0.95) 
                };
                let detail_border = if self.dark_mode { 
                    Color::from_rgb(0.4, 0.4, 0.4) 
                } else { 
                    Color::from_rgb(0.2, 0.2, 0.2) 
                };
                
                content_column = content_column.push(
                    container(
                        column![
                            text("Selected Process Details").size(18),
                            vertical_space().height(Length::Fixed(10.0)),
                            row![
                                column![
                                    text("Name:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)),
                                    text("PID:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)),
                                    text("Status:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)),
                                    text("Run Time:").size(14).style(Color::from_rgb(0.6, 0.6, 0.6)), 
                                ].spacing(6).width(Length::Fixed(80.0)),
                                column![
                                    text(proc_.name()).size(14),
                                    text(format!("{}", pid)).size(14),
                                    text(format!("{:?}", proc_.status())).size(14),
                                    text(format!("{} seconds", proc_.run_time())).size(14),  
                                ].spacing(6).width(Length::Fill),
                            ].spacing(8),
                            vertical_space().height(Length::Fixed(10.0)),
                            row![
                                column![
                                    text("CPU Usage").size(14),
                                    text(format!("{:.1}%", proc_.cpu_usage())).size(18),
                                ].spacing(4).align_items(Alignment::Center),
                                column![
                                    text("Memory").size(14),
                                    text(format!("{} MB", proc_.memory() / 1024)).size(18),
                                ].spacing(4).align_items(Alignment::Center),
                                column![
                                    text("Virtual Memory").size(14),
                                    text(format!("{} MB", proc_.virtual_memory() / 1024)).size(16),
                                ].spacing(4).align_items(Alignment::Center),
                            ].spacing(30),
                            vertical_space().height(Length::Fixed(15.0)),
                            button("KILL PROCESS")
                                .on_press(Message::KillProcess)
                                .padding(12),
                        ].spacing(12),
                    )
                    .padding(20)
                    .style(move |_theme: &Theme| Appearance {
                        text_color: None,
                        background: Some(Background::Color(detail_bg)),
                        border: Border { 
                            color: detail_border, 
                            width: 1.0, 
                            radius: 8.0.into() 
                        },
                        shadow: Default::default(),
                    }),
                );
            }
        }

        container(scrollable(content_column))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    // Settings tab - adjust app preferences
    fn view_settings(&self) -> Element<Message> {
        let setting_bg = if self.dark_mode { 
            Color::from_rgb(0.15, 0.15, 0.15) 
        } else { 
            Color::from_rgb(0.95, 0.95, 0.95) 
        };
        let setting_border = if self.dark_mode { 
            Color::from_rgb(0.4, 0.4, 0.4) 
        } else { 
            Color::from_rgb(0.2, 0.2, 0.2) 
        };
        
        column![
            text("Settings").size(28),
            vertical_space().height(Length::Fixed(15.0)),
            // Refresh rate setting
            container(
                column![
                    text_input("Update frequency (seconds)", &self.refresh_interval_input)
                        .on_input(Message::SetRefreshInterval)
                        .padding(10)
                        .size(14)
                        .width(Length::Fixed(200.0)),
                ].spacing(8)
            )
            .padding(15)
            .style(move |_theme: &Theme| Appearance {
                text_color: None,
                background: Some(Background::Color(setting_bg)),
                border: Border { 
                    color: setting_border, 
                    width: 1.0, 
                    radius: 4.0.into() 
                },
                shadow: Default::default(),
            }),
            vertical_space().height(Length::Fixed(20.0)),
            // Theme setting
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
                    ].spacing(12),
                ].spacing(8)
            )
            .padding(15)
            .style(move |_theme: &Theme| Appearance {
                text_color: None,
                background: Some(Background::Color(setting_bg)),
                border: Border { 
                    color: setting_border, 
                    width: 1.0, 
                    radius: 4.0.into() 
                },
                shadow: Default::default(),
            }),
        ]
        .spacing(15)
        .padding(25)
        .align_items(Alignment::Start)
        .into()
    }
}

// Export process list to CSV file
async fn export_processes_to_csv(processes: Vec<(Pid, String, f32, u64, String)>) -> Result<(), String> {
    // Small delay to show the "Exporting..." state
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Create the CSV file
    let mut file = File::create("processes.csv")
        .map_err(|e| format!("Can't create CSV file: {} - check permissions", e))?;
    
    // Write header row
    writeln!(file, "PID,Name,CPU%,Memory (KB),Status")
        .map_err(|e| format!("Can't write to CSV: {} - disk may be full", e))?;

    // Write each process as a row
    for (pid, name, cpu_usage, memory, status) in processes {
        let line = format!("{},{},{:.1},{},{}", pid, name, cpu_usage, memory, status);
        writeln!(file, "{}", line)
            .map_err(|e| format!("Can't write process data: {} - disk error", e))?;
    }

    // Make sure everything is written to disk
    file.flush()
        .map_err(|e| format!("Can't save CSV file: {} - write failed", e))?;

    Ok(())
}

// Tests to make sure everything works
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_fetch_system_data_works() {
        let result = fetch_system_data().await;
        assert!(result.0 >= 0.0);  // CPU should be 0% or more
        assert!(result.1 <= result.2);  // Used memory should be <= total memory
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
        assert!(config.refresh_interval >= 1);  // Should have sensible value
    }

    #[test]
    fn test_screen_enum_debug() {
        let screen = Screen::Overview;
        format!("{:?}", screen);  // Should not crash
    }

    #[test]
    fn test_sortby_enum_debug() {
        let sort = SortBy::Cpu;
        format!("{:?}", sort);  // Should not crash
    }

    #[test]
    fn test_message_enum_clone() {
        let msg = Message::Tick;
        let _cloned = msg.clone();  // Should be able to clone messages
    }

    #[test]
    fn test_lightmon_get_processes_data() {
        let mon = LightMon::new(()).0;
        let data = mon.get_processes_data();
        assert!(!data.is_empty());  // Should have some processes
    }

    #[test]
    fn test_set_refresh_interval_parsing() {
        let mut mon = LightMon::new(()).0;

        mon.update(Message::SetRefreshInterval("5".to_string()));
        assert_eq!(mon.refresh_interval, 5);  // Should parse valid number

        mon.update(Message::SetRefreshInterval("abc".to_string()));
        assert_eq!(mon.refresh_interval, 5);  // Should keep previous value on invalid input
    }

    #[test]
    fn test_toggle_theme() {
        let mut mon = LightMon::new(()).0;
        let initial = mon.dark_mode;

        mon.update(Message::ToggleTheme);
        assert_ne!(mon.dark_mode, initial);  // Should flip the theme
    }

    #[test]
    fn test_filter_changed() {
        let mut mon = LightMon::new(()).0;
        mon.update(Message::FilterChanged("test".to_string()));
        assert_eq!(mon.filter_text, "test");  // Should update filter text
    }

    #[test]
    fn test_kill_process_function_exists() {
        let mon = LightMon::new(()).0;
        let result = mon.kill_process(Pid::from(99999));  // Invalid PID
        assert!(result.is_err());  // Should fail gracefully
    }

    #[test]
    fn test_config_file_creation() {
        let test_config = AppConfig {
            refresh_interval: 3,
            dark_mode: true,
        };

        let result = save_config(&test_config);
        assert!(result.is_ok());
        assert!(PathBuf::from("lightmon_config.toml").exists());

        // Clean up test file
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
        let processes = vec![(Pid::from(1), "test.exe".into(), 0.0, 1024, "Running".into())];
        let result = export_processes_to_csv(processes).await;
        assert!(result.is_ok());

        // Clean up test file
        let _ = fs::remove_file("processes.csv");
    }
}