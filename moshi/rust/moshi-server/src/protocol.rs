// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use anyhow::Result;
use axum::extract::ws;

// ============================================================================
// WebSocket Close Codes (RFC 6455 + Custom Application Codes)
// ============================================================================
//
// Standard codes (1000-1015) are defined by RFC 6455.
// Custom application codes must be in the range 4000-4999.
//
// See: https://www.rfc-editor.org/rfc/rfc6455.html#section-7.4.1

/// Custom WebSocket close codes for moshi-server.
/// These codes are in the 4000-4999 range reserved for application use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CloseCode {
    /// Normal closure (RFC 6455)
    Normal = 1000,
    /// Server is going away (RFC 6455)
    GoingAway = 1001,
    /// Protocol error (RFC 6455)
    ProtocolError = 1002,
    /// Internal server error (RFC 6455)
    InternalError = 1011,

    // Custom application codes (4000-4999)
    /// Server at capacity - no free channels available
    ServerAtCapacity = 4000,
    /// Authentication failed - invalid or missing credentials
    AuthenticationFailed = 4001,
    /// Session timeout - connection exceeded maximum duration
    SessionTimeout = 4002,
    /// Invalid message format - failed to parse client message
    InvalidMessage = 4003,
    /// Rate limited - too many requests
    RateLimited = 4004,
    /// Resource unavailable - requested resource not found
    ResourceUnavailable = 4005,
    /// Client timeout - no data received within expected timeframe
    ClientTimeout = 4006,
}

impl CloseCode {
    /// Returns the numeric code value
    pub fn code(&self) -> u16 {
        *self as u16
    }

    /// Returns a human-readable description of the close code
    pub fn reason(&self) -> &'static str {
        match self {
            CloseCode::Normal => "Normal closure",
            CloseCode::GoingAway => "Server going away",
            CloseCode::ProtocolError => "Protocol error",
            CloseCode::InternalError => "Internal server error",
            CloseCode::ServerAtCapacity => "Server at capacity",
            CloseCode::AuthenticationFailed => "Authentication failed",
            CloseCode::SessionTimeout => "Session timeout",
            CloseCode::InvalidMessage => "Invalid message format",
            CloseCode::RateLimited => "Rate limited",
            CloseCode::ResourceUnavailable => "Resource unavailable",
            CloseCode::ClientTimeout => "Client timeout",
        }
    }

    /// Returns true if this is a retryable error (client should reconnect)
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CloseCode::ServerAtCapacity
                | CloseCode::GoingAway
                | CloseCode::InternalError
                | CloseCode::RateLimited
        )
    }

    /// Creates a WebSocket CloseFrame with this code and reason
    pub fn to_close_frame(&self) -> ws::CloseFrame {
        ws::CloseFrame {
            code: self.code(),
            reason: self.reason().into(),
        }
    }

    /// Creates a WebSocket CloseFrame with a custom reason message
    pub fn with_reason(&self, reason: impl Into<String>) -> ws::CloseFrame {
        ws::CloseFrame {
            code: self.code(),
            reason: reason.into().into(),
        }
    }
}

impl From<CloseCode> for ws::CloseFrame {
    fn from(code: CloseCode) -> Self {
        code.to_close_frame()
    }
}

// ============================================================================
// Message Types
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub enum MsgType {
    Handshake,
    Audio,
    Text,
    Control,
    Metadata,
    Error,
    Ping,
    ColoredText,
    Image,
    Codes,
}

impl MsgType {
    pub fn from_u8(v: u8) -> Result<Self> {
        let s = match v {
            0 => MsgType::Handshake,
            1 => MsgType::Audio,
            2 => MsgType::Text,
            3 => MsgType::Control,
            4 => MsgType::Metadata,
            5 => MsgType::Error,
            6 => MsgType::Ping,
            7 => MsgType::ColoredText,
            8 => MsgType::Image,
            9 => MsgType::Codes,
            _ => anyhow::bail!("unexpected msg type {v}"),
        };
        Ok(s)
    }

    pub fn to_u8(self) -> u8 {
        match self {
            MsgType::Handshake => 0,
            MsgType::Audio => 1,
            MsgType::Text => 2,
            MsgType::Control => 3,
            MsgType::Metadata => 4,
            MsgType::Error => 5,
            MsgType::Ping => 6,
            MsgType::ColoredText => 7,
            MsgType::Image => 8,
            MsgType::Codes => 9,
        }
    }
}
