//! Server banner and styling utilities for beautiful console output.
//!
//! This module provides ASCII art banners, boxed configuration displays,
//! progress indicators, and consistent styling for server startup messages.

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::io::IsTerminal;
use std::time::Duration;

/// ASCII art logo for moshi-server
const LOGO: &str = r#"
  __  __  ___  _____ _   _ ___
 |  \/  |/ _ \/ ____| | | |_ _|
 | |\/| | | | \___ \| |_| || |
 | |  | | |_| |___) |  _  || |
 |_|  |_|\___/|____/|_| |_|___|
"#;

/// Box drawing characters for consistent styling
pub mod chars {
    pub const TOP_LEFT: char = '┌';
    pub const TOP_RIGHT: char = '┐';
    pub const BOTTOM_LEFT: char = '└';
    pub const BOTTOM_RIGHT: char = '┘';
    pub const HORIZONTAL: char = '─';
    pub const VERTICAL: char = '│';
    pub const T_LEFT: char = '├';
    pub const T_RIGHT: char = '┤';

    // Level icons
    pub const INFO: &str = "✓";
    pub const WARN: &str = "⚠";
    pub const ERROR: &str = "✕";
    pub const DEBUG: &str = "●";
    pub const TRACE: &str = "·";
}

/// Check if the terminal supports colored output
pub fn supports_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
}

/// Configuration for the server banner
#[derive(Debug, Clone)]
pub struct BannerConfig {
    pub version: String,
    pub addr: String,
    pub port: u16,
    pub modules: Vec<ModuleInfo>,
    pub auth_enabled: bool,
    pub gpu_name: Option<String>,
    pub gpu_vram_mb: Option<u64>,
    pub batch_size: Option<usize>,
    pub instance_name: String,
}

/// Information about a loaded module
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub module_type: String,
    pub path: String,
}

/// Server banner with styling utilities
pub struct ServerBanner {
    use_color: bool,
    box_width: usize,
}

impl Default for ServerBanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerBanner {
    /// Create a new server banner
    pub fn new() -> Self {
        Self {
            use_color: supports_color(),
            box_width: 60,
        }
    }

    /// Set whether to use colored output
    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    /// Set the box width for bordered content
    pub fn with_box_width(mut self, width: usize) -> Self {
        self.box_width = width;
        self
    }

    /// Print the ASCII art logo
    pub fn print_logo(&self) {
        if self.use_color {
            println!("{}", LOGO.cyan().bold());
        } else {
            println!("{LOGO}");
        }
    }

    /// Print a horizontal divider
    pub fn print_divider(&self) {
        let line: String = std::iter::repeat(chars::HORIZONTAL)
            .take(self.box_width)
            .collect();
        if self.use_color {
            println!("{}", line.dimmed());
        } else {
            println!("{line}");
        }
    }

    /// Print a boxed header with a title
    pub fn print_box_header(&self, title: &str) {
        let padding = (self.box_width - title.len() - 4).max(0);
        let left_pad = padding / 2;
        let right_pad = padding - left_pad;

        let top: String = format!(
            "{}{}{}{}{}",
            chars::TOP_LEFT,
            std::iter::repeat(chars::HORIZONTAL).take(left_pad + 1).collect::<String>(),
            format!(" {} ", title),
            std::iter::repeat(chars::HORIZONTAL).take(right_pad + 1).collect::<String>(),
            chars::TOP_RIGHT,
        );

        if self.use_color {
            println!("{}", top.bright_blue());
        } else {
            println!("{top}");
        }
    }

