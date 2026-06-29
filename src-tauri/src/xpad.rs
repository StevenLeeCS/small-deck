//! Talks to the XPAD over its WebUSB/vendor interface to discover the device's
//! key layout and which F13–F24 ("Small Deck") triggers are mapped.
//!
//! This is the ONLY device communication the agent does, and it is read-only.
//! At runtime launching is driven entirely by global F13–F24 hotkeys, so the
//! device need not be connected for launches to work — reading it just lets the
//! UI mirror the device's matrix and show the exact keys the user configured.

use serde::Serialize;
use std::time::Duration;

const VID: u16 = 0x1209; // pid.codes test VID — must match USB_VID in firmware
const PID: u16 = 0x0001;
const IFACE: u8 = 1; // ITF_NUM_VENDOR (HID=0, VENDOR=1)
const EP_OUT: u8 = 0x02; // EPNUM_VENDOR_OUT
const EP_IN: u8 = 0x82; // EPNUM_VENDOR_IN

const CMD_READ_LAYOUT: u8 = 0x32; // → [0x32,0x00, rows, cols, matrix[8][10] gpio]
const CMD_READ_KEYS: u8 = 0x33; // → [0x33,0x00, count, {gpio,hid,mod,type}×count]

const LAYOUT_MAX_COLS: usize = 10; // must match firmware flash_config.h

/// HID usage range for F13..F24 — the Small Deck trigger keys.
pub const FKEY_MIN: u8 = 0x68; // F13
pub const FKEY_MAX: u8 = 0x73; // F24

/// Map an F-key HID usage (0x68..0x73) to its label, e.g. 0x68 -> "F13".
pub fn fkey_label(hid: u8) -> String {
    if (FKEY_MIN..=FKEY_MAX).contains(&hid) {
        format!("F{}", 13 + (hid - FKEY_MIN) as u16)
    } else {
        format!("0x{:02X}", hid)
    }
}

fn is_fkey(hid: u8) -> bool {
    (FKEY_MIN..=FKEY_MAX).contains(&hid)
}

// ── One cell of the device matrix ────────────────────────────────────────────
#[derive(Serialize)]
pub struct Cell {
    pub row: u8,             // 0-based row index
    pub col: u8,             // 0-based column index
    pub pos: String,         // human label, e.g. "A1"
    pub occupied: bool,      // a key module is present at this cell
    pub fkey: Option<String>, // "F13".."F24" if this cell is a Small Deck trigger
}

#[derive(Serialize)]
pub struct DeviceMatrix {
    pub rows: u8,
    pub cols: u8,
    pub cells: Vec<Cell>,
}

// ── USB plumbing ─────────────────────────────────────────────────────────────

/// Open the XPAD, claim the vendor interface, run `f`, then always release.
fn with_handle<T>(f: impl FnOnce(&rusb::DeviceHandle<rusb::GlobalContext>) -> Result<T, String>) -> Result<T, String> {
    let handle = rusb::open_device_with_vid_pid(VID, PID)
        .ok_or_else(|| "XPAD not found. Plug it in and try again.".to_string())?;

    // On Linux a kernel driver may hold the interface; ask libusb to detach it
    // automatically. Unsupported elsewhere — ignore the error.
    let _ = handle.set_auto_detach_kernel_driver(true);
    handle
        .claim_interface(IFACE)
        .map_err(|e| format!("Could not open XPAD vendor interface: {e}"))?;

    let result = f(&handle);
    let _ = handle.release_interface(IFACE);
    result
}

/// Send a one-byte command and read the response into a buffer.
fn command(handle: &rusb::DeviceHandle<rusb::GlobalContext>, cmd: u8) -> Result<Vec<u8>, String> {
    let timeout = Duration::from_millis(1000);
    handle
        .write_bulk(EP_OUT, &[cmd], timeout)
        .map_err(|e| format!("USB write failed: {e}"))?;
    // libusb aggregates packets until a short packet, so one read returns the
    // full response (incl. the 84-byte layout that spans two USB packets).
    let mut buf = [0u8; 128];
    let n = handle
        .read_bulk(EP_IN, &mut buf, timeout)
        .map_err(|e| format!("USB read failed: {e}"))?;
    Ok(buf[..n].to_vec())
}

// ── Public reads ─────────────────────────────────────────────────────────────

/// The sorted, de-duplicated list of F13–F24 HID codes mapped on the XPAD.
pub fn read_mapped_fkeys() -> Result<Vec<u8>, String> {
    with_handle(|handle| {
        let resp = command(handle, CMD_READ_KEYS)?;
        if resp.len() < 3 || resp[0] != CMD_READ_KEYS {
            return Err("Unexpected response from XPAD.".to_string());
        }
        let count = resp[2] as usize;
        let mut fkeys = Vec::new();
        for i in 0..count {
            let base = 3 + i * 4;
            if base + 1 >= resp.len() {
                break;
            }
            let hid = resp[base + 1];
            if is_fkey(hid) {
                fkeys.push(hid);
            }
        }
        fkeys.sort_unstable();
        fkeys.dedup();
        Ok(fkeys)
    })
}

/// Read the device layout (0x32) + key map (0x33) and return the full matrix,
/// flagging which cells are Small Deck (F13–F24) triggers.
pub fn read_matrix() -> Result<DeviceMatrix, String> {
    with_handle(|handle| {
        let lay = command(handle, CMD_READ_LAYOUT)?;
        if lay.len() < 4 || lay[0] != CMD_READ_LAYOUT {
            return Err("Unexpected layout response from XPAD.".to_string());
        }
        let rows = lay[2];
        let cols = lay[3];

        let keys = command(handle, CMD_READ_KEYS)?;
        if keys.len() < 3 || keys[0] != CMD_READ_KEYS {
            return Err("Unexpected key-map response from XPAD.".to_string());
        }
        // gpio -> hid for as many keys as the firmware reported (caps at 15).
        let mut gpio_hid: std::collections::HashMap<u8, u8> = std::collections::HashMap::new();
        let count = keys[2] as usize;
        for i in 0..count {
            let base = 3 + i * 4;
            if base + 1 >= keys.len() {
                break;
            }
            gpio_hid.insert(keys[base], keys[base + 1]);
        }

        let mut cells = Vec::new();
        for r in 0..rows as usize {
            for c in 0..cols as usize {
                let idx = 4 + r * LAYOUT_MAX_COLS + c;
                let gpio = lay.get(idx).copied().unwrap_or(0xFF);
                let occupied = gpio != 0xFF;
                let fkey = if occupied {
                    gpio_hid.get(&gpio).copied().filter(|h| is_fkey(*h)).map(fkey_label)
                } else {
                    None
                };
                cells.push(Cell {
                    row: r as u8,
                    col: c as u8,
                    pos: format!("{}{}", (b'A' + r as u8) as char, c + 1),
                    occupied,
                    fkey,
                });
            }
        }
        Ok(DeviceMatrix { rows, cols, cells })
    })
}
