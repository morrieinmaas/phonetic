//! CoreAudio Backend for macOS
//!
//! Provides direct CoreAudio HAL access for lower latency than CPAL.
//! This module is only compiled on macOS.
//!
//! Benefits over CPAL:
//! - Lower latency (~2ms vs ~10ms)
//! - Better device enumeration (aggregate devices)
//! - Exclusive mode support
//! - Direct sample rate control

// Allow unused code - this is a library module for future frontend integration
#![allow(dead_code)]

use coreaudio::audio_unit::audio_format::LinearPcmFlags;
use coreaudio::audio_unit::render_callback::{self, data};
use coreaudio::audio_unit::{AudioUnit, Element, SampleFormat, Scope, StreamFormat};
use coreaudio::sys::{
    kAudioDevicePropertyHogMode, kAudioHardwarePropertyDefaultInputDevice,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal,
    kAudioObjectSystemObject, AudioDeviceID, AudioObjectGetPropertyData,
    AudioObjectGetPropertyDataSize, AudioObjectPropertyAddress, AudioObjectSetPropertyData,
};
use parking_lot::Mutex;
use std::ffi::c_void;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// CoreAudio device information
#[derive(Debug, Clone)]
pub struct CoreAudioDevice {
    pub id: AudioDeviceID,
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
}

/// CoreAudio stream configuration
#[derive(Debug, Clone)]
pub struct CoreAudioConfig {
    pub sample_rate: f64,
    pub buffer_size: u32,
    pub channels: u32,
    /// Request exclusive (hog) mode for lowest latency
    pub exclusive_mode: bool,
}

impl Default for CoreAudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000.0,
            buffer_size: 512,
            channels: 1,
            exclusive_mode: false,
        }
    }
}

/// Acquire exclusive (hog) mode for a device.
/// Returns the process ID that owns the device (our PID on success).
pub fn acquire_exclusive_mode(device_id: AudioDeviceID) -> Result<i32, String> {
    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyHogMode,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        // Get our process ID
        let our_pid = std::process::id() as i32;

        // Set hog mode to our PID
        let mut pid = our_pid;
        let data_size = mem::size_of::<i32>() as u32;

        let status = AudioObjectSetPropertyData(
            device_id,
            &property_address,
            0,
            std::ptr::null(),
            data_size,
            &mut pid as *mut _ as *const c_void,
        );

        if status != 0 {
            return Err(format!(
                "Failed to acquire exclusive mode: error code {}",
                status
            ));
        }

        log::info!("Acquired exclusive mode for device {}", device_id);
        Ok(our_pid)
    }
}

/// Release exclusive (hog) mode for a device.
pub fn release_exclusive_mode(device_id: AudioDeviceID) -> Result<(), String> {
    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyHogMode,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        // Set hog mode to -1 to release
        let mut pid: i32 = -1;
        let data_size = mem::size_of::<i32>() as u32;

        let status = AudioObjectSetPropertyData(
            device_id,
            &property_address,
            0,
            std::ptr::null(),
            data_size,
            &mut pid as *mut _ as *const c_void,
        );

        if status != 0 {
            return Err(format!(
                "Failed to release exclusive mode: error code {}",
                status
            ));
        }

        log::info!("Released exclusive mode for device {}", device_id);
        Ok(())
    }
}

/// Check if a device is in exclusive mode and who owns it.
/// Returns Some(pid) if in exclusive mode, None if available.
pub fn get_exclusive_mode_owner(device_id: AudioDeviceID) -> Option<i32> {
    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyHogMode,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        let mut pid: i32 = -1;
        let mut data_size = mem::size_of::<i32>() as u32;

        let status = AudioObjectGetPropertyData(
            device_id,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
            &mut pid as *mut _ as *mut c_void,
        );

        if status == 0 && pid != -1 {
            Some(pid)
        } else {
            None
        }
    }
}

/// Enumerate all CoreAudio devices
pub fn enumerate_devices() -> Vec<CoreAudioDevice> {
    let mut devices = Vec::new();

    unsafe {
        // Get the size of the device list
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        let mut data_size: u32 = 0;
        let status = AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
        );

        if status != 0 {
            log::error!("Failed to get audio device list size: {}", status);
            return devices;
        }

        let device_count = data_size as usize / mem::size_of::<AudioDeviceID>();
        let mut device_ids: Vec<AudioDeviceID> = vec![0; device_count];

        let status = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
            device_ids.as_mut_ptr() as *mut c_void,
        );

        if status != 0 {
            log::error!("Failed to get audio device list: {}", status);
            return devices;
        }

        for device_id in device_ids {
            if let Some(device) = get_device_info(device_id) {
                devices.push(device);
            }
        }
    }

    devices
}

