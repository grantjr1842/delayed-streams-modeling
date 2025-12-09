//! Server banner and styling utilities for beautiful console output.
//!
//! This module provides ASCII art banners, boxed configuration displays,
//! and consistent styling for server startup messages.

use owo_colors::OwoColorize;
use std::io::IsTerminal;

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
