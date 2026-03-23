//! Event publishers for different backends.
//!
//! This module provides abstractions for publishing rkBPF events to
//! different destinations (ROS2 topics, stdout, files, etc.).

use crate::event::RkEvent;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Configuration for event publishers.
#[derive(Debug, Clone)]
pub struct PublisherConfig {
    /// Topic name for ROS2 publisher
    pub topic: String,
    /// Maximum publish rate (events per second, 0 = unlimited)
    pub rate_limit: u32,
    /// Buffer size for batching
    pub buffer_size: usize,
    /// Whether to include timestamps in output
    pub include_timestamps: bool,
    /// Output format
    pub format: OutputFormat,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            topic: "/rk/events".to_string(),
            rate_limit: 0,
            buffer_size: 64,
            include_timestamps: true,
            format: OutputFormat::Json,
        }
    }
}

/// Output format for events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// JSON format (human-readable)
    Json,
    /// JSON Lines format (one JSON object per line)
    JsonLines,
    /// Compact binary format
    Binary,
    /// Human-readable text
    Text,
}

/// Trait for event publishers.
pub trait EventPublisher: Send + Sync {
    /// Publish a single event.
    fn publish(&self, event: &RkEvent) -> Result<(), PublishError>;

    /// Publish a batch of events.
    fn publish_batch(&self, events: &[RkEvent]) -> Result<(), PublishError> {
        for event in events {
            self.publish(event)?;
        }
        Ok(())
    }

    /// Flush any buffered events.
    fn flush(&self) -> Result<(), PublishError>;

    /// Get the number of events published.
    fn events_published(&self) -> u64;

    /// Get the number of events dropped (due to rate limiting, errors, etc.).
    fn events_dropped(&self) -> u64;
}

/// Errors that can occur during publishing.
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Rate limit exceeded
    #[error("rate limit exceeded")]
    RateLimited,

    /// Publisher closed
    #[error("publisher closed")]
    Closed,

    /// ROS2 error
    #[error("ROS2 error: {0}")]
    Ros2(String),
}

/// Publisher that writes events to stdout.
pub struct StdoutPublisher {
    config: PublisherConfig,
    events_published: AtomicU64,
    events_dropped: AtomicU64,
    last_publish: std::sync::Mutex<std::time::Instant>,
}

impl StdoutPublisher {
    /// Create a new stdout publisher.
    pub fn new(config: PublisherConfig) -> Self {
        Self {
            config,
            events_published: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
            last_publish: std::sync::Mutex::new(std::time::Instant::now()),
        }
    }

    /// Check rate limiting.
    fn check_rate_limit(&self) -> bool {
        if self.config.rate_limit == 0 {
            return true;
        }

        let mut last = self.last_publish.lock().unwrap();
        let now = std::time::Instant::now();
        let min_interval = Duration::from_secs_f64(1.0 / self.config.rate_limit as f64);

        if now.duration_since(*last) >= min_interval {
            *last = now;
            true
        } else {
            false
        }
    }

    /// Format an event according to the configured format.
    fn format_event(&self, event: &RkEvent) -> Result<String, PublishError> {
        match self.config.format {
            OutputFormat::Json => serde_json::to_string_pretty(event)
                .map_err(|e| PublishError::Serialization(e.to_string())),
            OutputFormat::JsonLines => {
                serde_json::to_string(event).map_err(|e| PublishError::Serialization(e.to_string()))
            }
            OutputFormat::Text => Ok(self.format_text(event)),
            OutputFormat::Binary => Err(PublishError::Serialization(
                "binary format not supported for stdout".to_string(),
            )),
        }
    }