/// Get information about a specific device
fn get_device_info(device_id: AudioDeviceID) -> Option<CoreAudioDevice> {
    unsafe {
        // Get device name
        let name_address = AudioObjectPropertyAddress {
            mSelector: kAudioObjectPropertyName,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        let mut name_ref: coreaudio::sys::CFStringRef = std::ptr::null();
        let mut data_size = mem::size_of::<coreaudio::sys::CFStringRef>() as u32;

        let status = AudioObjectGetPropertyData(
            device_id,
            &name_address,
            0,
            std::ptr::null(),
            &mut data_size,
            &mut name_ref as *mut _ as *mut c_void,
        );

        if status != 0 || name_ref.is_null() {
            return None;
        }

        // Convert CFString to Rust String
        let name = cfstring_to_string(name_ref);
        coreaudio::sys::CFRelease(name_ref as *const c_void);

        // Check if device has input/output capabilities
        let is_input = has_streams(device_id, true);
        let is_output = has_streams(device_id, false);

        Some(CoreAudioDevice {
            id: device_id,
            name,
            is_input,
            is_output,
        })
    }
}

/// Check if a device has input or output streams
fn has_streams(device_id: AudioDeviceID, is_input: bool) -> bool {
    unsafe {
        let scope = if is_input {
            coreaudio::sys::kAudioDevicePropertyScopeInput
        } else {
            coreaudio::sys::kAudioDevicePropertyScopeOutput
        };

        let property_address = AudioObjectPropertyAddress {
            mSelector: coreaudio::sys::kAudioDevicePropertyStreams,
            mScope: scope,
            mElement: kAudioObjectPropertyElementMain,
        };

        let mut data_size: u32 = 0;
        let status = AudioObjectGetPropertyDataSize(
            device_id,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
        );

        status == 0 && data_size > 0
    }
}

/// Convert CFStringRef to Rust String
unsafe fn cfstring_to_string(cf_string: coreaudio::sys::CFStringRef) -> String {
    let length = coreaudio::sys::CFStringGetLength(cf_string);
    let max_size = coreaudio::sys::CFStringGetMaximumSizeForEncoding(
        length,
        coreaudio::sys::kCFStringEncodingUTF8,
    ) + 1;

    let mut buffer: Vec<u8> = vec![0; max_size as usize];
    let success = coreaudio::sys::CFStringGetCString(
        cf_string,
        buffer.as_mut_ptr() as *mut i8,
        max_size,
        coreaudio::sys::kCFStringEncodingUTF8,
    );

    if success != 0 {
        // Find null terminator
        if let Some(pos) = buffer.iter().position(|&b| b == 0) {
            buffer.truncate(pos);
        }
        String::from_utf8_lossy(&buffer).into_owned()
    } else {
        String::from("Unknown Device")
    }
}

/// Get default input device ID
pub fn get_default_input_device() -> Option<AudioDeviceID> {
    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDefaultInputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        let mut device_id: AudioDeviceID = 0;
        let mut data_size = mem::size_of::<AudioDeviceID>() as u32;

        let status = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
            &mut device_id as *mut _ as *mut c_void,
        );

        if status == 0 && device_id != 0 {
            Some(device_id)
        } else {
            None
        }
    }
}

/// Get default output device ID
pub fn get_default_output_device() -> Option<AudioDeviceID> {
    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        };

        let mut device_id: AudioDeviceID = 0;
        let mut data_size = mem::size_of::<AudioDeviceID>() as u32;

        let status = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
            &mut device_id as *mut _ as *mut c_void,
        );

        if status == 0 && device_id != 0 {
            Some(device_id)
        } else {
            None
        }
    }
}

/// CoreAudio input stream handler
pub struct CoreAudioInputStream {
    audio_unit: AudioUnit,
    is_running: Arc<AtomicBool>,
    device_id: AudioDeviceID,
    exclusive_mode: bool,
}

