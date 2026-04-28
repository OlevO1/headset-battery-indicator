use crate::lang;
use crate::lang::Key::*;

use anyhow::Context;
use libc::{c_char, c_int, c_uchar, c_void};
use std::ffi::CStr;

#[repr(C)]
pub struct HscBattery {
    pub level_percent: c_int,
    pub status: BatteryStatus,
    pub voltage_mv: c_int,
    pub time_to_full_min: c_int,
    pub time_to_empty_min: c_int,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum BatteryStatus {
    #[default]
    Unavailable,
    Charging,
    Available,
    HidError,
    Timeout,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
enum Capability {
    Sidetone = 0,
    BatteryStatus = 1,
    InactiveTime = 4,
    MicrophoneMuteLedBrightness = 11,
}

impl Capability {
    fn bit(self) -> c_int {
        1 << self as c_int
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCapabilities {
    pub sidetone: bool,
    pub microphone_light: bool,
    pub inactive_time: bool,
}

impl ControlCapabilities {
    fn from_mask(mask: c_int) -> Self {
        Self {
            sidetone: mask & Capability::Sidetone.bit() != 0,
            microphone_light: mask & Capability::MicrophoneMuteLedBrightness.bit() != 0,
            inactive_time: mask & Capability::InactiveTime.bit() != 0,
        }
    }

    pub fn has_controls(self) -> bool {
        self.sidetone || self.microphone_light || self.inactive_time
    }
}

#[repr(C)]
struct HscSidetone {
    current_level: c_uchar,
    min_level: c_uchar,
    max_level: c_uchar,
}

#[repr(C)]
struct HscInactiveTime {
    minutes: c_uchar,
    min_minutes: c_uchar,
    max_minutes: c_uchar,
}

#[link(name = "headsetcontrol_static")]
unsafe extern "C" {
    unsafe fn hsc_discover(headsets: *mut *mut c_void) -> c_int;
    unsafe fn hsc_free_headsets(headsets: *mut c_void, count: c_int);
    // unsafe fn hsc_get_name(headset: *mut c_void) -> *const c_char;
    unsafe fn hsc_get_product_name(headset: *mut c_void) -> *const c_char;
    unsafe fn hsc_supports(headset: *mut c_void, cap: c_int) -> bool;
    unsafe fn hsc_get_capabilities(headset: *mut c_void) -> c_int;
    unsafe fn hsc_get_battery(headset: *mut c_void, battery: *mut HscBattery) -> c_int;
    unsafe fn hsc_set_sidetone(
        headset: *mut c_void,
        level: c_uchar,
        result: *mut HscSidetone,
    ) -> c_int;
    unsafe fn hsc_set_mic_mute_led_brightness(headset: *mut c_void, brightness: c_uchar) -> c_int;
    unsafe fn hsc_set_inactive_time(
        headset: *mut c_void,
        minutes: c_uchar,
        result: *mut HscInactiveTime,
    ) -> c_int;
}

pub fn query_device() -> Option<Device> {
    with_first_headset(|headset| unsafe {
        let product_name = string_from_ptr(hsc_get_product_name(headset));
        let capabilities = ControlCapabilities::from_mask(hsc_get_capabilities(headset));
        let mut battery = unavailable_battery();

        if hsc_supports(headset, Capability::BatteryStatus as c_int) {
            let mut queried_battery = unavailable_battery();
            if hsc_get_battery(headset, &mut queried_battery) == 0 {
                battery = queried_battery;
            }
        }

        Device {
            product_name,
            battery,
            capabilities,
        }
    })
}

pub fn set_sidetone_enabled(enabled: bool) -> anyhow::Result<()> {
    set_control(
        Capability::Sidetone,
        |headset| unsafe {
            let mut result = HscSidetone {
                current_level: 0,
                min_level: 0,
                max_level: 0,
            };
            hsc_set_sidetone(headset, if enabled { 128 } else { 0 }, &mut result)
        },
        "setting sidetone",
    )
}

pub fn set_microphone_light_enabled(enabled: bool) -> anyhow::Result<()> {
    set_control(
        Capability::MicrophoneMuteLedBrightness,
        |headset| unsafe { hsc_set_mic_mute_led_brightness(headset, if enabled { 3 } else { 0 }) },
        "setting microphone light",
    )
}

pub fn set_inactive_time_minutes(minutes: u8) -> anyhow::Result<()> {
    set_control(
        Capability::InactiveTime,
        |headset| unsafe {
            let mut result = HscInactiveTime {
                minutes: 0,
                min_minutes: 0,
                max_minutes: 0,
            };
            hsc_set_inactive_time(headset, minutes, &mut result)
        },
        "setting inactive time",
    )
}

fn set_control(
    capability: Capability,
    setter: impl FnOnce(*mut c_void) -> c_int,
    action: &'static str,
) -> anyhow::Result<()> {
    let result = with_first_headset(|headset| unsafe {
        if !hsc_supports(headset, capability as c_int) {
            return Err(anyhow::anyhow!("capability is not supported"));
        }

        let code = setter(headset);
        if code != 0 {
            return Err(anyhow::anyhow!("HeadsetControl returned {code}"));
        }

        Ok(())
    });

    result
        .context("discovering headset")?
        .with_context(|| action)
}

fn with_first_headset<T>(f: impl FnOnce(*mut c_void) -> T) -> Option<T> {
    unsafe {
        let mut headsets: *mut c_void = std::ptr::null_mut();
        let count = hsc_discover(&mut headsets);

        if count <= 0 || headsets.is_null() {
            return None;
        }

        let result = {
            let headset_array =
                std::slice::from_raw_parts(headsets as *const *mut c_void, count as usize);
            f(headset_array[0])
        };

        hsc_free_headsets(headsets, count);

        Some(result)
    }
}

fn unavailable_battery() -> HscBattery {
    HscBattery {
        level_percent: 0,
        status: BatteryStatus::Unavailable,
        voltage_mv: -1,
        time_to_full_min: -1,
        time_to_empty_min: -1,
    }
}

unsafe fn string_from_ptr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return "Unknown".to_string();
    }

    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .unwrap_or("Unknown")
        .to_string()
}

pub struct Device {
    pub product_name: String,
    pub battery: HscBattery,
    pub capabilities: ControlCapabilities,
}

impl Device {
    pub fn status_text(&self) -> Option<&'static str> {
        match self.battery.status {
            BatteryStatus::Charging => Some(lang::t(device_charging)),
            BatteryStatus::Available => None,
            BatteryStatus::Unavailable => Some(lang::t(battery_unavailable)),
            _ => Some(lang::t(device_disconnected)),
        }
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.battery.level_percent > 0 {
            write!(
                f,
                "{name}: {battery}%",
                name = self.product_name,
                battery = self.battery.level_percent,
            )?;
        } else {
            write!(f, "{}", self.product_name)?;
        }

        if let Some(status) = self.status_text() {
            write!(f, " {status}")?;
        }

        Ok(())
    }
}
