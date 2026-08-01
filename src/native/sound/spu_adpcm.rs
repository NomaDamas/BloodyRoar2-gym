pub const SPU_ADPCM_BLOCK_BYTES: usize = 16;
pub const SPU_ADPCM_SAMPLES_PER_BLOCK: usize = 28;

const PSX_SPU_FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];
const SPU_VOICE_RELEASE_STEP_Q15: i32 = 2048;
const SPU_VOICE_GAIN_ONE_Q15: i32 = 0x7fff;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpuAdpcmBlockFlags {
    pub end: bool,
    pub loop_repeat: bool,
    pub loop_start: bool,
}

impl SpuAdpcmBlockFlags {
    fn from_byte(value: u8) -> Self {
        Self {
            end: value & 0x01 != 0,
            loop_repeat: value & 0x02 != 0,
            loop_start: value & 0x04 != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpuAdpcmDecoder {
    previous: [i16; 2],
}

impl SpuAdpcmDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.previous = [0; 2];
    }

    pub fn decode_block(
        &mut self,
        block: &[u8; SPU_ADPCM_BLOCK_BYTES],
    ) -> ([i16; SPU_ADPCM_SAMPLES_PER_BLOCK], SpuAdpcmBlockFlags) {
        let shift = (block[0] & 0x0f).min(12);
        let filter = ((block[0] >> 4) & 0x07).min(4) as usize;
        let flags = SpuAdpcmBlockFlags::from_byte(block[1]);
        let (coef0, coef1) = PSX_SPU_FILTERS[filter];
        let mut samples = [0i16; SPU_ADPCM_SAMPLES_PER_BLOCK];

        for sample_index in 0..SPU_ADPCM_SAMPLES_PER_BLOCK {
            let encoded = block[2 + sample_index / 2];
            let nibble = if sample_index & 1 == 0 {
                encoded & 0x0f
            } else {
                encoded >> 4
            };
            let mut sample = i32::from(sign_extend_4bit(nibble)) << 12;
            sample >>= shift;
            sample +=
                (i32::from(self.previous[0]) * coef0 + i32::from(self.previous[1]) * coef1 + 32)
                    >> 6;
            let decoded = clamp_i16(sample);
            samples[sample_index] = decoded;
            self.previous[1] = self.previous[0];
            self.previous[0] = decoded;
        }

        (samples, flags)
    }
}

#[derive(Clone, Debug)]
pub struct PsxSpuVoice {
    ram: Vec<u8>,
    decoder: SpuAdpcmDecoder,
    block_samples: [i16; SPU_ADPCM_SAMPLES_PER_BLOCK],
    block_sample_index: usize,
    current_block_addr: usize,
    repeat_block_addr: usize,
    active: bool,
    stop_after_block: bool,
    releasing: bool,
    release_gain_q15: i32,
}

impl PsxSpuVoice {
    pub fn new(ram: Vec<u8>) -> Self {
        Self {
            ram,
            decoder: SpuAdpcmDecoder::new(),
            block_samples: [0; SPU_ADPCM_SAMPLES_PER_BLOCK],
            block_sample_index: SPU_ADPCM_SAMPLES_PER_BLOCK,
            current_block_addr: 0,
            repeat_block_addr: 0,
            active: false,
            stop_after_block: false,
            releasing: false,
            release_gain_q15: SPU_VOICE_GAIN_ONE_Q15,
        }
    }

    pub fn key_on(&mut self, start_block_addr: usize) {
        self.decoder.reset();
        self.current_block_addr = align_block_addr(start_block_addr);
        self.repeat_block_addr = self.current_block_addr;
        self.block_sample_index = SPU_ADPCM_SAMPLES_PER_BLOCK;
        self.active = true;
        self.stop_after_block = false;
        self.releasing = false;
        self.release_gain_q15 = SPU_VOICE_GAIN_ONE_Q15;
    }

