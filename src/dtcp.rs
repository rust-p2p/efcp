//! DTCP
#![allow(missing_docs)]
#![allow(unused)]
use crate::constants::{Instant, SequenceNumber};
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
    pub fn in_send_window(&self, sn: SequenceNumber) -> bool {
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

    pub fn in_recv_window(&self, sn: SequenceNumber) -> bool {
        if let Some(window) = self.window.as_ref() {
            if sn < window.receive_window_start {
                return false;
            }
            if sn > window.receive_window_end {
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

/// Handles common processing off control PDUs
pub fn common_recv_control(_packet: &Packet) {
    /*if packet.sequence_number() < last_control_sequence_number_received {
        if packet.acki() {
            // dup acks += 1
        }
        if packet.fci() {
            // dup fc += 1
        }
    } else {
        if packet.sequence_number() > last_control_sequence_number_received + 1 {
            // send control ack
        }
        last_control_sequence_number_received += 1;
    }*/
}

/// Event is invoked by Dtp when there is a change in the state vector that is
/// of interest to the Dtcp.
pub fn state_vector_update(_sn: SequenceNumber) {
    // increase right window edge
    // adjust sending_rate
    // if retransmission
    //   if immediate
    //     update left window edge
    //     send ack/flow control pdu
    //     stop A timers associated with this pdu or earlier ones
    //   else
    //     set A timer for this PDU
    // else
    //   if window
    //     send flow control pdu
}

pub fn ack_nack_flow_control_pdu(packet: Packet) {
    // TODO selective ack/nack
    common_recv_control(&packet);
    // update estimated rtt
    // if ack
    //   remove acked pdus from retransmission queue
    //   including any gaps less than allowable gap
    //   and stop retransmission timers
    //
    //   update left window edge
    // if nack
    //   retransmit required pdus
    // if flow control
    //   update right window edge and sending rate
    //   if closed window queue is not empty and left window edge < right window edge
    //     send pdus and put them in retransmission queue
    //   update last seq num sent
    //   if closed window queue empty and last sequence number sent < right window edge
    //     window closed = false
}

pub fn control_ack_pdu(_packet: Packet) {
    // check consistency of sending window values and adjust as necessary
    // send ack/flow control pdu with left window and right window edge
    // send empty transfer pdu with the last sequence number sent
}

/// Retransmission timer is used to determine when to retransmit PDUs that may
/// have been lost or discarded. The time interval is based on an estimate by
/// the sender of the time to get a positive acknowledgement of the PDU.
/// The timer should be set to 2MPL + A + e. The RTT estimator should provide
/// a good estimate for 2MPL + e.
pub fn retransmission_timer(_packet: &Packet) {
    // if retransmission count >= max retransmission attempts
    //   return error
    // send packet
    // update retransmission count
}

/// Timer used by the rate based flow control mechanism. There is one instance
/// of this timer for each direction of the flow and the rates may be different.
pub fn sending_rate_timer() {
    // sent pdus in time unit = 0
    // rate fulfilled = false
    // reset timer with sending_rate
}

/// Timer used to detect inactivity when the window is open and there is no
/// traffic. The timer should be set to 1 / data_rate + 2sd.
pub fn window_timer() {
    // send control ack pdu
    // reset timer
    // TODO: timer must be reset when receiving a dtp-pdu
}