    /// Format an event as human-readable text.
    fn format_text(&self, event: &RkEvent) -> String {
        let ts = if self.config.include_timestamps {
            format!("[{:>16}] ", event.timestamp_ns())
        } else {
            String::new()
        };

        match event {
            RkEvent::Imu(e) => {
                format!(
                    "{}IMU[{}]: accel=({}, {}, {}) gyro=({}, {}, {}) temp={}",
                    ts,
                    e.sensor_id,
                    e.accel_x,
                    e.accel_y,
                    e.accel_z,
                    e.gyro_x,
                    e.gyro_y,
                    e.gyro_z,
                    e.temperature
                )
            }
            RkEvent::Motor(e) => {
                format!(
                    "{}MOTOR[{}]: duty={} period={}ns enabled={}",
                    ts, e.channel, e.duty_cycle, e.period_ns, e.enabled
                )
            }
            RkEvent::Safety(e) => {
                format!(
                    "{}SAFETY[{:?}]: source={} value={} action={:?}",
                    ts, e.safety_type, e.source_id, e.value, e.action
                )
            }
            RkEvent::Gpio(e) => {
                format!(
                    "{}GPIO[chip{}/line{}]: edge={} value={}",
                    ts, e.chip, e.line, e.edge, e.value
                )
            }
            RkEvent::TimeSeries(e) => {
                format!(
                    "{}TIMESERIES[{}]: value={} tag={}",
                    ts, e.series_id, e.value, e.tag
                )
            }
            RkEvent::SchedSwitch(e) => {
                format!(
                    "{}SCHED_SWITCH: cpu={} prev(pid={}, tid={}) -> next(pid={}, tid={})",
                    ts, e.cpu_id, e.prev_pid, e.prev_tid, e.next_pid, e.next_tid
                )
            }
            RkEvent::Trace(e) => {
                format!("{}TRACE: {}", ts, e.message)
            }
            RkEvent::Unknown { event_type, data } => {
                format!("{}UNKNOWN[type={}]: {} bytes", ts, event_type, data.len())
            }
        }
    }
}

impl EventPublisher for StdoutPublisher {
    fn publish(&self, event: &RkEvent) -> Result<(), PublishError> {
        if !self.check_rate_limit() {
            self.events_dropped.fetch_add(1, Ordering::Relaxed);
            return Err(PublishError::RateLimited);
        }

        let formatted = self.format_event(event)?;
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{}", formatted)?;

        self.events_published.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn flush(&self) -> Result<(), PublishError> {
        std::io::stdout().flush()?;
        Ok(())
    }

    fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }

    fn events_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }
}

/// Publisher that writes events to a file.
pub struct FilePublisher {
    config: PublisherConfig,
    file: std::sync::Mutex<std::fs::File>,
    events_published: AtomicU64,
    events_dropped: AtomicU64,
}

impl FilePublisher {
    /// Create a new file publisher.
    pub fn new(config: PublisherConfig, path: &std::path::Path) -> std::io::Result<Self> {
        let file = std::fs::File::create(path)?;
        Ok(Self {
            config,
            file: std::sync::Mutex::new(file),
            events_published: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
        })
    }
}

impl EventPublisher for FilePublisher {
    fn publish(&self, event: &RkEvent) -> Result<(), PublishError> {
        let formatted = match self.config.format {
            OutputFormat::Json | OutputFormat::JsonLines => {
                serde_json::to_string(event).map_err(|e| PublishError::Serialization(e.to_string()))?
            }
            OutputFormat::Text => format!("{:?}", event),
            OutputFormat::Binary => {
                return Err(PublishError::Serialization(
                    "binary format not yet implemented".to_string(),
                ))
            }
        };

        let mut file = self.file.lock().unwrap();
        writeln!(file, "{}", formatted)?;

        self.events_published.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn flush(&self) -> Result<(), PublishError> {
        let mut file = self.file.lock().unwrap();
        file.flush()?;
        Ok(())
    }

    fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }

    fn events_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }
}

/// Placeholder ROS2 publisher.
///
/// In a full implementation, this would use rclrs (ros2_rust) bindings
/// to publish to actual ROS2 topics.
pub struct RosPublisher {
    config: PublisherConfig,
    events_published: AtomicU64,
    events_dropped: AtomicU64,
}

impl RosPublisher {
    /// Create a new ROS2 publisher.
    ///
    /// Note: This is a placeholder. Real implementation requires rclrs.
    pub fn new(config: PublisherConfig) -> Self {
        log::info!("Creating ROS2 publisher for topic: {}", config.topic);

        Self {
            config,
            events_published: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
        }
    }

    /// Get the topic name.
    pub fn topic(&self) -> &str {
        &self.config.topic
    }
}

