use std::thread;
use std::time::{Duration, SystemTime};

pub type Snowflake = u64;

pub const EPOCH_SECONDS: u64 = 1544832000;

const WORKER_ID_BITS: usize = 5;
const GROUP_ID_BITS: usize = 5;
const SEQUENCE_BITS: usize = 12;

pub const MAX_WORKER_ID: u64 = (1 << (WORKER_ID_BITS + 1)) - 1;
pub const MAX_GROUP_ID: u64 = (1 << (GROUP_ID_BITS + 1)) - 1;
const SEQUENCE_MASK: u64 = (1 << (SEQUENCE_BITS + 1)) - 1;

const WORKER_ID_SHIFT: usize = SEQUENCE_BITS;
const GROUP_ID_SHIFT: usize = SEQUENCE_BITS + WORKER_ID_BITS;
const TIMESTAMP_SHIFT: usize = SEQUENCE_BITS + WORKER_ID_BITS + GROUP_ID_BITS;

#[allow(dead_code)]
pub fn snowflake_timestamp(s: Snowflake) -> SystemTime {
    let epoch: SystemTime = SystemTime::UNIX_EPOCH + Duration::from_secs(EPOCH_SECONDS);
    epoch + Duration::from_millis(s >> TIMESTAMP_SHIFT)
}

#[allow(dead_code)]
pub fn snowflake_sequence(s: Snowflake) -> u64 {
    s & SEQUENCE_MASK
}

#[allow(dead_code)]
pub fn snowflake_worker_id(s: Snowflake) -> u64 {
    (s >> WORKER_ID_SHIFT) & MAX_WORKER_ID
}

#[allow(dead_code)]
pub fn snowflake_group_id(s: Snowflake) -> u64 {
    (s >> GROUP_ID_SHIFT) & MAX_GROUP_ID
}

#[derive(Debug)]
pub struct SnowflakeGenerator {
    epoch: SystemTime,
    last_timestamp: u64,
    sequence: u64,
}

impl SnowflakeGenerator {
    pub fn new() -> SnowflakeGenerator {
        SnowflakeGenerator {
            epoch: SystemTime::UNIX_EPOCH + Duration::from_secs(EPOCH_SECONDS),
            last_timestamp: 0,
            sequence: 0,
        }
    }

    fn get_current_timestamp(&self) -> u64 {
        match SystemTime::now().duration_since(self.epoch) {
            Ok(dt) => dt.as_millis() as u64,
            Err(_e) => panic!("System clock is set before snowflake epoch?"),
        }
    }

    pub fn generate(&mut self, group_id: u64, worker_id: u64) -> Snowflake {
        assert!(group_id <= MAX_GROUP_ID, "Invalid group ID");
        assert!(worker_id <= MAX_WORKER_ID, "Invalid worker ID");

        let cur_timestamp = self.get_current_timestamp();
        if self.last_timestamp > cur_timestamp {
            // Time is moving backwards-- sleep until last_timestamp and attempt to generate again
            thread::sleep(Duration::from_millis(self.last_timestamp - cur_timestamp));
            return self.generate(group_id, worker_id);
        }

        if self.last_timestamp == cur_timestamp {
            self.sequence = (self.sequence + 1) & SEQUENCE_MASK;

            if self.sequence == 0 {
                // Sequence overrun
                self.sequence = (1 << (SEQUENCE_BITS + 1)) - 1;
                thread::sleep(Duration::from_millis(1));
                return self.generate(group_id, worker_id);
            }
        } else {
            self.sequence = 0;
        }

        self.last_timestamp = cur_timestamp;

        ((cur_timestamp << TIMESTAMP_SHIFT)
            | (group_id << GROUP_ID_SHIFT)
            | (worker_id << WORKER_ID_SHIFT)
            | self.sequence)
    }
}
