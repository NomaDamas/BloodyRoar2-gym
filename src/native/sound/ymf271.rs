use crate::native::sound::queue::StereoSample;

pub const YMF271_MASTER_CLOCK_HZ: u32 = 16_934_400;
pub const YMF271_SAMPLE_RATE_HZ: u32 = YMF271_MASTER_CLOCK_HZ / 384;

const SLOT_COUNT: usize = 48;
const GROUP_COUNT: usize = 12;
const ENV_ATTACK: u8 = 0;
const ENV_DECAY1: u8 = 1;
const ENV_DECAY2: u8 = 2;
const ENV_RELEASE: u8 = 3;
const ENV_VOLUME_SHIFT: i32 = 16;
const MAX_I16_AS_I32: i32 = i16::MAX as i32;
const SLOT_MIX_HEADROOM_SHIFT: u32 = 2;
const OUTPUT_SOFT_KNEE: i32 = 24_000;
const OUTPUT_SOFT_LIMIT: i32 = 30_000;
const KEY_OFF_MIN_RELEASE_FRAMES: i32 = (YMF271_SAMPLE_RATE_HZ as i32) / 8;
const KEY_PROGRAM_SIGNATURE_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const KEY_PROGRAM_SIGNATURE_PRIME: u64 = 0x0000_0100_0000_01b3;

const FM_TAB: [i8; 16] = [0, 1, 2, -1, 3, 4, 5, -1, 6, 7, 8, -1, 9, 10, 11, -1];
const PCM_TAB: [i8; 16] = [0, 4, 8, -1, 12, 16, 20, -1, 24, 28, 32, -1, 36, 40, 44, -1];
const MULTIPLE_TABLE: [f64; 16] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
];
const POW_TABLE: [f64; 16] = [
    128.0, 256.0, 512.0, 1024.0, 2048.0, 4096.0, 8192.0, 16384.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0,
    32.0, 64.0,
];
const FS_FREQUENCY: [f64; 4] = [1.0, 0.5, 0.25, 0.125];
const CHANNEL_ATTENUATION_DB: [f64; 16] = [
    0.0, 2.5, 6.0, 8.5, 12.0, 14.5, 18.1, 20.6, 24.1, 26.6, 30.1, 32.6, 36.1, 96.1, 96.1, 96.1,
];

// MAME src/devices/sound/ymf271.cpp is BSD-3-Clause. The Rust code below is a
// clean port of the public register decode, PCM addressing, loop/end status,
// attenuation and sample-step formulas. It intentionally does not copy long
// lookup tables verbatim; those tables are regenerated from the same formulas.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Ymf271Stats {
    pub register_writes: u64,
    pub fm_writes: u64,
    pub pcm_writes: u64,
    pub timer_writes: u64,
    pub timer_a_expirations: u64,
    pub timer_b_expirations: u64,
    pub invalid_fm_writes: u64,
    pub invalid_pcm_writes: u64,
    pub key_register_writes: u64,
    pub last_key_register_data: u8,
    pub key_on_events: u64,
    pub key_off_events: u64,
    pub pcm_loop_events: u64,
    pub pcm_end_events: u64,
    pub pcm_invalid_loop_events: u64,
    pub pcm_stalled_events: u64,
    pub generated_frames: u64,
    pub nonzero_frames: u64,
    pub active_slots: u8,
    pub pcm_slots_active: u8,
    pub fm_slots_active: u8,
    pub fm_groups_skipped: u64,
    pub limited_frames: u64,
    pub peak_abs_left: u32,
    pub peak_abs_right: u32,
    pub last_rms_left: u32,
    pub last_rms_right: u32,
    pub irq_state: u8,
    pub status: u8,
    pub end_status: u16,
}

