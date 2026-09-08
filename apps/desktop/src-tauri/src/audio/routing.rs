use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Where a measurement pair's reference signal comes from.
///
/// `Generator` preserves today's implicit behaviour: the generator's own
/// output, captured in `reference_buffer`, is the reference. `Input` is the
/// new capability — measuring the return of a console output, or using one
/// microphone as the reference for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum ReferenceSource {
    Generator,
    Input { channel: usize },
}

/// One measurement channel paired with a reference source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementPair {
    pub id: String,
    pub name: String,
    pub measurement_channel: usize,
    pub reference: ReferenceSource,
    pub delay_samples: usize,
    pub calibration_offset_db: f32,
    pub enabled: bool,
}

/// The full set of measurement pairs configured for a session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Routing {
    pub pairs: Vec<MeasurementPair>,
}

impl Routing {
    /// Rejects configurations that would silently produce meaningless data:
    /// a channel index past the device's channel count, a pair referenced
    /// to its own measurement channel (the transfer function of a wire),
    /// and duplicate pair ids.
    pub fn validate(&self, channel_count: usize) -> Result<(), String> {
        let mut seen_ids = HashSet::with_capacity(self.pairs.len());

        for pair in &self.pairs {
            if !seen_ids.insert(&pair.id) {
                return Err(format!("duplicate pair id: '{}'", pair.id));
            }

            if pair.measurement_channel >= channel_count {
                return Err(format!(
                    "pair '{}': measurement channel {} is out of range for {channel_count} input channel(s)",
                    pair.id, pair.measurement_channel
                ));
            }

            if let ReferenceSource::Input { channel } = pair.reference {
                if channel >= channel_count {
                    return Err(format!(
                        "pair '{}': reference channel {channel} is out of range for {channel_count} input channel(s)",
                        pair.id
                    ));
                }

                if channel == pair.measurement_channel {
                    return Err(format!(
                        "pair '{}': measurement channel {} is referenced to itself",
                        pair.id, pair.measurement_channel
                    ));
                }
            }
        }

        Ok(())
    }

    /// Enabled pairs only — the ones the engine should actually measure.
    pub fn active_pairs(&self) -> impl Iterator<Item = &MeasurementPair> {
        self.pairs.iter().filter(|pair| pair.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(id: &str, measurement: usize, reference: ReferenceSource) -> MeasurementPair {
        MeasurementPair {
            id: id.into(),
            name: id.into(),
            measurement_channel: measurement,
            reference,
            delay_samples: 0,
            calibration_offset_db: 0.0,
            enabled: true,
        }
    }

    #[test]
    fn a_simple_generator_referenced_pair_is_valid() {
        let routing = Routing {
            pairs: vec![pair("a", 0, ReferenceSource::Generator)],
        };
        assert!(routing.validate(8).is_ok());
    }

    #[test]
    fn a_channel_past_the_device_count_is_rejected() {
        let routing = Routing {
            pairs: vec![pair("a", 9, ReferenceSource::Generator)],
        };
        let error = routing.validate(8).unwrap_err();
        assert!(error.contains('9'), "the error should name the bad channel: {error}");
    }

    /// Measuring a channel against itself gives a flat trace and perfect
    /// coherence regardless of what is connected. It looks like a perfect
    /// measurement, which is exactly why it must be refused rather than drawn.
    #[test]
    fn a_pair_referenced_to_itself_is_rejected() {
        let routing = Routing {
            pairs: vec![pair("a", 3, ReferenceSource::Input { channel: 3 })],
        };
        let error = routing.validate(8).unwrap_err();
        assert!(
            error.to_lowercase().contains("itself") || error.contains('3'),
            "unhelpful error: {error}"
        );
    }

    #[test]
    fn duplicate_pair_ids_are_rejected() {
        let routing = Routing {
            pairs: vec![
                pair("a", 0, ReferenceSource::Generator),
                pair("a", 1, ReferenceSource::Generator),
            ],
        };
        assert!(routing.validate(8).is_err());
    }

    /// Two pairs may share a measurement channel — measuring one mic against
    /// two different references is a real workflow, not a mistake.
    #[test]
    fn two_pairs_may_share_a_channel() {
        let routing = Routing {
            pairs: vec![
                pair("a", 0, ReferenceSource::Generator),
                pair("b", 0, ReferenceSource::Input { channel: 1 }),
            ],
        };
        assert!(routing.validate(8).is_ok());
    }

    #[test]
    fn disabled_pairs_are_skipped() {
        let mut disabled = pair("b", 1, ReferenceSource::Generator);
        disabled.enabled = false;
        let routing = Routing {
            pairs: vec![pair("a", 0, ReferenceSource::Generator), disabled],
        };
        assert_eq!(routing.active_pairs().count(), 1);
    }

    /// An empty routing is valid — it is what the engine starts with.
    #[test]
    fn an_empty_routing_is_valid() {
        let routing = Routing { pairs: vec![] };
        assert!(routing.validate(8).is_ok());
        assert_eq!(routing.active_pairs().count(), 0);
    }
}