    /// Print a boxed row with a key-value pair
    pub fn print_box_row(&self, key: &str, value: &str) {
        let inner_width = self.box_width - 2; // Account for │ on each side
        let key_width = 16;
        let value_width = inner_width.saturating_sub(key_width + 3); // +3 for ": "

        let truncated_value = if value.len() > value_width {
            format!("{}…", &value[..value_width.saturating_sub(1)])
        } else {
            value.to_string()
        };

        let content = format!(
            "{:key_width$}: {}",
            key,
            truncated_value,
            key_width = key_width
        );
        let padding = inner_width.saturating_sub(content.chars().count());

        if self.use_color {
            println!(
                "{} {}{}{} {}",
                chars::VERTICAL.bright_blue(),
                key.bright_yellow(),
                ": ".dimmed(),
                truncated_value,
                " ".repeat(padding.saturating_sub(key.len() + 2 - key_width.min(key.len()))),
            );
            // Simplified padding
            let full_line = format!("{}: {}", key, truncated_value);
            let line_padding = inner_width.saturating_sub(full_line.chars().count());
            print!("\x1b[1A\x1b[2K"); // Move up and clear line
            println!(
                "{} {}{}{}{} {}",
                chars::VERTICAL.bright_blue(),
                key.bright_yellow(),
                ": ".dimmed(),
                truncated_value,
                " ".repeat(line_padding),
                chars::VERTICAL.bright_blue(),
            );
        } else {
            let full_line = format!("{}: {}", key, truncated_value);
            let line_padding = inner_width.saturating_sub(full_line.chars().count());
            println!(
                "{} {}{} {}",
                chars::VERTICAL,
                full_line,
                " ".repeat(line_padding),
                chars::VERTICAL,
            );
        }
    }

    /// Print a boxed separator
    pub fn print_box_separator(&self) {
        let inner: String = std::iter::repeat(chars::HORIZONTAL)
            .take(self.box_width - 2)
            .collect();
        let line = format!("{}{}{}", chars::T_LEFT, inner, chars::T_RIGHT);

        if self.use_color {
            println!("{}", line.bright_blue());
        } else {
            println!("{line}");
        }
    }

    /// Print the box footer
    pub fn print_box_footer(&self) {
        let inner: String = std::iter::repeat(chars::HORIZONTAL)
            .take(self.box_width - 2)
            .collect();
        let line = format!("{}{}{}", chars::BOTTOM_LEFT, inner, chars::BOTTOM_RIGHT);

        if self.use_color {
            println!("{}", line.bright_blue());
        } else {
            println!("{line}");
        }
    }

    /// Print the full server banner with configuration summary
    /// Note: Logo is printed separately via print_logo() before this
    pub fn print_banner(&self, config: &BannerConfig) {
        // Print configuration box
        self.print_box_header("Configuration");

        // Server info section
        self.print_kv_line("Instance", &config.instance_name);
        self.print_kv_line("Address", &format!("{}:{}", config.addr, config.port));

        // Auth status
        let auth_status = if config.auth_enabled { "Enabled" } else { "Disabled" };
        self.print_kv_line("Auth", auth_status);

        // GPU info
        if let Some(ref gpu_name) = config.gpu_name {
            self.print_box_separator();
            self.print_kv_line("GPU", gpu_name);
            if let Some(vram) = config.gpu_vram_mb {
                self.print_kv_line("VRAM", &format!("{} MB", vram));
            }
            if let Some(batch_size) = config.batch_size {
                self.print_kv_line("Batch Size", &batch_size.to_string());
            }
        }

        // Modules section
        if !config.modules.is_empty() {
            self.print_box_separator();
            self.print_kv_line("Modules", &config.modules.len().to_string());
            for module in &config.modules {
                self.print_kv_line(&format!("  {}", module.module_type), &module.path);
            }
        }

        self.print_box_footer();
        println!();
    }

    /// Print a key-value line inside a box (simplified version)
    fn print_kv_line(&self, key: &str, value: &str) {
        let inner_width = self.box_width - 4; // Account for "│ " on each side
        let full_line = format!("{}: {}", key, value);
        let padding = inner_width.saturating_sub(full_line.chars().count());

        if self.use_color {
            println!(
                "{} {}{}{}{} {}",
                chars::VERTICAL.bright_blue(),
                key.bright_yellow(),
                ": ".dimmed(),
                value,
                " ".repeat(padding),
                chars::VERTICAL.bright_blue(),
            );
        } else {
            println!(
                "{} {}{} {}",
                chars::VERTICAL,
                full_line,
                " ".repeat(padding),
                chars::VERTICAL,
            );
        }
    }

