//! Data Transfer Protocol
use crate::constants::SequenceNumber;
use crate::dtcp::Dtcp;
use crate::packet::{Packet, PacketError};
use bytes::BytesMut;
use failure::Fail;
use std::collections::VecDeque;

/// Error sending a packet.
#[derive(Debug, Fail)]
pub enum SendError {
    /// Max closed window queue length exceeded is used to notify an upper
    /// layer that it should throttle it's sending rate.
    #[fail(display = "max closed window queue length exceeded")]
    MaxClosedWindowQueue,
}

/// Error receiving a packet.
#[derive(Debug, Fail)]
pub enum RecvError {
    /// Packet is invalid.
    #[fail(display = "invalid packet")]
    Packet(PacketError),
}

impl From<PacketError> for RecvError {
    fn from(e: PacketError) -> Self {
        Self::Packet(e)
    }
}

/// Dtp state machine.
pub struct Dtp {
    /// DTCP handler.
    dtcp: Option<Dtcp>,
    /// Maximum number of PDUs queued to send because the flow control window
    /// is closed.
    max_closed_window_queue_len: usize,
    /// Queue of PDUs ready to be sent once the window opens.
    closed_window_queue: VecDeque<Packet>,
    /// Largest sequence number received.
    max_sequence_number_received: SequenceNumber,
    /// Next sequence number.
    sequence_number: SequenceNumber,
    /// Queue of PDUs requiring reassembly.
    reassembly_queue: VecDeque<Packet>,
}

impl Dtp {
    /// Send a packet.
    pub fn send_packet(&mut self, payload: &[u8]) -> Result<(), SendError> {
        // stop sender inactivity timer

        // create packet
        let mut packet = Packet::dtp(payload);
        // sequence number
        packet.set_sequence_number(self.sequence_number);
        self.sequence_number += 1;

        if let Some(dtcp) = self.dtcp.as_mut() {
            // set data run flag
            packet.set_drf(dtcp.take_drf());
            if dtcp.in_send_window(packet.sequence_number()) {
                dtcp.register_packet(&packet)
            // TODO send packet
            } else {
                self.closed_window_queue.push_back(packet);
            }
        } else {
            // TODO send packet
        }

        // start sender inactivity timer

        // notify the caller about error conditions.
        if self.closed_window_queue.len() > self.max_closed_window_queue_len {
            return Err(SendError::MaxClosedWindowQueue);
        }

        Ok(())
    }

    /// Receive a packet.
    pub fn recv_packet(&mut self, bytes: BytesMut) -> Result<(), RecvError> {
        // stop receiver inactivity timer
        let packet = Packet::parse(bytes)?;
        // if flow control present reset window timer

        if packet.drf() {
            // first pdu or new run

            // TODO flush reassembly queue
            self.max_sequence_number_received = packet.sequence_number();
        // set_drf_flag to true to initialize other direction
        // update next sequence number to send via policy
        // notify dtcp of the received sequence number
        } else {
            if let Some(dtcp) = self.dtcp.as_mut() {
                if !dtcp.in_recv_window(packet.sequence_number()) {
                    // increment counter of dropped packets
                    // send ack/flow control pdu with current window values
                    return Ok(());
                }
            }
            if packet.sequence_number() <= self.max_sequence_number_received {
                // packet is a gap or a duplicate

                //if is duplicate {
                // TODO:
                // increment counter of dropped duplicates
                // send ack/flow control pdu with current window values
                //} else {
                // insert into reasembly queue
                // notify dtcp of received sequence number
                //}
            } else {
                // NOTE: max_sequence_number_received == receive_window_start
                self.max_sequence_number_received = packet.sequence_number();
                self.reassembly_queue.push_back(packet);
                // notify dtcp of the received sequence number
                // start A timer
            }
        }

        // start receiver inactivity timer
        Ok(())
    }
}

/// When the timer expires, it indicates that there is a gap greater than
/// allowed on this connection and retransmissions have not been successful.
///
/// Receiver inactivity timer should be set to 3(MPL + R + A).
/// Sender inactivity timer should be set to 2(MPL + R + A).
pub fn inactivity_timer(_sn: SequenceNumber) {
    // set_drf_flag = true
    // next sequence number to send via policy
    // discard pdus from retransmission queue
    // discard pdus from closed window queue
    // send control ack pdu
    // send transfer pdu with zero length
    // notify user flow there has been no activity for a while
}

/// A timer for incoming transfer PDUs.
///
/// Timer should be set to A - RTT/2 where RTT is the estimated round trip
/// time.
pub fn a_timer(_sn: SequenceNumber) {
    // update left window edge
    // invoke delimiting
    // if dtcp
    //     send an ack/flow control PDU
    // else
    //     reset sender inactivity timer
}