    pub fn key_off(&mut self) {
        if self.active {
            self.releasing = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn render_samples(&mut self, output: &mut [i16]) {
        for sample in output {
            *sample = self.next_sample();
        }
    }

    fn next_sample(&mut self) -> i16 {
        if !self.active {
            return 0;
        }

        if self.block_sample_index >= SPU_ADPCM_SAMPLES_PER_BLOCK && !self.load_next_block() {
            self.active = false;
            return 0;
        }

        let mut sample = self.block_samples[self.block_sample_index];
        self.block_sample_index += 1;

        if self.releasing {
            sample = scale_i16_q15(sample, self.release_gain_q15);
            self.release_gain_q15 = self
                .release_gain_q15
                .saturating_sub(SPU_VOICE_RELEASE_STEP_Q15);
            if self.release_gain_q15 <= 0 {
                self.active = false;
                self.stop_after_block = false;
                self.releasing = false;
                self.release_gain_q15 = 0;
            }
        }
        if self.stop_after_block && self.block_sample_index >= SPU_ADPCM_SAMPLES_PER_BLOCK {
            self.active = false;
            self.stop_after_block = false;
        }

        sample
    }

    fn load_next_block(&mut self) -> bool {
        let Some(block) = self.block_at(self.current_block_addr) else {
            return false;
        };
        let (samples, flags) = self.decoder.decode_block(&block);
        self.block_samples = samples;
        self.block_sample_index = 0;

        let decoded_block_addr = self.current_block_addr;
        if flags.loop_start {
            self.repeat_block_addr = decoded_block_addr;
        }
        if flags.end {
            if flags.loop_repeat {
                self.current_block_addr = self.repeat_block_addr;
                self.stop_after_block = false;
            } else {
                self.current_block_addr = decoded_block_addr.saturating_add(SPU_ADPCM_BLOCK_BYTES);
                self.stop_after_block = true;
            }
        } else {
            self.current_block_addr = decoded_block_addr.saturating_add(SPU_ADPCM_BLOCK_BYTES);
            self.stop_after_block = false;
        }
        true
    }

    fn block_at(&self, block_addr: usize) -> Option<[u8; SPU_ADPCM_BLOCK_BYTES]> {
        let end = block_addr.checked_add(SPU_ADPCM_BLOCK_BYTES)?;
        let bytes = self.ram.get(block_addr..end)?;
        let mut block = [0u8; SPU_ADPCM_BLOCK_BYTES];
        block.copy_from_slice(bytes);
        Some(block)
    }
}

fn align_block_addr(address: usize) -> usize {
    address - (address % SPU_ADPCM_BLOCK_BYTES)
}

fn sign_extend_4bit(value: u8) -> i8 {
    ((value << 4) as i8) >> 4
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn scale_i16_q15(sample: i16, gain_q15: i32) -> i16 {
    let scaled = (i32::from(sample) * gain_q15.max(0)) >> 15;
    clamp_i16(scaled)
}

#[cfg(test)]
mod tests {
    use super::{PsxSpuVoice, SPU_ADPCM_BLOCK_BYTES, SPU_ADPCM_SAMPLES_PER_BLOCK, SpuAdpcmDecoder};

    fn adpcm_block(header: u8, flags: u8, packed_nibbles: u8) -> [u8; SPU_ADPCM_BLOCK_BYTES] {
        let mut block = [0u8; SPU_ADPCM_BLOCK_BYTES];
        block[0] = header;
        block[1] = flags;
        block[2..].fill(packed_nibbles);
        block
    }

    #[test]
    fn psx_spu_adpcm_decodes_signed_nibbles_low_nibble_first() {
        let mut decoder = SpuAdpcmDecoder::new();
        let block = adpcm_block(0x00, 0x00, 0x1f);

        let (samples, flags) = decoder.decode_block(&block);

        assert_eq!(samples[0], -4096);
        assert_eq!(samples[1], 4096);
        assert_eq!(samples[2], -4096);
        assert_eq!(samples[3], 4096);
        assert!(!flags.end);
        assert!(!flags.loop_repeat);
        assert!(!flags.loop_start);
    }

    #[test]
    fn psx_spu_adpcm_predictor_uses_history_between_blocks() {
        let mut decoder = SpuAdpcmDecoder::new();
        let warmup = adpcm_block(0x00, 0x00, 0x11);
        let filtered_zeroes = adpcm_block(0x10, 0x00, 0x00);

        decoder.decode_block(&warmup);
        let (samples, _) = decoder.decode_block(&filtered_zeroes);

        assert!(samples[0] > 0, "{samples:?}");
        assert!(samples[1] > 0, "{samples:?}");
        assert!(samples[1] <= samples[0], "{samples:?}");
    }

    #[test]
    fn psx_spu_voice_stops_one_shot_end_block_instead_of_repeating_effect() {
        let ram = adpcm_block(0x00, 0x01, 0x11).to_vec();
        let mut voice = PsxSpuVoice::new(ram);
        let mut output = [0i16; SPU_ADPCM_SAMPLES_PER_BLOCK * 2];

        voice.key_on(0);
        voice.render_samples(&mut output);

        assert!(
            output[..SPU_ADPCM_SAMPLES_PER_BLOCK]
                .iter()
                .all(|sample| *sample != 0)
        );
        assert!(
            output[SPU_ADPCM_SAMPLES_PER_BLOCK..]
                .iter()
                .all(|sample| *sample == 0)
        );
        assert!(!voice.is_active());
    }

    #[test]
    fn psx_spu_voice_repeats_from_loop_start_only_when_end_block_requests_repeat() {
        let mut ram = Vec::new();
        ram.extend_from_slice(&adpcm_block(0x00, 0x04, 0x11));
        ram.extend_from_slice(&adpcm_block(0x00, 0x03, 0x22));
        let mut voice = PsxSpuVoice::new(ram);
        let mut output = [0i16; SPU_ADPCM_SAMPLES_PER_BLOCK * 3];

        voice.key_on(0);
        voice.render_samples(&mut output);

        assert!(
            output[..SPU_ADPCM_SAMPLES_PER_BLOCK]
                .iter()
                .all(|sample| *sample == 4096)
        );
        assert!(
            output[SPU_ADPCM_SAMPLES_PER_BLOCK..SPU_ADPCM_SAMPLES_PER_BLOCK * 2]
                .iter()
                .all(|sample| *sample == 8192)
        );
        assert!(
            output[SPU_ADPCM_SAMPLES_PER_BLOCK * 2..]
                .iter()
                .all(|sample| *sample == 4096)
        );
        assert!(voice.is_active());
    }

    #[test]
    fn psx_spu_voice_key_off_fades_and_stops_looped_sample() {
        let ram = adpcm_block(0x00, 0x03, 0x22).to_vec();
        let mut voice = PsxSpuVoice::new(ram);
        let mut output = [0i16; SPU_ADPCM_SAMPLES_PER_BLOCK];

        voice.key_on(0);
        voice.key_off();
        voice.render_samples(&mut output);

        assert!(output[0] > 0, "{output:?}");
        assert!(output[1] <= output[0], "{output:?}");
        assert!(output.contains(&0), "{output:?}");
        assert!(!voice.is_active());
    }
}