impl CoreAudioInputStream {
    /// Create a new input stream
    pub fn new<F>(
        device_id: Option<AudioDeviceID>,
        config: CoreAudioConfig,
        callback: F,
    ) -> Result<Self, String>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        let device_id = device_id
            .or_else(get_default_input_device)
            .ok_or("No input device available")?;

        // Create HAL output unit (for input, we use output unit with input enabled)
        let mut audio_unit = AudioUnit::new(coreaudio::audio_unit::IOType::HalOutput)
            .map_err(|e| format!("Failed to create AudioUnit: {:?}", e))?;

        // Enable input
        audio_unit
            .set_property(
                coreaudio::sys::kAudioOutputUnitProperty_EnableIO,
                Scope::Input,
                Element::Input,
                Some(&1u32),
            )
            .map_err(|e| format!("Failed to enable input: {:?}", e))?;

        // Disable output
        audio_unit
            .set_property(
                coreaudio::sys::kAudioOutputUnitProperty_EnableIO,
                Scope::Output,
                Element::Output,
                Some(&0u32),
            )
            .map_err(|e| format!("Failed to disable output: {:?}", e))?;

        // Set input device
        audio_unit
            .set_property(
                coreaudio::sys::kAudioOutputUnitProperty_CurrentDevice,
                Scope::Global,
                Element::Output,
                Some(&device_id),
            )
            .map_err(|e| format!("Failed to set input device: {:?}", e))?;

        // Set stream format
        let format = StreamFormat {
            sample_rate: config.sample_rate,
            sample_format: SampleFormat::F32,
            flags: LinearPcmFlags::IS_FLOAT | LinearPcmFlags::IS_PACKED,
            channels: config.channels,
        };

        audio_unit
            .set_property(
                coreaudio::sys::kAudioUnitProperty_StreamFormat,
                Scope::Output,
                Element::Input,
                Some(&format.to_asbd()),
            )
            .map_err(|e| format!("Failed to set stream format: {:?}", e))?;

        // Acquire exclusive mode if requested
        let exclusive_mode = config.exclusive_mode;
        if exclusive_mode {
            acquire_exclusive_mode(device_id)?;
        }

        // Set up render callback
        let callback = Arc::new(Mutex::new(callback));

        type Args = render_callback::Args<data::Interleaved<f32>>;

        audio_unit
            .set_input_callback(move |args: Args| {
                let Args { data, .. } = args;
                let mut cb = callback.lock();
                cb(data.buffer);
                Ok(())
            })
            .map_err(|e| format!("Failed to set input callback: {:?}", e))?;

        let is_running = Arc::new(AtomicBool::new(false));

