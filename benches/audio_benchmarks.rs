use criterion::{Criterion, criterion_group, criterion_main};
use plugin_render_bridge::RenderSettings;
use plugin_render_bridge::buffer::AudioBuffer;
use plugin_render_bridge::pool::BufferPool;
use plugin_render_bridge::render::{AudioProcessor, render_note};
use plugin_render_bridge::testing::MockVstPlugin;

fn bench_render_note(c: &mut Criterion) {
    let config = RenderSettings {
        note_duration_secs: 0.1,
        silence_threshold: 0.01,
        buffer_size: 512,
        ..Default::default()
    };

    c.bench_function("render_note_c4", |b| {
        b.iter(|| {
            let mut plugin = MockVstPlugin::new(44100);
            render_note(&mut plugin, 60, 127, &config, None, 0).unwrap();
        })
    });
}

fn bench_buffer_operations(c: &mut Criterion) {
    c.bench_function("buffer_new_stereo_1sec", |b| {
        b.iter(|| AudioBuffer::new(2, 44100, 44100))
    });

    c.bench_function("buffer_interleave_1sec", |b| {
        let buf = AudioBuffer::new(2, 44100, 44100);
        b.iter(|| buf.to_interleaved())
    });

    c.bench_function("buffer_rms_1sec", |b| {
        let mut buf = AudioBuffer::new(2, 44100, 44100);
        for i in 0..44100 {
            let val = (i as f32 * 0.01).sin();
            buf.channel_mut(0)[i] = val;
            buf.channel_mut(1)[i] = val;
        }
        b.iter(|| buf.rms())
    });
}

fn bench_buffer_pool(c: &mut Criterion) {
    c.bench_function("pool_checkout_return", |b| {
        let pool = BufferPool::new(2, 512, 44100, 8);
        b.iter(|| {
            let buf = pool.checkout();
            pool.return_buffer(buf);
        })
    });
}

fn bench_effect_chain(c: &mut Criterion) {
    use plugin_render_bridge::effect_chain::EffectChain;
    use plugin_render_bridge::midi::MidiEvent;

    struct GainEffect(f32);
    impl AudioProcessor for GainEffect {
        fn process_block(
            &mut self,
            _events: &[MidiEvent],
            output: &mut AudioBuffer,
        ) -> Result<(), String> {
            for ch in 0..output.num_channels() {
                for s in output.channel_mut(ch).iter_mut() {
                    *s *= self.0;
                }
            }
            Ok(())
        }
    }

    c.bench_function("effect_chain_3_effects", |b| {
        let mut chain = EffectChain::new();
        chain.add("G1".into(), Box::new(GainEffect(0.9)));
        chain.add("G2".into(), Box::new(GainEffect(0.8)));
        chain.add("G3".into(), Box::new(GainEffect(0.7)));

        let mut buf = AudioBuffer::new(2, 512, 44100);
        for i in 0..512 {
            buf.channel_mut(0)[i] = (i as f32 * 0.01).sin();
            buf.channel_mut(1)[i] = (i as f32 * 0.01).sin();
        }

        b.iter(|| {
            let mut work = buf.clone();
            chain.process(&mut work).unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_render_note,
    bench_buffer_operations,
    bench_buffer_pool,
    bench_effect_chain
);
criterion_main!(benches);
