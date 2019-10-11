//! DTCP
use crate::constants::{SequenceNumber, Instant};
use crate::packet::Packet;
use std::collections::VecDeque;

pub struct Dtcp {
    /// Next PDU should have DRF set.
    set_drf_flag: bool,
    /// Indicates if acks are sent immediately or after an A-Timer expires.
    immediate: bool,
    /// Estimated round trip time.
    rtt: u64,
    /// Retransmission control.
    retx: Option<RetransmissionControl>,
    /// Window flow control.
    window: Option<WindowFlowControl>,
    /// Rate flow control.
    rate: Option<RateFlowControl>,
}

impl Dtcp {
    /// Returns if the next PDU should have the DRF set and resets it to false.
    pub fn take_drf(&mut self) -> bool {
        let drf = self.set_drf_flag;
        self.set_drf_flag = false;
        drf
    }

    /// Window open.
    pub fn window_open(&self, sn: SequenceNumber) -> bool {
        if let Some(window) = self.window.as_ref() {
            if !window.window_open(sn) {
                return false;
            }
        }
        if let Some(rate) = self.rate.as_ref() {
            if !rate.window_open(sn) {
                return false;
            }
        }
        true
    }

    pub fn register_packet(&mut self, packet: &Packet) {
        if let Some(retx) = self.retx.as_mut() {
            retx.register_packet(packet);
        }
        if let Some(window) = self.window.as_mut() {
            window.register_packet(packet);
        }
        if let Some(rate) = self.rate.as_mut() {
            rate.register_packet(packet);
        }
    }
}

pub struct RetransmissionControl {
    /// Queue of sent packets that have not yet been acknowledged.
    retransmission_queue: VecDeque<(Packet, Instant)>,
    /// Maximum number of retransmission attempts.
    max_retransmission_attempts: u32,
    control_sequence_number: u16,
    max_received_control_sequence_number: u16,
}

impl RetransmissionControl {
    /// Registers a packet for potential retransmission.
    pub fn register_packet(&mut self, packet: &Packet) {
        let instant = Instant::now();
        let packet = packet.clone();
        self.retransmission_queue.push_back((packet, instant));
    }
}

pub struct WindowFlowControl {
    /// Largest sequence number that the receiver acknowledged.
    send_window_start: SequenceNumber,
    /// Largest sequence number that the receiver accepts.
    send_window_end: SequenceNumber,
    /// Largest sequence number that we acknowledged.
    receive_window_start: SequenceNumber,
    /// Largest sequence number that we accept.
    receive_window_end: SequenceNumber,
}

impl WindowFlowControl {
    pub fn window_open(&self, sn: SequenceNumber) -> bool {
        sn <= self.send_window_end
    }

    pub fn register_packet(&mut self, _packet: &Packet) {}
}

pub struct RateFlowControl {
    /// Unit of time in milliseconds over which the rate is computed.
    time_unit: u32,
    /// Number of PDUs that can be sent per time unit.
    sending_rate: u32,
    /// Number of sent PDUs in the current time slice.
    pdus_sent_in_time_unit: u32,
}

impl RateFlowControl {
    pub fn window_open(&self, _sn: SequenceNumber) -> bool {
        self.pdus_sent_in_time_unit < self.sending_rate
    }

    pub fn register_packet(&mut self, _packet: &Packet) {
        self.pdus_sent_in_time_unit += 1;
    }
}