    /// Print a styled info message
    pub fn info(&self, msg: &str) {
        if self.use_color {
            println!("{} {}", chars::INFO.green(), msg);
        } else {
            println!("{} {}", chars::INFO, msg);
        }
    }

    /// Print a styled warning message
    pub fn warn(&self, msg: &str) {
        if self.use_color {
            println!("{} {}", chars::WARN.yellow(), msg.yellow());
        } else {
            println!("{} {}", chars::WARN, msg);
        }
    }

    /// Print a styled error message
    pub fn error(&self, msg: &str) {
        if self.use_color {
            println!("{} {}", chars::ERROR.red(), msg.red());
        } else {
            println!("{} {}", chars::ERROR, msg);
        }
    }
}

/// Format a duration in a human-readable way
pub fn format_duration(secs: f64) -> String {
    if secs < 0.001 {
        format!("{:.0}µs", secs * 1_000_000.0)
    } else if secs < 1.0 {
        format!("{:.1}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.2}s", secs)
    } else {
        let mins = (secs / 60.0).floor();
        let remaining = secs % 60.0;
        format!("{}m {:.0}s", mins, remaining)
    }
}

/// Format bytes in a human-readable way
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ============================================================================
// Progress Indicators
// ============================================================================

/// Create a spinner progress bar for indeterminate operations (e.g., model loading)
pub fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .expect("Invalid progress style template"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Create a progress bar with percentage for determinate operations
pub fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")
            .expect("Invalid progress style template")
            .progress_chars("█▓▒░"),
    );
    pb.set_message(message.to_string());
    pb
}

/// A scoped spinner that automatically finishes when dropped
pub struct ScopedSpinner {
    pb: ProgressBar,
    success_msg: String,
}

impl ScopedSpinner {
    /// Create a new scoped spinner
    pub fn new(message: &str, success_msg: &str) -> Self {
        Self {
            pb: create_spinner(message),
            success_msg: success_msg.to_string(),
        }
    }

    /// Mark the spinner as successful and finish with a checkmark
    pub fn success(self) {
        self.pb.finish_with_message(format!(
            "{} {}",
            chars::INFO.green(),
            self.success_msg
        ));
    }

    /// Mark the spinner as failed and finish with an X
    pub fn failure(self, error: &str) {
        self.pb.finish_with_message(format!(
            "{} {} - {}",
            chars::ERROR.red(),
            self.success_msg,
            error.red()
        ));
    }
}

impl Drop for ScopedSpinner {
    fn drop(&mut self) {
        // If not explicitly finished, just stop the spinner
        if !self.pb.is_finished() {
            self.pb.finish_and_clear();
        }
    }
}

// ============================================================================
// Table Formatter
// ============================================================================

/// A simple table formatter for multi-row data display
pub struct TableFormatter {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    use_color: bool,
}

impl TableFormatter {
    /// Create a new table with the given headers
    pub fn new(headers: Vec<&str>) -> Self {
        Self {
            headers: headers.into_iter().map(String::from).collect(),
            rows: Vec::new(),
            use_color: supports_color(),
        }
    }

    /// Set whether to use colored output
    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    /// Add a row to the table
    pub fn add_row(&mut self, row: Vec<&str>) {
        self.rows.push(row.into_iter().map(String::from).collect());
    }

