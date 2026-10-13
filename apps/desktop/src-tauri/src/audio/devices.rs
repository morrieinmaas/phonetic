use cpal::traits::{DeviceTrait, HostTrait};
use cpal::SupportedStreamConfigRange;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub sample_rates: Vec<u32>,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceList {
    pub inputs: Vec<AudioDevice>,
    pub outputs: Vec<AudioDevice>,
    pub default_input: Option<String>,
    pub default_output: Option<String>,
}

/// Input device names only, for the CLI.
pub fn list_input_devices() -> Vec<String> {
    enumerate_devices()
        .inputs
        .into_iter()
        .map(|d| d.name)
        .collect()
}

pub fn enumerate_devices() -> AudioDeviceList {
    let host = cpal::default_host();

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    // Get default devices
    let default_input = host.default_input_device().and_then(|d| d.name().ok());
    let default_output = host.default_output_device().and_then(|d| d.name().ok());

    // Enumerate input devices
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Some(audio_device) = device_to_audio_device_input(&device) {
                inputs.push(audio_device);
            }
        }
    }

    // Enumerate output devices
    if let Ok(devices) = host.output_devices() {
        for device in devices {
            if let Some(audio_device) = device_to_audio_device_output(&device) {
                outputs.push(audio_device);
            }
        }
    }

    AudioDeviceList {
        inputs,
        outputs,
        default_input,
        default_output,
    }
}

fn device_to_audio_device_input(device: &cpal::Device) -> Option<AudioDevice> {
    let name = device.name().ok()?;
    // Create unique ID: input devices use "input:" prefix + name
    // This distinguishes from output devices with the same name
    let id = format!("input:{}", name);

    let configs = device.supported_input_configs().ok()?;
    let (sample_rates, max_channels) = extract_config_info(configs);

    Some(AudioDevice {
        id,
        name,
        is_input: true,
        is_output: false,
        sample_rates,
        channels: max_channels,
    })
}

fn device_to_audio_device_output(device: &cpal::Device) -> Option<AudioDevice> {
    let name = device.name().ok()?;
    // Create unique ID: output devices use "output:" prefix + name
    // This distinguishes from input devices with the same name
    let id = format!("output:{}", name);

    let configs = device.supported_output_configs().ok()?;
    let (sample_rates, max_channels) = extract_config_info(configs);

    Some(AudioDevice {
        id,
        name,
        is_input: false,
        is_output: true,
        sample_rates,
        channels: max_channels,
    })
}

fn extract_config_info(
    configs: impl Iterator<Item = SupportedStreamConfigRange>,
) -> (Vec<u32>, u16) {
    let mut sample_rates = Vec::new();
    let mut max_channels = 0u16;

    for config in configs {
        let min = config.min_sample_rate().0;
        let max = config.max_sample_rate().0;

        for &rate in &[44100, 48000, 88200, 96000, 176400, 192000] {
            if rate >= min && rate <= max && !sample_rates.contains(&rate) {
                sample_rates.push(rate);
            }
        }

        max_channels = max_channels.max(config.channels());
    }

    sample_rates.sort();
    (sample_rates, max_channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_device_struct() {
        let device = AudioDevice {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            is_input: true,
            is_output: false,
            sample_rates: vec![44100, 48000],
            channels: 2,
        };

        assert_eq!(device.id, "test-device");
        assert_eq!(device.name, "Test Device");
        assert!(device.is_input);
        assert!(!device.is_output);
        assert_eq!(device.sample_rates.len(), 2);
        assert_eq!(device.channels, 2);
    }

    #[test]
    fn test_audio_device_list_struct() {
        let list = AudioDeviceList {
            inputs: vec![],
            outputs: vec![],
            default_input: Some("Input 1".to_string()),
            default_output: None,
        };

        assert!(list.inputs.is_empty());
        assert!(list.outputs.is_empty());
        assert_eq!(list.default_input, Some("Input 1".to_string()));
        assert!(list.default_output.is_none());
    }

    #[test]
    fn test_enumerate_devices_returns_valid_structure() {
        // This test will run on CI, so we can't assume specific devices
        // But we can verify the structure is valid
        let devices = enumerate_devices();

        // The structure should always be valid (even if empty on CI)
        // Default device names should either be None or a non-empty string
        if let Some(ref name) = devices.default_input {
            assert!(!name.is_empty());
        }
        if let Some(ref name) = devices.default_output {
            assert!(!name.is_empty());
        }

        // All input devices should be marked as input
        for device in &devices.inputs {
            assert!(device.is_input);
            assert!(!device.is_output);
            assert!(!device.id.is_empty());
            assert!(!device.name.is_empty());
        }

        // All output devices should be marked as output
        for device in &devices.outputs {
            assert!(!device.is_input);
            assert!(device.is_output);
            assert!(!device.id.is_empty());
            assert!(!device.name.is_empty());
        }
    }

    #[test]
    fn test_device_serialization() {
        let device = AudioDevice {
            id: "test".to_string(),
            name: "Test".to_string(),
            is_input: true,
            is_output: false,
            sample_rates: vec![48000],
            channels: 2,
        };

        let json = serde_json::to_string(&device).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"sample_rates\":[48000]"));

        let deserialized: AudioDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, device.id);
        assert_eq!(deserialized.channels, device.channels);
    }
}
