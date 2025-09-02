/// ZMQ message format: frame 0 is topic, frame 1 is payload.
/// We expect at least 2 frames: [topic, payload].
pub const ZMQ_MESSAGE_MIN_FRAMES: usize = 2;
pub const ZMQ_PAYLOAD_FRAME_INDEX: usize = 1;
pub const ZMQ_TOPIC_FRAME_INDEX: usize = 0;