    /// Calculate column widths based on content
    fn column_widths(&self) -> Vec<usize> {
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.chars().count()).collect();

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.chars().count());
                }
            }
        }

        widths
    }

    /// Print the table to stdout
    pub fn print(&self) {
        let widths = self.column_widths();
        let total_width: usize = widths.iter().sum::<usize>() + (widths.len() * 3) + 1;

        // Top border
        let top_border: String = format!(
            "{}{}{}",
            chars::TOP_LEFT,
            widths
                .iter()
                .map(|w| std::iter::repeat(chars::HORIZONTAL).take(*w + 2).collect::<String>())
                .collect::<Vec<_>>()
                .join(&chars::HORIZONTAL.to_string()),
            chars::TOP_RIGHT
        );

        if self.use_color {
            println!("{}", top_border.bright_blue());
        } else {
            println!("{}", top_border);
        }

        // Header row
        let header_row: String = format!(
            "{} {} {}",
            chars::VERTICAL,
            self.headers
                .iter()
                .zip(&widths)
                .map(|(h, w)| format!("{:^width$}", h, width = *w))
                .collect::<Vec<_>>()
                .join(&format!(" {} ", chars::VERTICAL)),
            chars::VERTICAL
        );

        if self.use_color {
            println!(
                "{} {} {}",
                chars::VERTICAL.bright_blue(),
                self.headers
                    .iter()
                    .zip(&widths)
                    .map(|(h, w)| format!("{:^width$}", h.bright_yellow(), width = *w))
                    .collect::<Vec<_>>()
                    .join(&format!(" {} ", chars::VERTICAL.bright_blue())),
                chars::VERTICAL.bright_blue()
            );
        } else {
            println!("{}", header_row);
        }

        // Header separator
        let separator: String = format!(
            "{}{}{}",
            chars::T_LEFT,
            widths
                .iter()
                .map(|w| std::iter::repeat(chars::HORIZONTAL).take(*w + 2).collect::<String>())
                .collect::<Vec<_>>()
                .join(&chars::HORIZONTAL.to_string()),
            chars::T_RIGHT
        );

        if self.use_color {
            println!("{}", separator.bright_blue());
        } else {
            println!("{}", separator);
        }

        // Data rows
        for row in &self.rows {
            if self.use_color {
                println!(
                    "{} {} {}",
                    chars::VERTICAL.bright_blue(),
                    row.iter()
                        .zip(&widths)
                        .map(|(cell, w)| format!("{:<width$}", cell, width = *w))
                        .collect::<Vec<_>>()
                        .join(&format!(" {} ", chars::VERTICAL.bright_blue())),
                    chars::VERTICAL.bright_blue()
                );
            } else {
                println!(
                    "{} {} {}",
                    chars::VERTICAL,
                    row.iter()
                        .zip(&widths)
                        .map(|(cell, w)| format!("{:<width$}", cell, width = *w))
                        .collect::<Vec<_>>()
                        .join(&format!(" {} ", chars::VERTICAL)),
                    chars::VERTICAL
                );
            }
        }

        // Bottom border
        let bottom_border: String = format!(
            "{}{}{}",
            chars::BOTTOM_LEFT,
            widths
                .iter()
                .map(|w| std::iter::repeat(chars::HORIZONTAL).take(*w + 2).collect::<String>())
                .collect::<Vec<_>>()
                .join(&chars::HORIZONTAL.to_string()),
            chars::BOTTOM_RIGHT
        );

        if self.use_color {
            println!("{}", bottom_border.bright_blue());
        } else {
            println!("{}", bottom_border);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0.0001), "100µs");
        assert_eq!(format_duration(0.150), "150.0ms");
        assert_eq!(format_duration(5.5), "5.50s");
        assert_eq!(format_duration(125.0), "2m 5s");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1500), "1.5 KB");
        assert_eq!(format_bytes(1_500_000), "1.4 MB");
        assert_eq!(format_bytes(1_500_000_000), "1.40 GB");
    }

    #[test]
    fn test_banner_no_color() {
        let banner = ServerBanner::new().with_color(false);
        // Just ensure it doesn't panic
        banner.print_logo();
        banner.print_divider();
    }
}
