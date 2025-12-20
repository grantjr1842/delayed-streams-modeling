//! Custom logging formatters for beautiful console output.
//!
//! This module provides custom `FormatEvent` implementations for `tracing-subscriber`
//! that add level icons, colors, and improved formatting.

use owo_colors::OwoColorize;
use std::fmt;
use std::io::IsTerminal;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::{self, FormatEvent, FormatFields};
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::registry::LookupSpan;

/// Log style configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogStyle {
    /// Compact: minimal output, single line per event
    Compact,
    /// Pretty: colored output with icons (default for terminals)
    #[default]
    Pretty,
    /// Verbose: full details including file/line numbers
    Verbose,
}

impl std::str::FromStr for LogStyle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "compact" => Ok(LogStyle::Compact),
            "pretty" => Ok(LogStyle::Pretty),
            "verbose" => Ok(LogStyle::Verbose),
            _ => Err(format!("Invalid log style '{}'. Expected: compact, pretty, or verbose", s)),
        }
    }
}

/// Level icons for pretty console output
pub mod icons {
    pub const TRACE: &str = "·";
    pub const DEBUG: &str = "●";
    pub const INFO: &str = "✓";
    pub const WARN: &str = "⚠";
    pub const ERROR: &str = "✕";
}

/// Format a log level with its icon
fn format_level_icon(level: Level) -> &'static str {
    match level {
        Level::TRACE => icons::TRACE,
        Level::DEBUG => icons::DEBUG,
        Level::INFO => icons::INFO,
        Level::WARN => icons::WARN,
        Level::ERROR => icons::ERROR,
    }
}

/// Custom event formatter with level icons and improved styling
pub struct PrettyFormatter<T> {
    timer: T,
    use_ansi: bool,
    show_file: bool,
    show_target: bool,
    style: LogStyle,
}

impl<T> PrettyFormatter<T> {
    /// Create a new pretty formatter with the given timer
    pub fn new(timer: T) -> Self {
        Self {
            timer,
            use_ansi: std::io::stdout().is_terminal(),
            show_file: false,
            show_target: true,
            style: LogStyle::Pretty,
        }
    }

    /// Set whether to use ANSI colors
    pub fn with_ansi(mut self, use_ansi: bool) -> Self {
        self.use_ansi = use_ansi;
        self
    }

    /// Set whether to show file/line information
    pub fn with_file(mut self, show_file: bool) -> Self {
        self.show_file = show_file;
        self
    }

    /// Set whether to show the target module
    pub fn with_target(mut self, show_target: bool) -> Self {
        self.show_target = show_target;
        self
    }

    /// Set the log style
    pub fn with_style(mut self, style: LogStyle) -> Self {
        self.style = style;
        // Verbose style implies showing file info
        if style == LogStyle::Verbose {
            self.show_file = true;
        }
        self
    }
}

impl<S, N, T> FormatEvent<S, N> for PrettyFormatter<T>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();
        let level = *meta.level();

        // Format timestamp
        self.timer.format_time(&mut writer)?;
        write!(writer, " ")?;

        // Format level with icon
        let icon = format_level_icon(level);
        let level_str = match level {
            Level::TRACE => "TRACE",
            Level::DEBUG => "DEBUG",
            Level::INFO => "INFO",
            Level::WARN => "WARN",
            Level::ERROR => "ERROR",
        };

        if self.use_ansi {
            match level {
                Level::TRACE => write!(writer, "{}", format!("{} {}", icon, level_str).dimmed())?,
                Level::DEBUG => write!(writer, "{}", format!("{} {}", icon, level_str).blue())?,
                Level::INFO => write!(writer, "{}", format!("{} {}", icon, level_str).green())?,
                Level::WARN => write!(writer, "{}", format!("{} {}", icon, level_str).yellow())?,
                Level::ERROR => {
                    write!(writer, "{}", format!("{} {}", icon, level_str).red().bold())?
                }
            }
        } else {
            write!(writer, "{} {}", icon, level_str)?;
        }

        // Format target (module path)
        if self.show_target && self.style != LogStyle::Compact {
            let target = meta.target();
            if self.use_ansi {
                write!(writer, " {}", target.dimmed())?;
            } else {
                write!(writer, " {}", target)?;
            }
        }

        // Show span context for verbose mode
        if self.style == LogStyle::Verbose {
            if let Some(scope) = ctx.event_scope() {
                for span in scope.from_root() {
                    if self.use_ansi {
                        write!(writer, " {}", format!("{}:", span.name()).cyan())?;
                    } else {
                        write!(writer, " {}:", span.name())?;
                    }
                }
            }
        }

        // File/line info for verbose mode
        if self.show_file {
            if let (Some(file), Some(line)) = (meta.file(), meta.line()) {
                // Shorten the file path
                let short_file = file.rsplit('/').next().unwrap_or(file);
                if self.use_ansi {
                    write!(writer, " {}", format!("{}:{}", short_file, line).dimmed())?;
                } else {
                    write!(writer, " {}:{}", short_file, line)?;
                }
            }
        }

        write!(writer, " ")?;

        // Format the event's fields
        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

/// Compact formatter for minimal output
#[allow(dead_code)]
pub struct CompactFormatter<T> {
    timer: T,
    use_ansi: bool,
}

#[allow(dead_code)]
impl<T> CompactFormatter<T> {
    /// Create a new compact formatter
    pub fn new(timer: T) -> Self {
        Self { timer, use_ansi: std::io::stdout().is_terminal() }
    }

    /// Set whether to use ANSI colors
    pub fn with_ansi(mut self, use_ansi: bool) -> Self {
        self.use_ansi = use_ansi;
        self
    }
}

impl<S, N, T> FormatEvent<S, N> for CompactFormatter<T>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();
        let level = *meta.level();

        // Compact timestamp (time only, no date)
        self.timer.format_time(&mut writer)?;
        write!(writer, " ")?;

        // Just the icon, no level text
        let icon = format_level_icon(level);
        if self.use_ansi {
            match level {
                Level::TRACE => write!(writer, "{}", icon.dimmed())?,
                Level::DEBUG => write!(writer, "{}", icon.blue())?,
                Level::INFO => write!(writer, "{}", icon.green())?,
                Level::WARN => write!(writer, "{}", icon.yellow())?,
                Level::ERROR => write!(writer, "{}", icon.red().bold())?,
            }
        } else {
            write!(writer, "{}", icon)?;
        }

        write!(writer, " ")?;

        // Event fields only
        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_log_style_from_str() {
        assert_eq!(LogStyle::from_str("compact").unwrap(), LogStyle::Compact);
        assert_eq!(LogStyle::from_str("pretty").unwrap(), LogStyle::Pretty);
        assert_eq!(LogStyle::from_str("verbose").unwrap(), LogStyle::Verbose);
        assert_eq!(LogStyle::from_str("COMPACT").unwrap(), LogStyle::Compact);
        assert!(LogStyle::from_str("invalid").is_err());
    }

    #[test]
    fn test_level_icons() {
        assert_eq!(format_level_icon(Level::INFO), icons::INFO);
        assert_eq!(format_level_icon(Level::WARN), icons::WARN);
        assert_eq!(format_level_icon(Level::ERROR), icons::ERROR);
    }
}
