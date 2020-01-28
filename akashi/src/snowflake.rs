//! Unique 64-bit IDs.

use std::fmt;
use std::thread;
use std::time::{Duration, SystemTime};

extern crate serde;
use serde::{Deserialize, Serialize};

use crate::ecs::{Component, Entity};

/// The epoch used when generating [`Snowflakes`](Snowflake), represented in
/// milliseconds since the UNIX epoch.
///
/// This is currently the UNIX timestamp for midnight (UTC) on Dec. 15, 2018.
pub const EPOCH_SECONDS: u64 = 1_544_832_000;

const WORKER_ID_BITS: usize = 5;
const GROUP_ID_BITS: usize = 5;
const SEQUENCE_BITS: usize = 12;

/// The highest possible worker ID a [`Snowflake`] can contain.
pub const MAX_WORKER_ID: u64 = (1 << (WORKER_ID_BITS + 1)) - 1;

/// The highest possible worker ID a [`Snowflake`] can contain.
pub const MAX_GROUP_ID: u64 = (1 << (GROUP_ID_BITS + 1)) - 1;

const SEQUENCE_MASK: u64 = (1 << (SEQUENCE_BITS + 1)) - 1;

const WORKER_ID_SHIFT: usize = SEQUENCE_BITS;
const GROUP_ID_SHIFT: usize = SEQUENCE_BITS + WORKER_ID_BITS;
const TIMESTAMP_SHIFT: usize = SEQUENCE_BITS + WORKER_ID_BITS + GROUP_ID_BITS;

/// This type is used to represent unique IDs across Akashi.
///
/// Snowflake instances encode a timestamp, application-specific
/// "group" and "worker" IDs, as well as a sequence number to disambiguate
/// objects made in the same millisecond.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(from = "u64", into = "u64")]
pub struct Snowflake(u64);

impl Snowflake {
    /// Get the time at which this `Snowflake` was generated.
    pub fn timestamp(&self) -> SystemTime {
        let epoch: SystemTime = SystemTime::UNIX_EPOCH + Duration::from_secs(EPOCH_SECONDS);
        epoch + Duration::from_millis(self.0 >> TIMESTAMP_SHIFT)
    }

    /// Get the sequence number of this `Snowflake`.
    pub fn sequence(&self) -> u64 {
        self.0 & SEQUENCE_MASK
    }

    /// Get the worker ID that generated this `Snowflake`.
    pub fn worker_id(&self) -> u64 {
        (self.0 >> WORKER_ID_SHIFT) & MAX_WORKER_ID
    }

    /// Get the group ID that generated this `Snowflake`.
    pub fn group_id(&self) -> u64 {
        (self.0 >> GROUP_ID_SHIFT) & MAX_WORKER_ID
    }
}

impl From<u64> for Snowflake {
    fn from(val: u64) -> Snowflake {
        Snowflake(val)
    }
}

impl From<Snowflake> for u64 {
    fn from(val: Snowflake) -> u64 {
        val.0
    }
}

impl From<i64> for Snowflake {
    fn from(val: i64) -> Snowflake {
        Snowflake(val as u64)
    }
}

impl From<Snowflake> for i64 {
    fn from(val: Snowflake) -> i64 {
        val.0 as i64
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.0)
    }
}

impl<E: Entity + 'static> Component<E> for Snowflake {}

/// Generates [`Snowflake`] IDs.
#[derive(Debug)]
pub struct SnowflakeGenerator {
    epoch: SystemTime,
    last_timestamp: u64,
    sequence: u64,
    group_id: u64,
    worker_id: u64,
}

impl SnowflakeGenerator {
    /// Creates a new `SnowflakeGenerator`.
    ///
    /// `SnowflakeGenerator` instances that are used concurrently
    /// should be created with different group and/or worker IDs to
    /// ensure that all generated IDs are unique.
    pub fn new(group_id: u64, worker_id: u64) -> SnowflakeGenerator {
        assert!(group_id <= MAX_GROUP_ID, "Invalid group ID");
        assert!(worker_id <= MAX_WORKER_ID, "Invalid worker ID");

        SnowflakeGenerator {
            epoch: SystemTime::UNIX_EPOCH + Duration::from_secs(EPOCH_SECONDS),
            last_timestamp: 0,
            sequence: 0,
            group_id,
            worker_id,
        }
    }

    fn get_current_timestamp(&self) -> u64 {
        match SystemTime::now().duration_since(self.epoch) {
            Ok(dt) => dt.as_millis() as u64,
            Err(_e) => panic!("System clock is set before snowflake epoch?"),
        }
    }

    /// Generates a new [`Snowflake`] ID.
    ///
    /// This might cause the current thread to sleep in the rare event
    /// that the system clock goes backwards.
    pub fn generate(&mut self) -> Snowflake {
        let cur_timestamp = self.get_current_timestamp();
        if self.last_timestamp > cur_timestamp {
            // Time is moving backwards-- sleep until last_timestamp and attempt to generate again
            thread::sleep(Duration::from_millis(self.last_timestamp - cur_timestamp));
            return self.generate();
        }

        if self.last_timestamp == cur_timestamp {
            self.sequence = (self.sequence + 1) & SEQUENCE_MASK;

            if self.sequence == 0 {
                // Sequence overrun
                self.sequence = (1 << (SEQUENCE_BITS + 1)) - 1;
                thread::sleep(Duration::from_millis(1));
                return self.generate();
            }
        } else {
            self.sequence = 0;
        }

        self.last_timestamp = cur_timestamp;

        Snowflake(
            (cur_timestamp << TIMESTAMP_SHIFT)
                | (self.group_id << GROUP_ID_SHIFT)
                | (self.worker_id << WORKER_ID_SHIFT)
                | self.sequence,
        )
    }
}
