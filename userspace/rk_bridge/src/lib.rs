//! rkBPF to ROS2 Bridge
//!
//! This crate provides functionality to bridge kernel events from rkBPF
//! ring buffers to ROS2 topics, enabling unified observability of kernel
//! and userspace events in the ROS2 ecosystem.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        User Space                                │
//! │                                                                  │
//! │  ┌─────────────┐    ┌─────────────┐    ┌────────────────────┐  │
//! │  │  rk-bridge  │    │  Ring       │    │   ROS2 Node        │  │
//! │  │  (this)     │───▶│  Buffer     │───▶│   /rk/* topics     │  │
//! │  └──────┬──────┘    │  Consumer   │    └────────────────────┘  │
//! │         │           └──────▲──────┘                             │
//! └─────────┼──────────────────┼────────────────────────────────────┘
//!           │ mmap             │
//! ┌─────────┼──────────────────┼────────────────────────────────────┐
//! │         │           Kernel │                                    │
//! │    ┌────▼────┐       ┌─────┴─────┐                              │
//! │    │ rkBPF   │──────▶│ Ring      │                              │
//! │    │ Program │       │ Buffer    │                              │
//! │    └─────────┘       └───────────┘                              │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```bash
//! # Bridge IMU events to ROS2 topic
//! rk-to-ros --map /sys/fs/bpf/maps/imu_events --topic /rk/imu
//!
//! # Bridge motor events with custom rate limiting
//! rk-to-ros --map /sys/fs/bpf/maps/motor_events --topic /rk/motor --rate-limit 1000
//! ```

pub mod event;
pub mod publisher;
pub mod ringbuf;

pub use event::{EventHeader, ImuEvent, MotorEvent, RkEvent, SafetyEvent, SchedSwitchEvent};
pub use publisher::{EventPublisher, PublisherConfig, RosPublisher, StdoutPublisher};
pub use ringbuf::{RingBufConsumer, RingBufError};

/// Result type for rk_bridge operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in the rk_bridge.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Ring buffer error
    #[error("ring buffer error: {0}")]
    RingBuf(#[from] RingBufError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Event parsing error
    #[error("event parsing error: {0}")]
    Parse(String),

    /// Publisher error
    #[error("publisher error: {0}")]
    Publisher(#[from] publisher::PublishError),

    /// Configuration error
    #[error("configuration error: {0}")]
    Config(String),
}
