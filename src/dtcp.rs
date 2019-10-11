//! DTCP
use crate::constants::{SequenceNumber, Instant};
use crate::packet::Packet;
use std::collections::VecDeque;

pub struct Dtcp {
    /// Next PDU should have DRF set.
    set_drf_flag: bool,
    /// Indicates if acks are sent immediately or after an A-Timer expires.
    immediate: bool,
    ///
    sender_right_window_edge: SequenceNumber,
    ///
    receiver_right_window_edge_sent: SequenceNumber,
    ///
    rtt: u64,
    /// Queue of sent packets that have not yet been acknowledged.
    retransmission_queue: VecDeque<(Packet, Instant)>,
}

impl Dtcp {
    pub fn take_drf(&mut self) -> bool {
        let drf = self.set_drf_flag;
        self.set_drf_flag = false;
        drf
    }

    pub fn register_retransmission(&mut self, packet: &Packet) {
        let instant = Instant::now();
        let packet = packet.clone();
        self.retransmission_queue.push_back((packet, instant));
    }
}