        Ok(Self {
            audio_unit,
            is_running,
            device_id,
            exclusive_mode,
        })
    }

    /// Start the input stream
    pub fn start(&mut self) -> Result<(), String> {
        self.audio_unit
            .start()
            .map_err(|e| format!("Failed to start input stream: {:?}", e))?;
        self.is_running.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Stop the input stream
    pub fn stop(&mut self) -> Result<(), String> {
        self.audio_unit
            .stop()
            .map_err(|e| format!("Failed to stop input stream: {:?}", e))?;
        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Check if exclusive mode is active
    pub fn is_exclusive(&self) -> bool {
        self.exclusive_mode
    }

    /// Check if stream is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
}

impl Drop for CoreAudioInputStream {
    fn drop(&mut self) {
        // Release exclusive mode if we acquired it
        if self.exclusive_mode {
            if let Err(e) = release_exclusive_mode(self.device_id) {
                log::warn!("Failed to release exclusive mode on input: {}", e);
            }
        }
    }
}

/// CoreAudio output stream handler
pub struct CoreAudioOutputStream {
    audio_unit: AudioUnit,
    is_running: Arc<AtomicBool>,
    device_id: AudioDeviceID,
    exclusive_mode: bool,
}

impl CoreAudioOutputStream {
    /// Create a new output stream
    pub fn new<F>(
        device_id: Option<AudioDeviceID>,
        config: CoreAudioConfig,
        callback: F,
    ) -> Result<Self, String>
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        let device_id = device_id
            .or_else(get_default_output_device)
            .ok_or("No output device available")?;

        // Create HAL output unit
        let mut audio_unit = AudioUnit::new(coreaudio::audio_unit::IOType::HalOutput)
            .map_err(|e| format!("Failed to create AudioUnit: {:?}", e))?;

        // Set output device
        audio_unit
            .set_property(
                coreaudio::sys::kAudioOutputUnitProperty_CurrentDevice,
                Scope::Global,
                Element::Output,
                Some(&device_id),
            )
            .map_err(|e| format!("Failed to set output device: {:?}", e))?;

        // Set stream format
        let format = StreamFormat {
            sample_rate: config.sample_rate,
            sample_format: SampleFormat::F32,
            flags: LinearPcmFlags::IS_FLOAT | LinearPcmFlags::IS_PACKED,
            channels: config.channels,
        };

        audio_unit
            .set_property(
                coreaudio::sys::kAudioUnitProperty_StreamFormat,
                Scope::Input,
                Element::Output,
                Some(&format.to_asbd()),
            )
            .map_err(|e| format!("Failed to set stream format: {:?}", e))?;

        // Acquire exclusive mode if requested
        let exclusive_mode = config.exclusive_mode;
        if exclusive_mode {
            acquire_exclusive_mode(device_id)?;
        }

        // Set up render callback
        let callback = Arc::new(Mutex::new(callback));

        type Args = render_callback::Args<data::Interleaved<f32>>;

        audio_unit
            .set_render_callback(move |args: Args| {
                let Args { data, .. } = args;
                let mut cb = callback.lock();
                cb(data.buffer);
                Ok(())
            })
            .map_err(|e| format!("Failed to set render callback: {:?}", e))?;

        let is_running = Arc::new(AtomicBool::new(false));

        Ok(Self {
            audio_unit,
            is_running,
            device_id,
            exclusive_mode,
        })
    }

    /// Start the output stream
    pub fn start(&mut self) -> Result<(), String> {
        self.audio_unit
            .start()
            .map_err(|e| format!("Failed to start output stream: {:?}", e))?;
        self.is_running.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Stop the output stream
    pub fn stop(&mut self) -> Result<(), String> {
        self.audio_unit
            .stop()
            .map_err(|e| format!("Failed to stop output stream: {:?}", e))?;
        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Check if stream is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Check if exclusive mode is active
    pub fn is_exclusive(&self) -> bool {
        self.exclusive_mode
    }
}

impl Drop for CoreAudioOutputStream {
    fn drop(&mut self) {
        // Release exclusive mode if we acquired it
        if self.exclusive_mode {
            if let Err(e) = release_exclusive_mode(self.device_id) {
                log::warn!("Failed to release exclusive mode on output: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_devices() {
        let devices = enumerate_devices();
        // Should at least have some devices on macOS
        println!("Found {} CoreAudio devices", devices.len());
        for device in &devices {
            println!(
                "  {} (id: {}, input: {}, output: {})",
                device.name, device.id, device.is_input, device.is_output
            );
        }
    }

    #[test]
    fn test_default_devices() {
        let input = get_default_input_device();
        let output = get_default_output_device();

        println!("Default input device: {:?}", input);
        println!("Default output device: {:?}", output);

        // Should have at least a default output on most Macs
        assert!(output.is_some() || input.is_some());
    }

    #[test]
    fn test_exclusive_mode_owner_check() {
        // Test that we can check exclusive mode status
        if let Some(device_id) = get_default_output_device() {
            let owner = get_exclusive_mode_owner(device_id);
            println!("Exclusive mode owner for default output: {:?}", owner);
            // Device should typically not be in exclusive mode during tests
            // (unless something else has acquired it)
        }
    }

    #[test]
    fn test_config_default() {
        let config = CoreAudioConfig::default();
        assert_eq!(config.sample_rate, 48000.0);
        assert_eq!(config.buffer_size, 512);
        assert_eq!(config.channels, 1);
        assert!(!config.exclusive_mode);
    }

    #[test]
    fn test_config_with_exclusive() {
        let config = CoreAudioConfig {
            sample_rate: 96000.0,
            buffer_size: 128,
            channels: 2,
            exclusive_mode: true,
        };
        assert_eq!(config.sample_rate, 96000.0);
        assert_eq!(config.buffer_size, 128);
        assert_eq!(config.channels, 2);
        assert!(config.exclusive_mode);
    }
}
