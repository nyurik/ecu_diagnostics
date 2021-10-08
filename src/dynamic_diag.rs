//! Dynamic diagnostic session helper
//! 

use std::{borrow::BorrowMut, sync::{Arc, Mutex}};

use crate::{DiagError, DiagServerResult, channel::{IsoTPSettings}, dtc::DTC, hardware::Hardware, kwp2000::{self, Kwp2000DiagnosticServer, Kwp2000ServerOptions, Kwp2000VoidHandler}, uds::{self, UdsDiagnosticServer, UdsServerOptions, UdsVoidHandler}};


/// Dynamic diagnostic session
/// 
/// This is used if a target ECU has an unknown diagnostic protocol.
/// 
/// This also contains some useful wrappers for basic functions such as 
/// reading and clearing error codes.
#[derive(Debug)]
pub struct DynamicDiagSession {
    session: DynamicSessionType
}

#[derive(Debug)]
enum DynamicSessionType {
    Kwp(Kwp2000DiagnosticServer),
    Uds(UdsDiagnosticServer)
}

impl DynamicDiagSession {
    /// Creates a new dynamic session.
    /// This will first try with KWP2000, then if that fails,
    /// will try with UDS. If both server creations fail,
    /// then the last error will be returned.
    /// 
    /// NOTE: In order to test if the ECU supports the protocol,
    /// the ECU will be put into extended diagnostic session briefly to test
    /// if it supports the tested diagnostic protocol.
    #[allow(unused_must_use, unused_assignments)]
    pub fn new_over_iso_tp<C>(
        hw_device: Arc<Mutex<C>>,
        channel_cfg: IsoTPSettings,
        tx_id: u32,
        rx_id: u32,
    ) -> DiagServerResult<Self>
    where
        C: Hardware + 'static 
    {

        let mut last_err : Option<DiagError>; // Setting up last recorded error

        // Create iso tp channel using provided HW interface. If this fails, we cannot setup KWP or UDS session!
        let mut iso_tp_channel = Hardware::create_iso_tp_channel(hw_device.clone())?;

        // Firstly, try KWP2000
        match Kwp2000DiagnosticServer::new_over_iso_tp(Kwp2000ServerOptions { 
            send_id: tx_id, 
            recv_id: rx_id, 
            read_timeout_ms: 1500, 
            write_timeout_ms: 1500, 
            global_tp_id: 0x00, 
            tester_present_interval_ms: 2000, 
            tester_present_require_response: true 
        }, iso_tp_channel, channel_cfg, Kwp2000VoidHandler{}) {
            Ok(mut kwp) => {
                if kwp2000::set_diagnostic_session_mode(&mut kwp, kwp2000::SessionType::ExtendedDiagnostics).is_ok() {
                    // KWP accepted! The ECU supports KWP2000!
                    // Return the ECU back to normal mode
                    kwp2000::set_diagnostic_session_mode(&mut kwp, kwp2000::SessionType::Normal);
                    return Ok(Self {
                        session: DynamicSessionType::Kwp(kwp)
                    })
                } else {
                    last_err = Some(DiagError::NotSupported)
                }
            },
            Err(e) => { last_err = Some(e); }
        }

        iso_tp_channel = Hardware::create_iso_tp_channel(hw_device)?;
        match UdsDiagnosticServer::new_over_iso_tp(UdsServerOptions { 
            send_id: tx_id, 
            recv_id: rx_id, 
            read_timeout_ms: 1500, 
            write_timeout_ms: 1500, 
            global_tp_id: 0x00, 
            tester_present_interval_ms: 2000, 
            tester_present_require_response: true 
        }, iso_tp_channel, channel_cfg, UdsVoidHandler{}) {
            Ok(mut uds) => {
                if uds::set_extended_mode(&mut uds).is_ok() {
                    // KWP accepted! The ECU supports KWP2000!
                    // Return the ECU back to normal mode
                    uds::set_default_mode(&mut uds);
                    return Ok(Self {
                        session: DynamicSessionType::Uds(uds)
                    })
                } else {
                    last_err = Some(DiagError::NotSupported)
                }
            },
            Err(e) => { last_err = Some(e); }
        }
        Err(last_err.unwrap())
    }

    /// Returns a reference to KWP2000 session. None is returned if server type is not KWP2000
    pub fn as_kwp_session(&'_ mut self) -> Option<&'_ mut Kwp2000DiagnosticServer> {
        if let DynamicSessionType::Kwp(kwp) = self.session.borrow_mut() {
            Some(kwp)
        } else {
            None
        }
    }

    /// Returns a reference to UDS session. None is returned if server type is not UDS
    pub fn as_uds_session(&'_ mut self) -> Option<&'_ mut UdsDiagnosticServer> {
        if let DynamicSessionType::Uds(uds) = self.session.borrow_mut() {
            Some(uds)
        } else {
            None
        }
    }

    /// Puts the ECU into an extended diagnostic session
    pub fn enter_extended_diagnostic_mode(&mut self) -> DiagServerResult<()> {
        match self.session.borrow_mut() {
            DynamicSessionType::Kwp(k) => {
                kwp2000::set_diagnostic_session_mode(k, kwp2000::SessionType::ExtendedDiagnostics)
            },
            DynamicSessionType::Uds(u) => {
                uds::set_extended_mode(u)
            },
        }
    }

    /// Puts the ECU into a default diagnostic session. This is how the ECU normally operates
    pub fn enter_default_diagnostic_mode(&mut self) -> DiagServerResult<()> {
        match self.session.borrow_mut() {
            DynamicSessionType::Kwp(k) => {
                kwp2000::set_diagnostic_session_mode(k, kwp2000::SessionType::Normal)
            },
            DynamicSessionType::Uds(u) => {
                uds::set_default_mode(u)
            },
        }
    }

    /// Reads all diagnostic trouble codes from the ECU
    pub fn read_all_dtcs(&mut self) -> DiagServerResult<Vec<DTC>> {
        match self.session.borrow_mut() {
            DynamicSessionType::Kwp(k) => {
                kwp2000::read_stored_dtcs(k, kwp2000::DTCRange::All)
            },
            DynamicSessionType::Uds(u) => {
                uds::get_dtcs_by_status_mask(u, 0xFF)
            },
        }
    }

    /// Attempts to clear all DTCs stored on the ECU
    pub fn clear_all_dtcs(&mut self) -> DiagServerResult<()> {
        match self.session.borrow_mut() {
            DynamicSessionType::Kwp(k) => {
                kwp2000::clear_dtc(k, kwp2000::ClearDTCRange::AllDTCs)
            },
            DynamicSessionType::Uds(u) => {
                uds::clear_diagnostic_information(u, 0x00FFFFFF)
            },
        }
    }
}