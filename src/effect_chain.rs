use crate::buffer::AudioBuffer;
use crate::render::AudioProcessor;

/// An individual effect in the chain.
pub struct EffectSlot {
    pub name: String,
    pub processor: Box<dyn AudioProcessor>,
    pub bypassed: bool,
}

/// An ordered chain of audio effects applied sequentially to a buffer.
pub struct EffectChain {
    slots: Vec<EffectSlot>,
}

impl EffectChain {
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Add an effect to the end of the chain.
    pub fn add(&mut self, name: String, processor: Box<dyn AudioProcessor>) {
        self.slots.push(EffectSlot {
            name,
            processor,
            bypassed: false,
        });
    }

    /// Number of effects in the chain.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Toggle bypass for a slot by index.
    pub fn set_bypass(&mut self, index: usize, bypassed: bool) {
        if let Some(slot) = self.slots.get_mut(index) {
            slot.bypassed = bypassed;
        }
    }

    /// Process a buffer through the entire chain in-place.
    /// Each effect receives the output of the previous one.
    pub fn process(&mut self, buffer: &mut AudioBuffer) -> Result<(), String> {
        let block_size = buffer.num_samples();
        if block_size == 0 {
            return Ok(());
        }

        for slot in &mut self.slots {
            if slot.bypassed {
                continue;
            }
            // Process the buffer through this effect
            // The AudioProcessor trait works on blocks, so we pass the whole buffer
            slot.processor.process_block(&[], buffer)?;
        }

        Ok(())
    }
}

impl Default for EffectChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::MidiEvent;

    /// A gain effect that multiplies all samples by a factor.
    struct GainEffect {
        gain: f32,
    }

    impl AudioProcessor for GainEffect {
        fn process_block(
            &mut self,
            _events: &[MidiEvent],
            output: &mut AudioBuffer,
        ) -> Result<(), String> {
            for ch in 0..output.num_channels() {
                for sample in output.channel_mut(ch).iter_mut() {
                    *sample *= self.gain;
                }
            }
            Ok(())
        }
    }

    #[test]
    fn empty_chain() {
        let mut chain = EffectChain::new();
        assert!(chain.is_empty());

        let mut buf = AudioBuffer::new(1, 10, 44100);
        buf.channel_mut(0)[0] = 1.0;
        chain.process(&mut buf).unwrap();
        assert_eq!(buf.channel(0)[0], 1.0);
    }

    #[test]
    fn chain_applies_effects() {
        let mut chain = EffectChain::new();
        chain.add("Gain 0.5".into(), Box::new(GainEffect { gain: 0.5 }));
        chain.add("Gain 0.5".into(), Box::new(GainEffect { gain: 0.5 }));

        let mut buf = AudioBuffer::new(1, 10, 44100);
        buf.channel_mut(0)[0] = 1.0;
        chain.process(&mut buf).unwrap();

        // 1.0 * 0.5 * 0.5 = 0.25
        assert!((buf.channel(0)[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn bypass_skips_effect() {
        let mut chain = EffectChain::new();
        chain.add("Gain".into(), Box::new(GainEffect { gain: 0.5 }));
        chain.set_bypass(0, true);

        let mut buf = AudioBuffer::new(1, 10, 44100);
        buf.channel_mut(0)[0] = 1.0;
        chain.process(&mut buf).unwrap();

        assert_eq!(buf.channel(0)[0], 1.0); // unaffected
    }
}