impl EventPublisher for RosPublisher {
    fn publish(&self, event: &RkEvent) -> Result<(), PublishError> {
        // Placeholder: In real implementation, convert to ROS2 message and publish
        // For now, we just count the events

        #[cfg(feature = "ros2")]
        {
            // rclrs integration would go here
            // self.publisher.publish(ros_msg)?;
            unimplemented!("ROS2 integration requires rclrs feature");
        }

        #[cfg(not(feature = "ros2"))]
        {
            log::debug!(
                "ROS2 publish to {}: event_type={}",
                self.config.topic,
                event.event_type()
            );
        }

        self.events_published.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn flush(&self) -> Result<(), PublishError> {
        // ROS2 typically doesn't need explicit flushing
        Ok(())
    }

    fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }

    fn events_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }
}

/// Multi-publisher that sends events to multiple destinations.
pub struct MultiPublisher {
    publishers: Vec<Box<dyn EventPublisher>>,
}

impl MultiPublisher {
    /// Create a new multi-publisher.
    pub fn new() -> Self {
        Self {
            publishers: Vec::new(),
        }
    }

    /// Add a publisher.
    pub fn add(&mut self, publisher: Box<dyn EventPublisher>) {
        self.publishers.push(publisher);
    }
}

impl Default for MultiPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventPublisher for MultiPublisher {
    fn publish(&self, event: &RkEvent) -> Result<(), PublishError> {
        for publisher in &self.publishers {
            // Continue publishing to other destinations even if one fails
            if let Err(e) = publisher.publish(event) {
                log::warn!("Publisher error: {}", e);
            }
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), PublishError> {
        for publisher in &self.publishers {
            publisher.flush()?;
        }
        Ok(())
    }

    fn events_published(&self) -> u64 {
        self.publishers
            .iter()
            .map(|p| p.events_published())
            .max()
            .unwrap_or(0)
    }

    fn events_dropped(&self) -> u64 {
        self.publishers.iter().map(|p| p.events_dropped()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventHeader, ImuEvent, SchedSwitchEvent};

    fn make_test_event() -> RkEvent {
        RkEvent::Imu(ImuEvent {
            header: EventHeader {
                timestamp_ns: 1000000,
                event_type: 1,
                cpu: 0,
                pid: 100,
                _reserved: 0,
            },
            accel_x: 100,
            accel_y: -200,
            accel_z: 9800,
            gyro_x: 0,
            gyro_y: 0,
            gyro_z: 0,
            temperature: 2500,
            sensor_id: 1,
        })
    }

    #[test]
    fn test_stdout_publisher_format_text() {
        let config = PublisherConfig {
            format: OutputFormat::Text,
            include_timestamps: false,
            ..Default::default()
        };
        let publisher = StdoutPublisher::new(config);
        let event = make_test_event();

        let formatted = publisher.format_event(&event).unwrap();
        assert!(formatted.contains("IMU[1]"));
        assert!(formatted.contains("accel=(100, -200, 9800)"));
    }

    #[test]
    fn test_stdout_publisher_format_json() {
        let config = PublisherConfig {
            format: OutputFormat::JsonLines,
            ..Default::default()
        };
        let publisher = StdoutPublisher::new(config);
        let event = make_test_event();

        let formatted = publisher.format_event(&event).unwrap();
        assert!(formatted.contains("\"accel_x\":100"));
    }

    #[test]
    fn test_publisher_config_default() {
        let config = PublisherConfig::default();
        assert_eq!(config.topic, "/rk/events");
        assert_eq!(config.rate_limit, 0);
    }

    #[test]
    fn test_stdout_publisher_format_sched_switch_text() {
        let config = PublisherConfig {
            format: OutputFormat::Text,
            include_timestamps: false,
            ..Default::default()
        };
        let publisher = StdoutPublisher::new(config);
        let event = RkEvent::SchedSwitch(SchedSwitchEvent {
            cpu_id: 0,
            prev_pid: 2,
            prev_tid: 4,
            next_pid: 3,
            next_tid: 5,
        });

        let formatted = publisher.format_event(&event).unwrap();
        assert!(formatted.contains("SCHED_SWITCH"));
        assert!(formatted.contains("prev(pid=2, tid=4)"));
        assert!(formatted.contains("next(pid=3, tid=5)"));
    }
}