impl Ymf271Stats {
    pub fn json(self) -> String {
        format!(
            "{{\"sample_rate_hz\":{},\"register_writes\":{},\"fm_writes\":{},\"pcm_writes\":{},\"timer_writes\":{},\"timer_a_expirations\":{},\"timer_b_expirations\":{},\"invalid_fm_writes\":{},\"invalid_pcm_writes\":{},\"key_register_writes\":{},\"last_key_register_data\":{},\"last_key_register_data_hex\":\"0x{:02x}\",\"key_on_events\":{},\"key_off_events\":{},\"pcm_loop_events\":{},\"pcm_end_events\":{},\"pcm_invalid_loop_events\":{},\"pcm_stalled_events\":{},\"generated_frames\":{},\"nonzero_frames\":{},\"active_slots\":{},\"pcm_slots_active\":{},\"fm_slots_active\":{},\"fm_groups_skipped\":{},\"limited_frames\":{},\"peak_abs_left\":{},\"peak_abs_right\":{},\"last_rms_left\":{},\"last_rms_right\":{},\"irq_state\":{},\"irq_state_hex\":\"0x{:02x}\",\"status\":{},\"status_hex\":\"0x{:02x}\",\"end_status\":{},\"end_status_hex\":\"0x{:04x}\"}}",
            YMF271_SAMPLE_RATE_HZ,
            self.register_writes,
            self.fm_writes,
            self.pcm_writes,
            self.timer_writes,
            self.timer_a_expirations,
            self.timer_b_expirations,
            self.invalid_fm_writes,
            self.invalid_pcm_writes,
            self.key_register_writes,
            self.last_key_register_data,
            self.last_key_register_data,
            self.key_on_events,
            self.key_off_events,
            self.pcm_loop_events,
            self.pcm_end_events,
            self.pcm_invalid_loop_events,
            self.pcm_stalled_events,
            self.generated_frames,
            self.nonzero_frames,
            self.active_slots,
            self.pcm_slots_active,
            self.fm_slots_active,
            self.fm_groups_skipped,
            self.limited_frames,
            self.peak_abs_left,
            self.peak_abs_right,
            self.last_rms_left,
            self.last_rms_right,
            self.irq_state,
            self.irq_state,
            self.status,
            self.status,
            self.end_status,
            self.end_status
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    ext_en: u8,
    ext_out: u8,
    lfo_freq: u8,
    lfo_wave: u8,
    pms: u8,
    ams: u8,
    detune: u8,
    multiple: u8,
    total_level: u8,
    keyscale: u8,
    ar: u8,
    decay1_rate: u8,
    decay2_rate: u8,
    decay1_level: u8,
    release_rate: u8,
    block: u8,
    fns_hi: u8,
    fns: u32,
    feedback: u8,
    waveform: u8,
    accon: u8,
    algorithm: u8,
    ch_level: [u8; 4],
    start_addr: u32,
    loop_addr: u32,
    end_addr: u32,
    alt_loop: u8,
    fs: u8,
    src_note: u8,
    src_b: u8,
    step: u32,
    step_ptr: u64,
    active: bool,
    key_on_latched: bool,
    program_dirty_since_key_on: bool,
    last_key_on_program_signature: u64,
    last_key_on_frame: u64,
    ended_frame: Option<u64>,
    bits: u8,
    volume: i32,
    env_state: u8,
    env_attack_step: i32,
    env_decay1_step: i32,
    env_decay2_step: i32,
    env_release_step: i32,
    lfo_phase_mod: f64,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            ext_en: 0,
            ext_out: 0,
            lfo_freq: 0,
            lfo_wave: 0,
            pms: 0,
            ams: 0,
            detune: 0,
            multiple: 0,
            total_level: 0,
            keyscale: 0,
            ar: 0,
            decay1_rate: 0,
            decay2_rate: 0,
            decay1_level: 0,
            release_rate: 0,
            block: 0,
            fns_hi: 0,
            fns: 0,
            feedback: 0,
            waveform: 0,
            accon: 0,
            algorithm: 0,
            ch_level: [0; 4],
            start_addr: 0,
            loop_addr: 0,
            end_addr: 0,
            alt_loop: 0,
            fs: 0,
            src_note: 0,
            src_b: 0,
            step: 0,
            step_ptr: 0,
            active: false,
            key_on_latched: false,
            program_dirty_since_key_on: false,
            last_key_on_program_signature: 0,
            last_key_on_frame: 0,
            ended_frame: None,
            bits: 8,
            volume: 0,
            env_state: ENV_RELEASE,
            env_attack_step: 0,
            env_decay1_step: 0,
            env_decay2_step: 0,
            env_release_step: 0,
            lfo_phase_mod: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Group {
    sync: u8,
    pfm: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PcmStopReason {
    End,
    InvalidLoop,
    Stalled,
}

#[derive(Clone, Debug)]
pub struct Ymf271 {
    sample_rom: Vec<u8>,
    slots: [Slot; SLOT_COUNT],
    groups: [Group; GROUP_COUNT],
    main_regs: [u8; 0x10],
    timer_a: u32,
    timer_b: u32,
    timer_a_remaining_frames: Option<u64>,
    timer_b_remaining_frames: Option<u64>,
    irq_state: u8,
    status: u8,
    end_status: u16,
    enable: u8,
    ext_address: u32,
    ext_rw: u8,
    ext_read_latch: u8,
    attenuation_lut: [i64; 16],
    total_level_lut: [i64; 128],
    env_volume_lut: [i64; 256],
    stats: Ymf271Stats,
}

impl Ymf271 {
    pub fn new(sample_rom: Vec<u8>) -> Self {
        let mut ymf = Self {
            sample_rom,
            slots: [Slot::default(); SLOT_COUNT],
            groups: [Group::default(); GROUP_COUNT],
            main_regs: [0; 0x10],
            timer_a: 0,
            timer_b: 0,
            timer_a_remaining_frames: None,
            timer_b_remaining_frames: None,
            irq_state: 0,
            status: 0,
            end_status: 0,
            enable: 0,
            ext_address: 0,
            ext_rw: 0,
            ext_read_latch: 0,
            attenuation_lut: [0; 16],
            total_level_lut: [0; 128],
            env_volume_lut: [0; 256],
            stats: Ymf271Stats::default(),
        };
        ymf.init_tables();
        ymf
    }

    pub fn sample_rom_len(&self) -> usize {
        self.sample_rom.len()
    }

    pub fn sample_rom(&self) -> &[u8] {
        &self.sample_rom
    }

    pub fn read(&mut self, offset: u32) -> u8 {
        match offset & 0x0f {
            0x0 => self.status | ((self.end_status as u8 & 0x0f) << 3),
            0x1 => (self.end_status >> 4) as u8,
            0x2 => {
                if self.ext_rw == 0 {
                    return 0xff;
                }
                let ret = self.ext_read_latch;
                self.ext_address = self.ext_address.wrapping_add(1) & 0x7f_ffff;
                self.ext_read_latch = self.read_rom_byte(self.ext_address);
                ret
            }
            _ => 0xff,
        }
    }

    pub fn write(&mut self, offset: u32, data: u8) {
        let offset = (offset & 0x0f) as usize;
        self.main_regs[offset] = data;
        self.stats.register_writes = self.stats.register_writes.saturating_add(1);

        match offset {
            0x0 | 0x2 | 0x4 | 0x6 | 0x8 | 0x0c => {}
            0x1 => self.write_fm(0, self.main_regs[0x0], data),
            0x3 => self.write_fm(1, self.main_regs[0x2], data),
            0x5 => self.write_fm(2, self.main_regs[0x4], data),
            0x7 => self.write_fm(3, self.main_regs[0x6], data),
            0x9 => self.write_pcm(self.main_regs[0x8], data),
            0x0d => self.write_timer(self.main_regs[0x0c], data),
            _ => {}
        }
    }

    pub fn render_stereo(&mut self, frames: usize) -> Vec<StereoSample> {
        let mut rendered = Vec::with_capacity(frames);
        self.render_stereo_into(frames, &mut rendered);
        rendered
    }

    pub fn render_stereo_into(&mut self, frames: usize, rendered: &mut Vec<StereoSample>) {
        self.advance_timers(frames as u64);
        rendered.reserve(frames);
        let mut sum_sq_l = 0u64;
        let mut sum_sq_r = 0u64;
        let mut nonzero = 0u64;
        let mut limited = 0u64;
        let mut peak_abs_l = 0u32;
        let mut peak_abs_r = 0u32;
        let fm_groups_skipped = 0u64;

        for _ in 0..frames {
            let mut mix = [0i32; 4];
            for group_index in 0..GROUP_COUNT {
                match self.groups[group_index].sync {
                    2 => {
                        self.mix_fm_slot(group_index, &mut mix);
                        self.mix_fm_slot(group_index + 12, &mut mix);
                        self.mix_fm_slot(group_index + 24, &mut mix);
                        let slot = group_index + 36;
                        self.mix_pcm_slot(slot, &mut mix);
                    }
                    3 => {
                        self.mix_pcm_slot(group_index, &mut mix);
                        self.mix_pcm_slot(group_index + 12, &mut mix);
                        self.mix_pcm_slot(group_index + 24, &mut mix);
                        self.mix_pcm_slot(group_index + 36, &mut mix);
                    }
                    _ => {
                        if self.group_has_active_fm(group_index) {
                            self.mix_fm_group(group_index, &mut mix);
                        }
                    }
                }
            }

            let (left, left_limited) = soft_limit_i16(mix[0].saturating_add(mix[2]));
            let (right, right_limited) = soft_limit_i16(mix[1].saturating_add(mix[3]));
            if left_limited || right_limited {
                limited = limited.saturating_add(1);
            }
            if left != 0 || right != 0 {
                nonzero = nonzero.saturating_add(1);
            }
            let abs_l = (left as i32).unsigned_abs();
            let abs_r = (right as i32).unsigned_abs();
            peak_abs_l = peak_abs_l.max(abs_l);
            peak_abs_r = peak_abs_r.max(abs_r);
            sum_sq_l = sum_sq_l.saturating_add(u64::from(abs_l).saturating_mul(u64::from(abs_l)));
            sum_sq_r = sum_sq_r.saturating_add(u64::from(abs_r).saturating_mul(u64::from(abs_r)));
            rendered.push(StereoSample { left, right });
        }

        self.stats.generated_frames = self.stats.generated_frames.saturating_add(frames as u64);
        self.stats.nonzero_frames = self.stats.nonzero_frames.saturating_add(nonzero);
        self.stats.fm_groups_skipped = self
            .stats
            .fm_groups_skipped
            .saturating_add(fm_groups_skipped);
        self.stats.limited_frames = self.stats.limited_frames.saturating_add(limited);
        self.stats.peak_abs_left = peak_abs_l;
        self.stats.peak_abs_right = peak_abs_r;
        if frames > 0 {
            let frame_count = u64::try_from(frames).unwrap_or(u64::MAX);
            self.stats.last_rms_left = integer_sqrt(sum_sq_l / frame_count) as u32;
            self.stats.last_rms_right = integer_sqrt(sum_sq_r / frame_count) as u32;
        }
        self.refresh_active_slot_stats();
    }

    pub fn stats(&self) -> Ymf271Stats {
        let mut stats = self.stats;
        stats.irq_state = self.irq_state;
        stats.status = self.status;
        stats.end_status = self.end_status;
        stats
    }

    pub fn stats_json(&self) -> String {
        self.stats().json()
    }

    pub fn irq_pending(&self) -> bool {
        self.irq_state != 0
    }

    pub fn irq_state(&self) -> u8 {
        self.irq_state
    }

    pub fn timers_running(&self) -> bool {
        self.timer_a_remaining_frames.is_some() || self.timer_b_remaining_frames.is_some()
    }

    fn init_tables(&mut self) {
        for (index, db) in CHANNEL_ATTENUATION_DB.iter().copied().enumerate() {
            self.attenuation_lut[index] = db_to_linear(db);
        }
        for i in 0..128 {
            self.total_level_lut[i] = db_to_linear(0.75 * i as f64);
        }
        for i in 0..256 {
            self.env_volume_lut[i] = db_to_linear((i as f64) / (256.0 / 96.0));
        }
    }

    fn write_fm(&mut self, bank: usize, address: u8, data: u8) {
        self.stats.fm_writes = self.stats.fm_writes.saturating_add(1);
        let group = FM_TAB[(address & 0x0f) as usize];
        if group < 0 {
            self.stats.invalid_fm_writes = self.stats.invalid_fm_writes.saturating_add(1);
            return;
        }
        let group = group as usize;
        let reg = (address >> 4) & 0x0f;

        let sync_reg = matches!(reg, 0 | 9 | 10 | 12 | 13 | 14);
        let sync_mode = match self.groups[group].sync {
            0 => bank == 0,
            1 => bank == 0 || bank == 1,
            2 => bank == 0,
            _ => false,
        };

        if sync_mode && sync_reg {
            match self.groups[group].sync {
                0 => {
                    for slot_bank in 0..4 {
                        self.write_register(slot_bank * 12 + group, reg, data);
                    }
                }
                1 => {
                    let banks = if bank == 0 { [0, 2] } else { [1, 3] };
                    for slot_bank in banks {
                        self.write_register(slot_bank * 12 + group, reg, data);
                    }
                }
                2 => {
                    for slot_bank in 0..3 {
                        self.write_register(slot_bank * 12 + group, reg, data);
                    }
                }
                _ => {}
            }
            return;
        }

        self.write_register(bank * 12 + group, reg, data);
    }

    fn write_pcm(&mut self, address: u8, data: u8) {
        self.stats.pcm_writes = self.stats.pcm_writes.saturating_add(1);
        let slot = PCM_TAB[(address & 0x0f) as usize];
        if slot < 0 {
            self.stats.invalid_pcm_writes = self.stats.invalid_pcm_writes.saturating_add(1);
            return;
        }
        let slot = &mut self.slots[slot as usize];
        let mut changed = false;

        match (address >> 4) & 0x0f {
            0x0 => {
                let value = (slot.start_addr & !0x0000ff) | u32::from(data);
                changed |= set_if_changed(&mut slot.start_addr, value);
            }
            0x1 => {
                let value = (slot.start_addr & !0x00ff00) | (u32::from(data) << 8);
                changed |= set_if_changed(&mut slot.start_addr, value);
            }
            0x2 => {
                let value = (slot.start_addr & !0x7f0000) | (u32::from(data & 0x7f) << 16);
                changed |= set_if_changed(&mut slot.start_addr, value);
                changed |= set_if_changed(&mut slot.alt_loop, u8::from(data & 0x80 != 0));
            }
            0x3 => {
                let value = (slot.end_addr & !0x0000ff) | u32::from(data);
                changed |= set_if_changed(&mut slot.end_addr, value);
            }
            0x4 => {
                let value = (slot.end_addr & !0x00ff00) | (u32::from(data) << 8);
                changed |= set_if_changed(&mut slot.end_addr, value);
            }
            0x5 => {
                let value = (slot.end_addr & !0x7f0000) | (u32::from(data & 0x7f) << 16);
                changed |= set_if_changed(&mut slot.end_addr, value);
            }
            0x6 => {
                let value = (slot.loop_addr & !0x0000ff) | u32::from(data);
                changed |= set_if_changed(&mut slot.loop_addr, value);
            }
            0x7 => {
                let value = (slot.loop_addr & !0x00ff00) | (u32::from(data) << 8);
                changed |= set_if_changed(&mut slot.loop_addr, value);
            }
            0x8 => {
                let value = (slot.loop_addr & !0x7f0000) | (u32::from(data & 0x7f) << 16);
                changed |= set_if_changed(&mut slot.loop_addr, value);
            }
            0x9 => {
                let fs_changed = set_if_changed(&mut slot.fs, data & 0x03);
                let bits_changed =
                    set_if_changed(&mut slot.bits, if data & 0x04 != 0 { 12 } else { 8 });
                let src_note_changed = set_if_changed(&mut slot.src_note, (data >> 3) & 0x03);
                let src_b_changed = set_if_changed(&mut slot.src_b, (data >> 5) & 0x07);
                changed |= fs_changed || bits_changed || src_note_changed || src_b_changed;
                if changed {
                    calculate_step(slot);
                }
            }
            _ => {}
        }
        if changed {
            slot.program_dirty_since_key_on = true;
        }
    }

    fn write_timer(&mut self, address: u8, data: u8) {
        self.stats.timer_writes = self.stats.timer_writes.saturating_add(1);
        if address & 0xf0 == 0 {
            let group = FM_TAB[(address & 0x0f) as usize];
            if group >= 0 {
                let group = &mut self.groups[group as usize];
                group.sync = data & 0x03;
                group.pfm = data >> 7;
            }
            return;
        }

        match address {
            0x10 => self.timer_a = (self.timer_a & 0x003) | (u32::from(data) << 2),
            0x11 => self.timer_a = (self.timer_a & 0x3fc) | u32::from(data & 0x03),
            0x12 => self.timer_b = u32::from(data),
            0x13 => {
                if data & 0x01 == 0 {
                    self.timer_a_remaining_frames = None;
                } else if self.enable & 0x01 == 0 {
                    self.timer_a_remaining_frames = Some(self.timer_a_period_frames());
                }
                if data & 0x02 == 0 {
                    self.timer_b_remaining_frames = None;
                } else if self.enable & 0x02 == 0 {
                    self.timer_b_remaining_frames = Some(self.timer_b_period_frames());
                }
                if data & 0x10 != 0 {
                    self.irq_state &= !1;
                    self.status &= !1;
                }
                if data & 0x20 != 0 {
                    self.irq_state &= !2;
                    self.status &= !2;
                }
                self.enable = data;
            }
            0x14 => self.ext_address = (self.ext_address & !0x0000ff) | u32::from(data),
            0x15 => self.ext_address = (self.ext_address & !0x00ff00) | (u32::from(data) << 8),
            0x16 => {
                self.ext_address = (self.ext_address & !0x7f0000) | (u32::from(data & 0x7f) << 16);
                self.ext_rw = u8::from(data & 0x80 != 0);
            }
            0x17 => {
                self.ext_address = self.ext_address.wrapping_add(1) & 0x7f_ffff;
            }
            _ => {}
        }
    }

    fn advance_timers(&mut self, frames: u64) {
        let timer_a_period = self.timer_a_period_frames();
        advance_timer(
            frames,
            &mut self.timer_a_remaining_frames,
            timer_a_period,
            &mut self.status,
            0x01,
            &mut self.irq_state,
            self.enable & 0x04 != 0,
            &mut self.stats.timer_a_expirations,
        );

        let timer_b_period = self.timer_b_period_frames();
        advance_timer(
            frames,
            &mut self.timer_b_remaining_frames,
            timer_b_period,
            &mut self.status,
            0x02,
            &mut self.irq_state,
            self.enable & 0x08 != 0,
            &mut self.stats.timer_b_expirations,
        );
    }

    fn timer_a_period_frames(&self) -> u64 {
        u64::from(1024u32.saturating_sub(self.timer_a.min(1023))).max(1)
    }

    fn timer_b_period_frames(&self) -> u64 {
        u64::from(256u32.saturating_sub(self.timer_b.min(255)))
            .saturating_mul(16)
            .max(1)
    }

    fn write_register(&mut self, slot_index: usize, reg: u8, data: u8) {
        if slot_index >= self.slots.len() {
            return;
        }
        let slot = &mut self.slots[slot_index];
        match reg {
            0x0 => {
                self.stats.key_register_writes = self.stats.key_register_writes.saturating_add(1);
                self.stats.last_key_register_data = data;
                slot.ext_en = u8::from(data & 0x80 != 0);
                slot.ext_out = (data >> 3) & 0x0f;
                if data & 1 != 0 {
                    if should_restart_slot_on_key_on(slot, self.stats.generated_frames) {
                        let program_signature = slot_program_signature(slot);
                        slot.key_on_latched = true;
                        slot.program_dirty_since_key_on = false;
                        slot.last_key_on_program_signature = program_signature;
                        slot.last_key_on_frame = self.stats.generated_frames;
                        slot.ended_frame = None;
                        slot.step = 0;
                        slot.step_ptr = 0;
                        slot.active = true;
                        calculate_step(slot);
                        calculate_status_end(&mut self.end_status, slot_index, false);
                        init_envelope(slot);
                        init_lfo(slot);
                        self.stats.key_on_events = self.stats.key_on_events.saturating_add(1);
                    }
                } else {
                    slot.key_on_latched = false;
                    slot.ended_frame = None;
                    if slot.active {
                        slot.env_state = ENV_RELEASE;
                        if slot.env_release_step <= 0 {
                            slot.env_release_step = key_off_min_release_step(slot.volume);
                        }
                        self.stats.key_off_events = self.stats.key_off_events.saturating_add(1);
                    }
                }
            }
            0x1 => {
                let changed = set_if_changed(&mut slot.lfo_freq, data);
                mark_program_dirty(slot, changed);
            }
            0x2 => {
                let mut changed = set_if_changed(&mut slot.lfo_wave, data & 0x03);
                changed |= set_if_changed(&mut slot.pms, (data >> 3) & 0x07);
                changed |= set_if_changed(&mut slot.ams, (data >> 6) & 0x03);
                mark_program_dirty(slot, changed);
            }
            0x3 => {
                let mut changed = set_if_changed(&mut slot.multiple, data & 0x0f);
                changed |= set_if_changed(&mut slot.detune, (data >> 4) & 0x07);
                if changed {
                    mark_program_dirty(slot, true);
                    calculate_step(slot);
                }
            }
            0x4 => {
                let changed = set_if_changed(&mut slot.total_level, data & 0x7f);
                mark_program_dirty(slot, changed);
            }
            0x5 => {
                let mut changed = set_if_changed(&mut slot.ar, data & 0x1f);
                changed |= set_if_changed(&mut slot.keyscale, (data >> 5) & 0x07);
                mark_program_dirty(slot, changed);
            }
            0x6 => {
                let changed = set_if_changed(&mut slot.decay1_rate, data & 0x1f);
                mark_program_dirty(slot, changed);
            }
            0x7 => {
                let changed = set_if_changed(&mut slot.decay2_rate, data & 0x1f);
                mark_program_dirty(slot, changed);
            }
            0x8 => {
                let mut changed = set_if_changed(&mut slot.release_rate, data & 0x0f);
                changed |= set_if_changed(&mut slot.decay1_level, (data >> 4) & 0x0f);
                mark_program_dirty(slot, changed);
            }
            0x9 => {
                let fns = ((u32::from(slot.fns_hi) << 8) & 0x0f00) | u32::from(data);
                let block = (slot.fns_hi >> 4) & 0x0f;
                let mut changed = set_if_changed(&mut slot.fns, fns);
                changed |= set_if_changed(&mut slot.block, block);
                if changed {
                    mark_program_dirty(slot, true);
                    calculate_step(slot);
                }
            }
            0x0a => {
                let fns_hi_changed = set_if_changed(&mut slot.fns_hi, data);
                let fns = ((u32::from(slot.fns_hi) << 8) & 0x0f00) | (slot.fns & 0xff);
                let block = (slot.fns_hi >> 4) & 0x0f;
                let mut changed = fns_hi_changed;
                changed |= set_if_changed(&mut slot.fns, fns);
                changed |= set_if_changed(&mut slot.block, block);
                if changed {
                    mark_program_dirty(slot, true);
                    calculate_step(slot);
                }
            }
            0x0b => {
                let mut changed = set_if_changed(&mut slot.waveform, data & 0x07);
                changed |= set_if_changed(&mut slot.feedback, (data >> 4) & 0x07);
                changed |= set_if_changed(&mut slot.accon, u8::from(data & 0x80 != 0));
                if changed {
                    mark_program_dirty(slot, true);
                    calculate_step(slot);
                }
            }
            0x0c => {
                let changed = set_if_changed(&mut slot.algorithm, data & 0x0f);
                mark_program_dirty(slot, changed);
            }
            0x0d => {
                let mut changed = set_if_changed(&mut slot.ch_level[0], data >> 4);
                changed |= set_if_changed(&mut slot.ch_level[1], data & 0x0f);
                mark_program_dirty(slot, changed);
            }
            0x0e => {
                let mut changed = set_if_changed(&mut slot.ch_level[2], data >> 4);
                changed |= set_if_changed(&mut slot.ch_level[3], data & 0x0f);
                mark_program_dirty(slot, changed);
            }
            _ => {}
        }
        self.refresh_active_slot_stats();
    }

    fn mix_pcm_slot(&mut self, slot_index: usize, mix: &mut [i32; 4]) {
        if slot_index >= self.slots.len() || !self.slots[slot_index].active {
            return;
        }
        if self.slots[slot_index].waveform != 7 {
            return;
        }
        if !self.prepare_pcm_slot_for_sample(slot_index) {
            return;
        }

        let sample = self.current_pcm_sample(slot_index);
        update_envelope(&mut self.slots[slot_index]);
        if !self.slots[slot_index].active {
            self.finish_envelope_ended_slot(slot_index);
            return;
        }

        let final_volume = self.calculate_slot_volume(slot_index);
        let channel_levels = self.slots[slot_index].ch_level;

        for channel in 0..4 {
            let mut channel_volume =
                (final_volume * self.attenuation_lut[channel_levels[channel] as usize]) >> 16;
            if channel_volume > 65_536 {
                channel_volume = 65_536;
            }
            mix[channel] = mix[channel].saturating_add(mix_slot_sample(sample, channel_volume));
        }

        self.slots[slot_index].step_ptr = self.slots[slot_index]
            .step_ptr
            .wrapping_add(u64::from(self.slots[slot_index].step));
        self.handle_pcm_end(slot_index);
    }

    fn mix_fm_group(&mut self, group_index: usize, mix: &mut [i32; 4]) {
        for base in [0usize, 12, 24, 36] {
            self.mix_fm_slot(base + group_index, mix);
        }
    }

    fn mix_fm_slot(&mut self, slot_index: usize, mix: &mut [i32; 4]) {
        if slot_index >= self.slots.len() || !self.slots[slot_index].active {
            return;
        }
        if self.slots[slot_index].waveform == 7 {
            return;
        }

        let sample = self.current_fm_sample(slot_index);
        let final_volume = self.calculate_slot_volume(slot_index);
        let channel_levels = self.slots[slot_index].ch_level;

        for channel in 0..4 {
            let mut channel_volume =
                (final_volume * self.attenuation_lut[channel_levels[channel] as usize]) >> 16;
            if channel_volume > 65_536 {
                channel_volume = 65_536;
            }
            mix[channel] = mix[channel].saturating_add(mix_slot_sample(sample, channel_volume));
        }

        update_envelope(&mut self.slots[slot_index]);
        if !self.slots[slot_index].active {
            self.finish_envelope_ended_slot(slot_index);
            return;
        }
        self.slots[slot_index].step_ptr = self.slots[slot_index]
            .step_ptr
            .wrapping_add(u64::from(self.slots[slot_index].step));
    }

    fn current_pcm_sample(&self, slot_index: usize) -> i16 {
        let slot = self.slots[slot_index];
        if slot.bits == 8 {
            return i16::from(
                self.read_rom_byte(slot.start_addr + (slot.step_ptr >> 16) as u32) as i8,
            ) << 8;
        }

        let frame_index = slot.step_ptr >> 17;
        let base = slot.start_addr + (frame_index as u32).saturating_mul(3);
        let raw = if slot.step_ptr & 0x1_0000 != 0 {
            (u16::from(self.read_rom_byte(base + 2)) << 8)
                | ((u16::from(self.read_rom_byte(base + 1)) << 4) & 0x00f0)
        } else {
            (u16::from(self.read_rom_byte(base)) << 8)
                | (u16::from(self.read_rom_byte(base + 1)) & 0x00f0)
        };
        raw as i16
    }

    fn current_fm_sample(&self, slot_index: usize) -> i16 {
        let slot = self.slots[slot_index];
        let phase = ((slot.step_ptr >> 16) & 0xffff) as u32;
        let sample = match slot.waveform & 0x07 {
            0 => triangle_wave_sample(phase),
            1 => saw_wave_sample(phase),
            2 => square_wave_sample(phase),
            3 => -triangle_wave_sample(phase),
            4 => triangle_wave_sample(phase).unsigned_abs() as i32 - 16_384,
            5 => saw_wave_sample(phase).unsigned_abs() as i32 - 16_384,
            6 => {
                if phase < 0x4000 || (0x8000..0xc000).contains(&phase) {
                    24_576
                } else {
                    -24_576
                }
            }
            _ => 0,
        };
        sample.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
    }

    fn prepare_pcm_slot_for_sample(&mut self, slot_index: usize) -> bool {
        if self.slots[slot_index].step == 0 {
            self.stop_pcm_slot(slot_index, PcmStopReason::Stalled);
            return false;
        }

        self.handle_pcm_end(slot_index);
        if !self.slots[slot_index].active {
            return false;
        }

        if !self.pcm_current_sample_available(slot_index) {
            self.stop_pcm_slot(slot_index, PcmStopReason::InvalidLoop);
            return false;
        }

        true
    }

    fn handle_pcm_end(&mut self, slot_index: usize) {
        let sample_offset = self.slots[slot_index].step_ptr >> 16;
        let end_addr = u64::from(self.slots[slot_index].end_addr);
        if sample_offset <= end_addr {
            return;
        }

        if !self.pcm_loop_range_valid(slot_index) {
            let reason = if self.slots[slot_index].alt_loop == 0 {
                PcmStopReason::End
            } else {
                PcmStopReason::InvalidLoop
            };
            self.stop_pcm_slot(slot_index, reason);
            return;
        }

        let slot = &mut self.slots[slot_index];
        let fractional_step = slot.step_ptr & 0xffff;
        let loop_addr = u64::from(slot.loop_addr);
        let loop_span = end_addr.saturating_sub(loop_addr).saturating_add(1);
        let overshoot = sample_offset.saturating_sub(end_addr.saturating_add(1));
        let wrapped_sample = loop_addr.saturating_add(overshoot % loop_span.max(1));
        slot.step_ptr = (wrapped_sample << 16) | fractional_step;
        calculate_status_end(&mut self.end_status, slot_index, true);
        self.stats.pcm_loop_events = self.stats.pcm_loop_events.saturating_add(1);

        if !self.pcm_current_sample_available(slot_index) {
            self.slots[slot_index].step_ptr =
                (u64::from(self.slots[slot_index].end_addr) << 16) | fractional_step;
            self.stop_pcm_slot(slot_index, PcmStopReason::InvalidLoop);
        }
    }

    fn stop_pcm_slot(&mut self, slot_index: usize, reason: PcmStopReason) {
        if slot_index >= self.slots.len() {
            return;
        }
        let slot = &mut self.slots[slot_index];
        if !slot.active {
            return;
        }
        slot.active = false;
        slot.ended_frame = Some(self.stats.generated_frames);
        let fractional_step = slot.step_ptr & 0xffff;
        slot.step_ptr = (u64::from(slot.end_addr) << 16) | fractional_step;
        calculate_status_end(&mut self.end_status, slot_index, true);
        self.stats.pcm_end_events = self.stats.pcm_end_events.saturating_add(1);
        match reason {
            PcmStopReason::InvalidLoop => {
                self.stats.pcm_invalid_loop_events =
                    self.stats.pcm_invalid_loop_events.saturating_add(1);
            }
            PcmStopReason::Stalled => {
                self.stats.pcm_stalled_events = self.stats.pcm_stalled_events.saturating_add(1);
            }
            PcmStopReason::End => {}
        }
    }

    fn finish_envelope_ended_slot(&mut self, slot_index: usize) {
        if slot_index >= self.slots.len() || self.slots[slot_index].ended_frame.is_some() {
            return;
        }
        self.slots[slot_index].ended_frame = Some(self.stats.generated_frames);
        if self.slots[slot_index].waveform == 7 {
            calculate_status_end(&mut self.end_status, slot_index, true);
            self.stats.pcm_end_events = self.stats.pcm_end_events.saturating_add(1);
        }
    }

    fn pcm_loop_range_valid(&self, slot_index: usize) -> bool {
        let slot = self.slots[slot_index];
        slot.loop_addr < slot.end_addr
            && self.pcm_sample_offset_available(slot_index, slot.loop_addr)
            && self.pcm_sample_offset_available(slot_index, slot.end_addr)
    }

    fn pcm_current_sample_available(&self, slot_index: usize) -> bool {
        let slot = self.slots[slot_index];
        self.pcm_sample_offset_available(slot_index, (slot.step_ptr >> 16) as u32)
    }

    fn pcm_sample_offset_available(&self, slot_index: usize, sample_offset: u32) -> bool {
        if self.sample_rom.is_empty() {
            return false;
        }
        let slot = self.slots[slot_index];
        let (byte_offset, bytes_needed) = if slot.bits == 8 {
            (Some(sample_offset), 1usize)
        } else {
            let packed_index = sample_offset / 2;
            (
                packed_index.checked_mul(3),
                if sample_offset & 1 == 0 {
                    2usize
                } else {
                    3usize
                },
            )
        };
        let Some(byte_offset) = byte_offset else {
            return false;
        };
        let Some(address) = slot.start_addr.checked_add(byte_offset) else {
            return false;
        };
        let Some(end) = (address as usize).checked_add(bytes_needed) else {
            return false;
        };
        end <= self.sample_rom.len()
    }

    fn calculate_slot_volume(&self, slot_index: usize) -> i64 {
        let slot = self.slots[slot_index];
        let volume = (slot.volume >> ENV_VOLUME_SHIFT).clamp(0, 255) as usize;
        let env_volume = self.env_volume_lut[255usize.saturating_sub(volume)];
        (env_volume * self.total_level_lut[slot.total_level as usize]) >> 16
    }

    fn read_rom_byte(&self, address: u32) -> u8 {
        if self.sample_rom.is_empty() {
            return 0xff;
        }
        self.sample_rom
            .get(address as usize)
            .copied()
            .unwrap_or(0xff)
    }

    fn refresh_active_slot_stats(&mut self) {
        let mut active = 0u8;
        let mut pcm = 0u8;
        let mut fm = 0u8;
        for slot in &self.slots {
            if slot.active {
                active = active.saturating_add(1);
                if slot.waveform == 7 {
                    pcm = pcm.saturating_add(1);
                } else {
                    fm = fm.saturating_add(1);
                }
            }
        }
        self.stats.active_slots = active;
        self.stats.pcm_slots_active = pcm;
        self.stats.fm_slots_active = fm;
    }

    fn group_has_active_fm(&self, group_index: usize) -> bool {
        [0usize, 12, 24, 36].iter().any(|base| {
            let slot = self.slots[*base + group_index];
            slot.active && slot.waveform != 7
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn advance_timer(
    mut elapsed_frames: u64,
    remaining_frames: &mut Option<u64>,
    period_frames: u64,
    status: &mut u8,
    status_bit: u8,
    irq_state: &mut u8,
    irq_enabled: bool,
    expiration_count: &mut u64,
) {
    let Some(mut remaining) = *remaining_frames else {
        return;
    };

    while elapsed_frames >= remaining {
        elapsed_frames -= remaining;
        *status |= status_bit;
        if irq_enabled {
            *irq_state |= status_bit;
        }
        *expiration_count = expiration_count.saturating_add(1);
        remaining = period_frames.max(1);
    }
    *remaining_frames = Some(remaining.saturating_sub(elapsed_frames).max(1));
}

fn set_if_changed<T: Eq>(target: &mut T, value: T) -> bool {
    if *target == value {
        false
    } else {
        *target = value;
        true
    }
}

fn mark_program_dirty(slot: &mut Slot, changed: bool) {
    if changed {
        slot.program_dirty_since_key_on = true;
    }
}

fn should_restart_slot_on_key_on(slot: &Slot, _current_frame: u64) -> bool {
    if !slot.key_on_latched {
        return true;
    }
    if slot.active {
        return false;
    }
    if slot.program_dirty_since_key_on
        && slot_program_signature(slot) != slot.last_key_on_program_signature
    {
        return true;
    }
    false
}

fn slot_program_signature(slot: &Slot) -> u64 {
    let mut hash = KEY_PROGRAM_SIGNATURE_OFFSET_BASIS;
    hash = hash_signature_u32(hash, slot.start_addr);
    hash = hash_signature_u32(hash, slot.loop_addr);
    hash = hash_signature_u32(hash, slot.end_addr);
    hash = hash_signature_u32(hash, slot.fns);
    for byte in [
        slot.alt_loop,
        slot.fs,
        slot.src_note,
        slot.src_b,
        slot.block,
        slot.multiple,
        slot.waveform,
        slot.bits,
    ] {
        hash = hash_signature_u8(hash, byte);
    }
    hash
}

fn hash_signature_u32(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash = hash_signature_u8(hash, byte);
    }
    hash
}

fn hash_signature_u8(hash: u64, value: u8) -> u64 {
    (hash ^ u64::from(value)).wrapping_mul(KEY_PROGRAM_SIGNATURE_PRIME)
}

fn calculate_step(slot: &mut Slot) {
    let fns = if slot.waveform == 7 {
        slot.fns | 2048
    } else {
        slot.fns
    };
    let mut step = f64::from(2 * fns)
        * POW_TABLE[slot.block as usize]
        * MULTIPLE_TABLE[slot.multiple as usize]
        * slot.lfo_phase_mod;
    if slot.waveform == 7 {
        step *= FS_FREQUENCY[slot.fs as usize];
        step /= 524_288.0 / 65_536.0;
    } else {
        step *= 1024.0;
        step /= 536_870_912.0 / 65_536.0;
    }
    slot.step = step.max(0.0).min(u32::MAX as f64) as u32;
}

fn calculate_status_end(end_status: &mut u16, slot_index: usize, state: bool) {
    if slot_index & 3 != 0 {
        return;
    }
    let subbit = slot_index / 12;
    let bankbit = (slot_index % 12) >> 2;
    let mask = 1u16 << (subbit + bankbit * 4);
    if state {
        *end_status |= mask;
    } else {
        *end_status &= !mask;
    }
}

fn init_envelope(slot: &mut Slot) {
    let decay_level = 255 - (i32::from(slot.decay1_level) << 4);
    let attack_rate = i32::from(slot.ar) * 2;
    let decay1_rate = i32::from(slot.decay1_rate) * 2;
    let decay2_rate = i32::from(slot.decay2_rate) * 2;
    let release_rate = i32::from(slot.release_rate) * 4;

    slot.env_attack_step = envelope_step(attack_rate, 255);
    slot.env_decay1_step = envelope_step(decay1_rate, 255 - decay_level);
    slot.env_decay2_step = envelope_step(decay2_rate, 255);
    slot.env_release_step = envelope_step(release_rate, 255);
    slot.volume = (255 - 160) << ENV_VOLUME_SHIFT;
    slot.env_state = ENV_ATTACK;
}

fn init_lfo(slot: &mut Slot) {
    slot.lfo_phase_mod = 1.0;
    calculate_step(slot);
}

fn update_envelope(slot: &mut Slot) {
    match slot.env_state {
        ENV_ATTACK => {
            slot.volume = slot.volume.saturating_add(slot.env_attack_step);
            if slot.volume >= 255 << ENV_VOLUME_SHIFT {
                slot.volume = 255 << ENV_VOLUME_SHIFT;
                slot.env_state = ENV_DECAY1;
            }
        }
        ENV_DECAY1 => {
            let decay_level = 255 - (i32::from(slot.decay1_level) << 4);
            slot.volume = slot.volume.saturating_sub(slot.env_decay1_step);
            if !check_envelope_end(slot) && (slot.volume >> ENV_VOLUME_SHIFT) <= decay_level {
                slot.env_state = ENV_DECAY2;
            }
        }
        ENV_DECAY2 => {
            slot.volume = slot.volume.saturating_sub(slot.env_decay2_step);
            check_envelope_end(slot);
        }
        ENV_RELEASE => {
            slot.volume = slot.volume.saturating_sub(slot.env_release_step);
            check_envelope_end(slot);
        }
        _ => {}
    }
}

fn check_envelope_end(slot: &mut Slot) -> bool {
    if slot.volume <= 0 {
        slot.active = false;
        slot.volume = 0;
        return true;
    }
    false
}

fn envelope_step(rate: i32, span: i32) -> i32 {
    if rate < 4 {
        return 0;
    }
    let normalized = (rate.min(63) - 3) as f64 / 60.0;
    let samples = (YMF271_SAMPLE_RATE_HZ as f64 * (1.0 - normalized).powf(3.0) * 0.4).max(1.0);
    ((f64::from(span.max(0)) / samples) * 65_536.0) as i32
}

fn key_off_min_release_step(current_volume: i32) -> i32 {
    current_volume
        .max(1)
        .saturating_add(KEY_OFF_MIN_RELEASE_FRAMES - 1)
        / KEY_OFF_MIN_RELEASE_FRAMES
}

fn db_to_linear(db: f64) -> i64 {
    (65_536.0 / 10.0_f64.powf(db / 20.0)) as i64
}

fn mix_slot_sample(sample: i16, channel_volume: i64) -> i32 {
    (((i64::from(sample) * channel_volume) >> 16) >> SLOT_MIX_HEADROOM_SHIFT) as i32
}

fn soft_limit_i16(value: i32) -> (i16, bool) {
    let abs = value.unsigned_abs().min(MAX_I16_AS_I32 as u32) as i32;
    if abs <= OUTPUT_SOFT_KNEE {
        return (value as i16, false);
    }

    let extra = abs - OUTPUT_SOFT_KNEE;
    let knee_span = OUTPUT_SOFT_LIMIT - OUTPUT_SOFT_KNEE;
    let limited_abs = OUTPUT_SOFT_KNEE + (knee_span * extra) / (extra + knee_span);
    let limited_abs = limited_abs.min(MAX_I16_AS_I32 - 1);
    let limited = if value < 0 { -limited_abs } else { limited_abs };
    (limited as i16, true)
}

fn triangle_wave_sample(phase: u32) -> i32 {
    let phase = (phase & 0xffff) as i32;
    if phase < 0x8000 {
        phase.saturating_mul(2).saturating_sub(0x8000)
    } else {
        0x7fff - (phase - 0x8000).saturating_mul(2)
    }
}

fn saw_wave_sample(phase: u32) -> i32 {
    (phase & 0xffff) as i32 - 0x8000
}

fn square_wave_sample(phase: u32) -> i32 {
    if phase & 0x8000 == 0 { 0x7fff } else { -0x8000 }
}

fn integer_sqrt(value: u64) -> u64 {
    if value == 0 {
        return 0;
    }
    let mut x0 = value;
    let mut x1 = (x0 + value / x0) / 2;
    while x1 < x0 {
        x0 = x1;
        x1 = (x0 + value / x0) / 2;
    }
    x0
}

#[cfg(test)]
mod tests {
    use super::{
        ENV_VOLUME_SHIFT, GROUP_COUNT, KEY_OFF_MIN_RELEASE_FRAMES, OUTPUT_SOFT_LIMIT, SLOT_COUNT,
        YMF271_SAMPLE_RATE_HZ, Ymf271,
    };

    fn write_main(ymf: &mut Ymf271, address: u8, data: u8) {
        ymf.write(0x08, address);
        ymf.write(0x09, data);
    }

    fn write_fm_bank0(ymf: &mut Ymf271, address: u8, data: u8) {
        ymf.write(0x00, address);
        ymf.write(0x01, data);
    }

    fn write_timer(ymf: &mut Ymf271, address: u8, data: u8) {
        ymf.write(0x0c, address);
        ymf.write(0x0d, data);
    }

    fn assert_audible_without_i16_clip(
        frames: &[crate::native::sound::queue::StereoSample],
        ymf: &Ymf271,
    ) {
        assert!(
            frames
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0),
            "{:?}",
            ymf.stats()
        );
        assert!(
            frames.iter().all(|frame| {
                frame.left != i16::MIN
                    && frame.left != i16::MAX
                    && frame.right != i16::MIN
                    && frame.right != i16::MAX
            }),
            "{:?}",
            ymf.stats()
        );
        let stats = ymf.stats();
        assert!(stats.peak_abs_left <= OUTPUT_SOFT_LIMIT as u32, "{stats:?}");
        assert!(
            stats.peak_abs_right <= OUTPUT_SOFT_LIMIT as u32,
            "{stats:?}"
        );
        assert!(stats.last_rms_left < 32_000, "{stats:?}");
        assert!(stats.last_rms_right < 32_000, "{stats:?}");
    }

    fn activate_direct_looped_pcm_slot(ymf: &mut Ymf271, slot_index: usize) {
        let group = slot_index % GROUP_COUNT;
        ymf.groups[group].sync = 3;
        ymf.slots[slot_index].active = true;
        ymf.slots[slot_index].key_on_latched = true;
        ymf.slots[slot_index].waveform = 7;
        ymf.slots[slot_index].bits = 8;
        ymf.slots[slot_index].start_addr = 0;
        ymf.slots[slot_index].loop_addr = 0;
        ymf.slots[slot_index].end_addr = 1023;
        ymf.slots[slot_index].alt_loop = 1;
        ymf.slots[slot_index].step = 1 << 16;
        ymf.slots[slot_index].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[slot_index].env_state = super::ENV_DECAY2;
        ymf.slots[slot_index].env_decay2_step = 0;
        ymf.slots[slot_index].ch_level = [0, 0, 0, 0];
    }

    #[test]
    fn sample_rate_matches_raizing_ps9805_clock() {
        assert_eq!(YMF271_SAMPLE_RATE_HZ, 44_100);
    }

    #[test]
    fn timer_status_expires_reloads_and_resets() {
        let mut ymf = Ymf271::new(vec![0; 256]);
        assert!(!ymf.timers_running());
        write_timer(&mut ymf, 0x10, 0xff);
        write_timer(&mut ymf, 0x11, 0x03);
        write_timer(&mut ymf, 0x12, 0xff);
        write_timer(&mut ymf, 0x13, 0x03);
        assert!(ymf.timers_running());

        ymf.render_stereo(1);
        assert_eq!(ymf.read(0x00) & 0x03, 0x01);
        ymf.render_stereo(15);
        assert_eq!(ymf.read(0x00) & 0x03, 0x03);
        assert_eq!(ymf.stats().timer_a_expirations, 16);
        assert_eq!(ymf.stats().timer_b_expirations, 1);

        write_timer(&mut ymf, 0x13, 0x33);
        assert_eq!(ymf.read(0x00) & 0x03, 0);
        assert!(ymf.timers_running());

        write_timer(&mut ymf, 0x13, 0x00);
        assert!(!ymf.timers_running());
    }

    #[test]
    fn timer_irq_state_asserts_when_enabled_and_clears_with_control_bits() {
        let mut ymf = Ymf271::new(vec![0; 256]);
        write_timer(&mut ymf, 0x10, 0xff);
        write_timer(&mut ymf, 0x11, 0x03);
        write_timer(&mut ymf, 0x13, 0x05);

        ymf.render_stereo(1);
        assert!(ymf.irq_pending());
        assert_eq!(ymf.irq_state(), 0x01);
        assert_eq!(ymf.stats().irq_state, 0x01);

        write_timer(&mut ymf, 0x13, 0x10);
        assert!(!ymf.irq_pending());
        assert_eq!(ymf.irq_state(), 0);
        assert_eq!(ymf.read(0x00) & 0x01, 0);
    }

    #[test]
    fn pcm_registers_generate_nonzero_stereo_from_sample_rom() {
        let mut sample_rom = vec![0u8; 256];
        for (index, byte) in sample_rom.iter_mut().enumerate() {
            *byte = (index as u8).wrapping_mul(17).wrapping_add(0x20);
        }
        let mut ymf = Ymf271::new(sample_rom);

        write_timer(&mut ymf, 0x00, 0x03);
        write_main(&mut ymf, 0x00, 0x00);
        write_main(&mut ymf, 0x10, 0x00);
        write_main(&mut ymf, 0x20, 0x00);
        write_main(&mut ymf, 0x30, 0x20);
        write_main(&mut ymf, 0x40, 0x00);
        write_main(&mut ymf, 0x50, 0x00);
        write_main(&mut ymf, 0x60, 0x00);
        write_main(&mut ymf, 0x70, 0x00);
        write_main(&mut ymf, 0x80, 0x00);
        write_main(&mut ymf, 0x90, 0x00);
        write_fm_bank0(&mut ymf, 0x30, 0x01);
        write_fm_bank0(&mut ymf, 0x40, 0x00);
        write_fm_bank0(&mut ymf, 0x50, 0xff);
        write_fm_bank0(&mut ymf, 0x80, 0x0f);
        write_fm_bank0(&mut ymf, 0xa0, 0x40);
        write_fm_bank0(&mut ymf, 0x90, 0xff);
        write_fm_bank0(&mut ymf, 0xb0, 0x07);
        write_fm_bank0(&mut ymf, 0xd0, 0x00);
        write_fm_bank0(&mut ymf, 0x00, 0x01);

        let frames = ymf.render_stereo(128);
        assert!(
            frames
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0)
        );
        let stats = ymf.stats();
        assert!(stats.key_on_events >= 1, "{stats:?}");
        assert!(stats.last_rms_left > 0, "{stats:?}");
        assert!(stats.last_rms_right > 0, "{stats:?}");
    }

    #[test]
    fn repeated_key_on_write_does_not_restart_active_pcm_voice() {
        let mut ymf = Ymf271::new(vec![0x7f; 128]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 127;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        ymf.render_stereo(16);
        let step_ptr = ymf.slots[0].step_ptr;
        let volume = ymf.slots[0].volume;

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let duplicate = ymf.stats();

        assert_eq!(duplicate.key_register_writes, 2, "{duplicate:?}");
        assert_eq!(duplicate.key_on_events, 1, "{duplicate:?}");
        assert!(ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert_eq!(ymf.slots[0].step_ptr, step_ptr);
        assert_eq!(ymf.slots[0].volume, volume);

        write_fm_bank0(&mut ymf, 0x00, 0x00);
        assert!(!ymf.slots[0].key_on_latched);
        assert_eq!(ymf.slots[0].env_state, super::ENV_RELEASE);

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let retrigger = ymf.stats();
        assert_eq!(retrigger.key_on_events, 2, "{retrigger:?}");
        assert!(ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert_eq!(ymf.slots[0].step_ptr, 0);
        assert_eq!(ymf.slots[0].env_state, super::ENV_ATTACK);
    }

    #[test]
    fn pcm_rear_channels_are_folded_into_stereo_output() {
        let mut ymf = Ymf271::new(vec![0x7f; 128]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 64;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].ch_level = [15, 15, 0, 0];

        let frames = ymf.render_stereo(4);

        assert!(
            frames
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0),
            "{frames:?}"
        );
        assert!(ymf.stats().last_rms_left > 0, "{:?}", ymf.stats());
        assert!(ymf.stats().last_rms_right > 0, "{:?}", ymf.stats());
    }

    #[test]
    fn render_stereo_into_appends_without_reallocating_small_batches() {
        let mut ymf = Ymf271::new(vec![0x7f; 128]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 64;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let mut frames = Vec::with_capacity(64);
        let capacity = frames.capacity();
        frames.push(crate::native::sound::queue::StereoSample {
            left: 11,
            right: -11,
        });
        ymf.render_stereo_into(8, &mut frames);
        let stats = ymf.stats();

        assert_eq!(frames.len(), 9);
        assert_eq!(frames[0].left, 11);
        assert_eq!(frames[0].right, -11);
        assert_eq!(frames.capacity(), capacity);
        assert_eq!(stats.generated_frames, 8, "{stats:?}");
        assert!(stats.nonzero_frames > 0, "{stats:?}");
    }

    #[test]
    fn looped_pcm_voice_sustains_nonzero_stereo_for_attack_beast_duration() {
        let mut ymf = Ymf271::new(vec![0x7f; 4096]);
        activate_direct_looped_pcm_slot(&mut ymf, 0);

        let frame_count = YMF271_SAMPLE_RATE_HZ as usize * 3;
        let frames = ymf.render_stereo(frame_count);
        let stats = ymf.stats();

        assert_eq!(stats.generated_frames, frame_count as u64, "{stats:?}");
        assert_eq!(stats.nonzero_frames, frame_count as u64, "{stats:?}");
        assert!(stats.pcm_loop_events >= 100, "{stats:?}");
        assert_eq!(stats.pcm_end_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_stalled_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_slots_active, 1, "{stats:?}");
        assert_audible_without_i16_clip(&frames, &ymf);
    }

    #[test]
    fn fm_slots_generate_nonzero_stereo_instead_of_being_skipped() {
        let mut ymf = Ymf271::new(vec![0; 64]);

        write_fm_bank0(&mut ymf, 0x30, 0x01);
        write_fm_bank0(&mut ymf, 0x40, 0x00);
        write_fm_bank0(&mut ymf, 0x50, 0xff);
        write_fm_bank0(&mut ymf, 0x90, 0xff);
        write_fm_bank0(&mut ymf, 0xa0, 0x4f);
        write_fm_bank0(&mut ymf, 0xd0, 0x00);
        write_fm_bank0(&mut ymf, 0xe0, 0x00);
        write_fm_bank0(&mut ymf, 0x00, 0x01);

        let frames = ymf.render_stereo(512);
        let stats = ymf.stats();

        assert!(
            frames
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0),
            "{stats:?}"
        );
        assert!(stats.fm_slots_active > 0, "{stats:?}");
        assert_eq!(stats.fm_groups_skipped, 0, "{stats:?}");
    }

    #[test]
    fn extreme_pcm_mix_uses_headroom_instead_of_i16_clipping() {
        let mut ymf = Ymf271::new(vec![0x80; 128]);
        for group in 0..GROUP_COUNT {
            ymf.groups[group].sync = 3;
        }
        for slot in 0..SLOT_COUNT {
            ymf.slots[slot].active = true;
            ymf.slots[slot].waveform = 7;
            ymf.slots[slot].bits = 8;
            ymf.slots[slot].end_addr = 127;
            ymf.slots[slot].loop_addr = 0;
            ymf.slots[slot].alt_loop = 1;
            ymf.slots[slot].step = 1 << 16;
            ymf.slots[slot].volume = 255 << ENV_VOLUME_SHIFT;
            ymf.slots[slot].ch_level = [0, 0, 0, 0];
        }

        let frames = ymf.render_stereo(32);
        let stats = ymf.stats();

        assert_audible_without_i16_clip(&frames, &ymf);
        assert!(stats.limited_frames > 0, "{stats:?}");
    }

    #[test]
    fn extreme_fm_mix_uses_headroom_instead_of_i16_clipping() {
        let mut ymf = Ymf271::new(vec![0; 128]);
        for slot in 0..SLOT_COUNT {
            ymf.slots[slot].active = true;
            ymf.slots[slot].waveform = 6;
            ymf.slots[slot].step = 1 << 16;
            ymf.slots[slot].volume = 255 << ENV_VOLUME_SHIFT;
            ymf.slots[slot].ch_level = [0, 0, 0, 0];
        }

        let frames = ymf.render_stereo(32);
        let stats = ymf.stats();

        assert_audible_without_i16_clip(&frames, &ymf);
        assert!(stats.limited_frames > 0, "{stats:?}");
        assert!(stats.fm_slots_active > 0, "{stats:?}");
    }

    #[test]
    fn sync_mode_two_mixes_three_fm_slots_before_pcm_slot() {
        let mut ymf = Ymf271::new(vec![0; 64]);
        ymf.groups[0].sync = 2;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 0;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(8);
        let stats = ymf.stats();

        assert!(
            frames
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0),
            "{stats:?}"
        );
        assert_eq!(stats.fm_groups_skipped, 0, "{stats:?}");
    }

    #[test]
    fn pcm_sample_decode_preserves_signed_8bit_and_12bit_waveforms() {
        let mut ymf = Ymf271::new(vec![0x00, 0x7f, 0x80, 0xff, 0x80, 0x0f, 0x7f]);

        ymf.slots[0].bits = 8;
        ymf.slots[0].start_addr = 0;
        ymf.slots[0].step_ptr = 0;
        assert_eq!(ymf.current_pcm_sample(0), 0);
        ymf.slots[0].step_ptr = 1 << 16;
        assert_eq!(ymf.current_pcm_sample(0), 0x7f00);
        ymf.slots[0].step_ptr = 2 << 16;
        assert_eq!(ymf.current_pcm_sample(0), i16::MIN);
        ymf.slots[0].step_ptr = 3 << 16;
        assert_eq!(ymf.current_pcm_sample(0), -0x0100);

        ymf.slots[0].bits = 12;
        ymf.slots[0].start_addr = 4;
        ymf.slots[0].step_ptr = 0;
        assert_eq!(ymf.current_pcm_sample(0), i16::MIN);
        ymf.slots[0].step_ptr = 0x1_0000;
        assert_eq!(ymf.current_pcm_sample(0), 0x7ff0);
    }

    #[test]
    fn pcm_12bit_terminal_even_sample_uses_two_bytes_instead_of_stopping_early() {
        let mut ymf = Ymf271::new(vec![0x40, 0xf0]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 12;
        ymf.slots[0].end_addr = 0;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].env_state = super::ENV_DECAY2;
        ymf.slots[0].env_decay2_step = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(2);
        let stats = ymf.stats();

        assert_ne!(frames[0].left, 0, "{frames:?}");
        assert_eq!(frames[1].left, 0, "{frames:?}");
        assert_eq!(stats.pcm_end_events, 1, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_slots_active, 0, "{stats:?}");
    }

    #[test]
    fn high_start_pcm_loop_uses_relative_end_without_requiring_alt_loop() {
        let start = 0x4000usize;
        let mut sample_rom = vec![0u8; start + 4];
        sample_rom[start..start + 4].copy_from_slice(&[0x20, 0x40, 0x60, 0x7f]);
        let mut ymf = Ymf271::new(sample_rom);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].start_addr = start as u32;
        ymf.slots[0].end_addr = 3;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 0;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].env_state = super::ENV_DECAY2;
        ymf.slots[0].env_decay2_step = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(6);
        let stats = ymf.stats();

        assert!(
            frames
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0),
            "{frames:?}"
        );
        assert!(stats.pcm_loop_events >= 1, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_stalled_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_end_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_slots_active, 1, "{stats:?}");
    }

    #[test]
    fn high_start_looped_pcm_uses_relative_loop_and_end_addresses() {
        let start = 0x5000usize;
        let mut sample_rom = vec![0u8; start + 8];
        sample_rom[start..start + 8]
            .copy_from_slice(&[0x10, 0x30, 0x50, 0x70, 0x7f, 0x60, 0x40, 0x20]);
        let mut ymf = Ymf271::new(sample_rom);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].start_addr = start as u32;
        ymf.slots[0].loop_addr = 2;
        ymf.slots[0].end_addr = 7;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].env_state = super::ENV_DECAY2;
        ymf.slots[0].env_decay2_step = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(24);
        let stats = ymf.stats();

        assert_audible_without_i16_clip(&frames, &ymf);
        assert!(stats.pcm_loop_events >= 2, "{stats:?}");
        assert_eq!(stats.pcm_end_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_stalled_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_slots_active, 1, "{stats:?}");
    }

    #[test]
    fn looped_pcm_wraps_to_loop_start_after_inclusive_end_without_skipping_first_sample() {
        let mut ymf = Ymf271::new(vec![0x10, 0x20, 0x30, 0x40]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].loop_addr = 1;
        ymf.slots[0].end_addr = 3;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].env_state = super::ENV_DECAY2;
        ymf.slots[0].env_decay2_step = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        ymf.render_stereo(4);
        let stats = ymf.stats();

        assert_eq!(ymf.slots[0].step_ptr >> 16, 1);
        assert_eq!(stats.pcm_loop_events, 1, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_slots_active, 1, "{stats:?}");
    }

    #[test]
    fn looped_pcm_large_step_wraps_modulo_loop_span_instead_of_sticking_to_tail() {
        let mut ymf = Ymf271::new(vec![0x7f; 16]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].loop_addr = 2;
        ymf.slots[0].end_addr = 5;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].step = 11 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].env_state = super::ENV_DECAY2;
        ymf.slots[0].env_decay2_step = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        ymf.render_stereo(1);
        let stats = ymf.stats();

        assert_eq!(ymf.slots[0].step_ptr >> 16, 3);
        assert_eq!(stats.pcm_loop_events, 1, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 0, "{stats:?}");
        assert_eq!(stats.pcm_slots_active, 1, "{stats:?}");
    }

    #[test]
    fn degenerate_pcm_loop_without_alt_loop_stops_at_end_address() {
        let mut ymf = Ymf271::new(vec![0x7f; 8]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 0;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 0;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(4);
        let stats = ymf.stats();

        assert!(
            frames
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0)
        );
        assert_eq!(stats.pcm_slots_active, 0, "{stats:?}");
        assert_ne!(stats.end_status, 0, "{stats:?}");
    }

    #[test]
    fn ended_one_shot_pcm_keeps_key_latch_until_key_off_to_block_stuck_retrigger() {
        let mut ymf = Ymf271::new(vec![0x7f; 8]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 0;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        ymf.render_stereo(4);
        let ended = ymf.stats();
        assert_eq!(ended.key_on_events, 1, "{ended:?}");
        assert_eq!(ended.pcm_slots_active, 0, "{ended:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let duplicate = ymf.stats();
        assert_eq!(duplicate.key_on_events, 1, "{duplicate:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);

        write_fm_bank0(&mut ymf, 0x00, 0x00);
        assert!(!ymf.slots[0].key_on_latched);
        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let retrigger = ymf.stats();
        assert_eq!(retrigger.key_on_events, 2, "{retrigger:?}");
        assert!(ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert_eq!(ymf.slots[0].step_ptr, 0);
    }

    #[test]
    fn ended_one_shot_pcm_suppresses_immediate_same_program_retrigger_without_key_off() {
        let mut ymf = Ymf271::new(vec![0x40, 0x7f, 0x7f, 0x7f]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 0;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        ymf.render_stereo(4);
        let ended = ymf.stats();
        assert_eq!(ended.key_on_events, 1, "{ended:?}");
        assert_eq!(ended.pcm_slots_active, 0, "{ended:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert!(ymf.slots[0].ended_frame.is_some());

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let retrigger = ymf.stats();

        assert_eq!(retrigger.key_on_events, 1, "{retrigger:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert!(ymf.slots[0].ended_frame.is_some());
        assert_ne!(ymf.slots[0].last_key_on_frame, retrigger.generated_frames);
    }

    #[test]
    fn ended_one_shot_pcm_does_not_replay_same_program_while_key_latched() {
        let mut ymf = Ymf271::new(vec![0x40, 0x7f, 0x7f, 0x7f]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 0;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        ymf.render_stereo(4);
        let ended = ymf.stats();
        assert_eq!(ended.key_on_events, 1, "{ended:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert!(ymf.slots[0].ended_frame.is_some());

        for _ in 0..8 {
            ymf.render_stereo((YMF271_SAMPLE_RATE_HZ / 30) as usize);
            write_fm_bank0(&mut ymf, 0x00, 0x01);
        }
        let duplicate = ymf.stats();

        assert_eq!(duplicate.key_on_events, 1, "{duplicate:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert!(ymf.slots[0].ended_frame.is_some());

        write_fm_bank0(&mut ymf, 0x00, 0x00);
        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let retrigger = ymf.stats();

        assert_eq!(retrigger.key_on_events, 2, "{retrigger:?}");
        assert!(ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert_eq!(ymf.slots[0].step_ptr, 0);
    }

    #[test]
    fn ended_one_shot_pcm_retriggers_after_slot_reprogramming() {
        let mut ymf = Ymf271::new(vec![0x40, 0x7f, 0x7f, 0x7f]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 0;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        ymf.render_stereo(4);
        let ended = ymf.stats();
        assert_eq!(ended.key_on_events, 1, "{ended:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);

        write_main(&mut ymf, 0x00, 0x01);
        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let retrigger = ymf.stats();

        assert_eq!(retrigger.key_on_events, 2, "{retrigger:?}");
        assert!(ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
        assert!(!ymf.slots[0].program_dirty_since_key_on);
        assert_eq!(ymf.slots[0].step_ptr, 0);
        assert_eq!(ymf.slots[0].start_addr, 1);
    }

    #[test]
    fn invalid_alt_loop_range_stops_pcm_instead_of_repeating_tail_sample() {
        let mut ymf = Ymf271::new(vec![0x7f, 0x40, 0x20, 0x10]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 1;
        ymf.slots[0].loop_addr = 3;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(8);
        let stats = ymf.stats();

        assert!(
            frames[..2]
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0),
            "{stats:?}"
        );
        assert!(
            frames[2..]
                .iter()
                .all(|frame| frame.left == 0 && frame.right == 0),
            "{frames:?}"
        );
        assert_eq!(stats.pcm_slots_active, 0, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 1, "{stats:?}");
        assert_eq!(stats.pcm_end_events, 1, "{stats:?}");
    }

    #[test]
    fn key_off_with_zero_release_rate_stops_looped_pcm_instead_of_repeating_forever() {
        let mut ymf = Ymf271::new(vec![0x7f; 4096]);
        activate_direct_looped_pcm_slot(&mut ymf, 0);
        ymf.slots[0].release_rate = 0;
        ymf.slots[0].env_release_step = 0;

        let pre_release = ymf.render_stereo(16);
        assert!(
            pre_release
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0)
        );

        write_fm_bank0(&mut ymf, 0x00, 0x00);
        let release_frames = (KEY_OFF_MIN_RELEASE_FRAMES as usize).saturating_add(8);
        let post_release = ymf.render_stereo(release_frames);
        let stats = ymf.stats();

        assert!(
            post_release
                .iter()
                .any(|frame| frame.left != 0 || frame.right != 0)
        );
        assert_eq!(stats.key_off_events, 1, "{stats:?}");
        assert_eq!(stats.pcm_slots_active, 0, "{stats:?}");
        assert!(!ymf.slots[0].active);
        assert!(!ymf.slots[0].key_on_latched);
    }

    #[test]
    fn out_of_range_pcm_address_stops_instead_of_wrapping_sample_rom() {
        let mut ymf = Ymf271::new(vec![0x7f, 0x7f]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].start_addr = 8;
        ymf.slots[0].end_addr = 8;
        ymf.slots[0].step = 1 << 16;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(4);
        let stats = ymf.stats();

        assert!(
            frames
                .iter()
                .all(|frame| frame.left == 0 && frame.right == 0),
            "{frames:?}"
        );
        assert_eq!(stats.pcm_slots_active, 0, "{stats:?}");
        assert_eq!(stats.pcm_invalid_loop_events, 1, "{stats:?}");
        assert_eq!(stats.pcm_end_events, 1, "{stats:?}");
    }

    #[test]
    fn stalled_pcm_step_is_stopped_instead_of_repeating_one_sample() {
        let mut ymf = Ymf271::new(vec![0x7f; 16]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].active = true;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 15;
        ymf.slots[0].step = 0;
        ymf.slots[0].volume = 255 << ENV_VOLUME_SHIFT;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        let frames = ymf.render_stereo(4);
        let stats = ymf.stats();

        assert!(
            frames
                .iter()
                .all(|frame| frame.left == 0 && frame.right == 0),
            "{frames:?}"
        );
        assert_eq!(stats.pcm_slots_active, 0, "{stats:?}");
        assert_eq!(stats.pcm_stalled_events, 1, "{stats:?}");
        assert_eq!(stats.pcm_end_events, 1, "{stats:?}");
    }

    #[test]
    fn envelope_ended_pcm_marks_ended_and_debounces_same_program_retrigger() {
        let mut ymf = Ymf271::new(vec![0x7f; 16]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 15;
        ymf.slots[0].alt_loop = 1;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        ymf.slots[0].volume = 1;
        ymf.slots[0].env_state = super::ENV_RELEASE;
        ymf.slots[0].env_release_step = 2;

        let frames = ymf.render_stereo(4);
        let stats = ymf.stats();

        assert!(
            frames
                .iter()
                .all(|frame| frame.left == 0 && frame.right == 0),
            "{frames:?}"
        );
        assert_eq!(stats.pcm_slots_active, 0, "{stats:?}");
        assert_eq!(stats.nonzero_frames, 0, "{stats:?}");
        assert_eq!(stats.pcm_end_events, 1, "{stats:?}");
        assert!(ymf.slots[0].ended_frame.is_some());
        assert!(ymf.slots[0].key_on_latched);

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let duplicate = ymf.stats();
        assert_eq!(duplicate.key_on_events, 1, "{duplicate:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);

        ymf.render_stereo((YMF271_SAMPLE_RATE_HZ * 2) as usize);
        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let duplicate = ymf.stats();
        assert_eq!(duplicate.key_on_events, 1, "{duplicate:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);

        write_fm_bank0(&mut ymf, 0x00, 0x00);
        assert!(!ymf.slots[0].key_on_latched);
        write_fm_bank0(&mut ymf, 0x00, 0x01);
        let retrigger = ymf.stats();
        assert_eq!(retrigger.key_on_events, 2, "{retrigger:?}");
        assert!(ymf.slots[0].active);
    }

    #[test]
    fn same_program_parameter_rewrites_do_not_retrigger_latched_ended_effect() {
        let mut ymf = Ymf271::new(vec![0x7f; 16]);
        ymf.groups[0].sync = 3;
        ymf.slots[0].waveform = 7;
        ymf.slots[0].bits = 8;
        ymf.slots[0].end_addr = 0;
        ymf.slots[0].loop_addr = 0;
        ymf.slots[0].alt_loop = 0;
        ymf.slots[0].ch_level = [0, 0, 0, 0];

        write_fm_bank0(&mut ymf, 0x00, 0x01);
        ymf.render_stereo(4);
        assert_eq!(ymf.stats().key_on_events, 1, "{:?}", ymf.stats());
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);

        write_fm_bank0(&mut ymf, 0x40, 0x10);
        write_fm_bank0(&mut ymf, 0xd0, 0xf0);
        write_fm_bank0(&mut ymf, 0xe0, 0x0f);
        write_fm_bank0(&mut ymf, 0x00, 0x01);

        let duplicate = ymf.stats();
        assert_eq!(duplicate.key_on_events, 1, "{duplicate:?}");
        assert!(!ymf.slots[0].active);
        assert!(ymf.slots[0].key_on_latched);
    }

    #[test]
    fn external_rom_read_uses_latched_data_and_auto_increment() {
        let mut ymf = Ymf271::new(vec![0xaa, 0xbb, 0xcc, 0xdd]);

        write_timer(&mut ymf, 0x14, 0x00);
        write_timer(&mut ymf, 0x15, 0x00);
        write_timer(&mut ymf, 0x16, 0x80);

        assert_eq!(ymf.read(0x02), 0x00);
        assert_eq!(ymf.read(0x02), 0xbb);
        assert_eq!(ymf.read(0x02), 0xcc);
    }
}
