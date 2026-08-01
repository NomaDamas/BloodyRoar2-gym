use std::collections::VecDeque;

use m68k::{AddressBus, CpuCore, CpuType};

use crate::native::sound::coreaudio::CoreAudioOutput;
use crate::native::sound::health::AudioHealth;
use crate::native::sound::queue::{SharedStereoQueue, StereoQueueStats, StereoSample};
use crate::native::sound::ymf271::{YMF271_SAMPLE_RATE_HZ, Ymf271, Ymf271Stats};

pub const PS9805_AUDIO_CPU_CLOCK_HZ: u32 = 12_000_000;

const PSX_CPU_CLOCK_HZ: u32 = 33_868_800;
const PS9805_ROM_START: u32 = 0x000000;
const PS9805_ROM_END: u32 = 0x07ffff;
const PS9805_RAM_START: u32 = 0x080000;
const PS9805_RAM_END: u32 = 0x0fffff;
const PS9805_YMF_START: u32 = 0x100000;
const PS9805_YMF_END: u32 = 0x10001f;
const PS9805_LATCH_READ: u32 = 0x180009;
const PS9805_RAM_BYTES: usize = (PS9805_RAM_END - PS9805_RAM_START + 1) as usize;
const PS9805_AUDIO_QUEUE_FRAMES: usize = YMF271_SAMPLE_RATE_HZ as usize;
const PS9805_AUDIO_PREBUFFER_FRAMES: usize = 16_384;
const PS9805_REALTIME_CATCHUP_TARGET_FRAMES: usize = (YMF271_SAMPLE_RATE_HZ as usize * 3) / 4;
const PS9805_REALTIME_CATCHUP_MIN_DEFICIT_FRAMES: usize = 64;
const PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK: usize = 1_024;
const PS9805_EXECUTION_QUANTUM_CYCLES: u64 = 8192;
const PS9805_MAX_EXECUTION_QUANTA_PER_MAIN_TICK: u64 = 32;
const PS9805_MAX_PENDING_SOUND_CYCLES: u64 = PS9805_EXECUTION_QUANTUM_CYCLES * 96;
const PS9805_LATCH_QUEUE_CAPACITY: usize = 64;
const PS9805_MAIN_CYCLE_BATCH_THRESHOLD: u64 = 512;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Ps9805SoundStats {
    pub available: bool,
    pub audio_cpu_rom_bytes: usize,
    pub audio_cpu_mapped_bytes: usize,
    pub ymf_sample_rom_bytes: usize,
    pub cpu_steps: u64,
    pub cpu_execute_batches: u64,
    pub cpu_cycles: u64,
    pub main_cycles_seen: u64,
    pub irq2_assertions: u64,
    pub irq2_acks: u64,
    pub ymf_irq_assertions: u64,
    pub ymf_irq_clears: u64,
    pub ymf_irq_state: u8,
    pub sound_irq_line: u8,
    pub latch_writes: u64,
    pub latch_reads: u64,
    pub latch_nonzero_writes: u64,
    pub latch_dropped_writes: u64,
    pub last_latch_write: u8,
    pub last_latch_read: u8,
    pub rom_reads: u64,
    pub ram_reads: u64,
    pub ram_writes: u64,
    pub ymf_reads: u64,
    pub ymf_writes: u64,
    pub unmapped_reads: u64,
    pub unmapped_writes: u64,
    pub generated_frames: u64,
    pub audio_cpu_backlog_cycles: u64,
    pub audio_cpu_dropped_cycles: u64,
    pub audio_cpu_throttled_ticks: u64,
    pub audio_render_batches: u64,
    pub audio_queue_push_batches: u64,
    pub audio_realtime_catchup_batches: u64,
    pub audio_realtime_catchup_frames: u64,
    pub audio_realtime_freewheel_batches: u64,
    pub audio_realtime_freewheel_frames: u64,
    pub audio_realtime_catchup_lock_busy_ticks: u64,
    pub coreaudio_start_attempts: u64,
    pub coreaudio_started: bool,
    pub cpu_pc: u32,
    pub cpu_stopped: bool,
    pub last_step_cycles: u32,
    pub last_step_result: &'static str,
}

impl Ps9805SoundStats {
    pub fn json(self, ymf: Ymf271Stats, queue: StereoQueueStats) -> String {
        let health = AudioHealth::from_stats(self, ymf, queue);
        format!(
            "{{\"available\":{},\"audio_cpu_clock_hz\":{},\"ymf_sample_rate_hz\":{},\"audio_cpu_rom_bytes\":{},\"audio_cpu_mapped_bytes\":{},\"ymf_sample_rom_bytes\":{},\"cpu_steps\":{},\"cpu_steps_exact\":false,\"cpu_execute_batches\":{},\"cpu_cycles\":{},\"main_cycles_seen\":{},\"irq2_assertions\":{},\"irq2_acks\":{},\"ymf_irq_assertions\":{},\"ymf_irq_clears\":{},\"ymf_irq_state\":{},\"ymf_irq_state_hex\":\"0x{:02x}\",\"sound_irq_line\":{},\"latch_writes\":{},\"latch_reads\":{},\"latch_nonzero_writes\":{},\"latch_dropped_writes\":{},\"last_latch_write\":{},\"last_latch_write_hex\":\"0x{:02x}\",\"last_latch_read\":{},\"last_latch_read_hex\":\"0x{:02x}\",\"rom_reads\":{},\"ram_reads\":{},\"ram_writes\":{},\"ymf_reads\":{},\"ymf_writes\":{},\"unmapped_reads\":{},\"unmapped_writes\":{},\"generated_frames\":{},\"audio_cpu_backlog_cycles\":{},\"audio_cpu_dropped_cycles\":{},\"audio_cpu_throttled_ticks\":{},\"audio_render_batches\":{},\"audio_queue_push_batches\":{},\"audio_realtime_catchup_batches\":{},\"audio_realtime_catchup_frames\":{},\"audio_realtime_freewheel_batches\":{},\"audio_realtime_freewheel_frames\":{},\"audio_realtime_catchup_lock_busy_ticks\":{},\"coreaudio_start_attempts\":{},\"coreaudio_started\":{},\"cpu_pc\":{},\"cpu_pc_hex\":\"0x{:08x}\",\"cpu_stopped\":{},\"last_step_cycles\":{},\"last_step_result\":\"{}\",\"health\":{},\"ymf271\":{},\"queue\":{}}}",
            self.available,
            PS9805_AUDIO_CPU_CLOCK_HZ,
            YMF271_SAMPLE_RATE_HZ,
            self.audio_cpu_rom_bytes,
            self.audio_cpu_mapped_bytes,
            self.ymf_sample_rom_bytes,
            self.cpu_steps,
            self.cpu_execute_batches,
            self.cpu_cycles,
            self.main_cycles_seen,
            self.irq2_assertions,
            self.irq2_acks,
            self.ymf_irq_assertions,
            self.ymf_irq_clears,
            self.ymf_irq_state,
            self.ymf_irq_state,
            self.sound_irq_line,
            self.latch_writes,
            self.latch_reads,
            self.latch_nonzero_writes,
            self.latch_dropped_writes,
            self.last_latch_write,
            self.last_latch_write,
            self.last_latch_read,
            self.last_latch_read,
            self.rom_reads,
            self.ram_reads,
            self.ram_writes,
            self.ymf_reads,
            self.ymf_writes,
            self.unmapped_reads,
            self.unmapped_writes,
            self.generated_frames,
            self.audio_cpu_backlog_cycles,
            self.audio_cpu_dropped_cycles,
            self.audio_cpu_throttled_ticks,
            self.audio_render_batches,
            self.audio_queue_push_batches,
            self.audio_realtime_catchup_batches,
            self.audio_realtime_catchup_frames,
            self.audio_realtime_freewheel_batches,
            self.audio_realtime_freewheel_frames,
            self.audio_realtime_catchup_lock_busy_ticks,
            self.coreaudio_start_attempts,
            self.coreaudio_started,
            self.cpu_pc,
            self.cpu_pc,
            self.cpu_stopped,
            self.last_step_cycles,
            self.last_step_result,
            health.json(),
            ymf.json(),
            queue.json()
        )
    }
}

#[derive(Debug)]
pub struct Ps9805SoundBoard {
    cpu: CpuCore,
    bus: Ps9805Bus,
    queue: SharedStereoQueue,
    batched_main_cycles: u64,
    main_cycle_accumulator: u64,
    pending_sound_cycles: u64,
    audio_frame_accumulator: u64,
    render_scratch: Vec<StereoSample>,
    ymf_irq_line_asserted: bool,
    stats: Ps9805SoundStats,
}

impl Clone for Ps9805SoundBoard {
    fn clone(&self) -> Self {
        let mut cloned = Self::new(self.bus.audio_cpu_rom.clone(), self.bus.ymf_sample_rom());
        cloned.bus.ram = self.bus.ram.clone();
        cloned.bus.latch = self.bus.latch;
        cloned.bus.latch_pending = self.bus.latch_pending;
        cloned.bus.latch_queue = self.bus.latch_queue.clone();
        cloned.bus.irq2_pending = self.bus.irq2_pending;
        cloned.batched_main_cycles = self.batched_main_cycles;
        cloned.main_cycle_accumulator = self.main_cycle_accumulator;
        cloned.pending_sound_cycles = self.pending_sound_cycles;
        cloned.audio_frame_accumulator = self.audio_frame_accumulator;
        cloned.ymf_irq_line_asserted = self.ymf_irq_line_asserted;
        cloned.stats = self.stats;
        cloned.stats.coreaudio_started = false;
        cloned
    }
}

impl Ps9805SoundBoard {
    pub fn new(audio_cpu_rom: Vec<u8>, ymf_sample_rom: Vec<u8>) -> Self {
        let mut bus = Ps9805Bus::new(audio_cpu_rom, ymf_sample_rom);
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(CpuType::M68000);
        cpu.reset(&mut bus);

        let stats = Ps9805SoundStats {
            available: true,
            audio_cpu_rom_bytes: bus.audio_cpu_rom.len(),
            audio_cpu_mapped_bytes: bus.audio_cpu_rom.len().min(PS9805_ROM_MAPPED_BYTES),
            ymf_sample_rom_bytes: bus.ymf.sample_rom_len(),
            last_step_result: "reset",
            ..Ps9805SoundStats::default()
        };

        Self {
            cpu,
            bus,
            queue: SharedStereoQueue::new(PS9805_AUDIO_QUEUE_FRAMES),
            batched_main_cycles: 0,
            main_cycle_accumulator: 0,
            pending_sound_cycles: 0,
            audio_frame_accumulator: 0,
            render_scratch: Vec::new(),
            ymf_irq_line_asserted: false,
            stats,
        }
    }

    pub fn from_roms(
        audio_cpu_rom: Option<Vec<u8>>,
        ymf_sample_rom: Option<Vec<u8>>,
    ) -> Option<Self> {
        Some(Self::new(audio_cpu_rom?, ymf_sample_rom?))
    }

    pub fn write_latch(&mut self, data: u8) {
        self.flush_batched_main_cycles();
        self.bus.latch = data;
        self.bus.latch_pending = true;
        if self.bus.latch_queue.len() == PS9805_LATCH_QUEUE_CAPACITY {
            self.bus.latch_queue.pop_front();
            self.bus.stats.latch_dropped_writes =
                self.bus.stats.latch_dropped_writes.saturating_add(1);
        }
        self.bus.latch_queue.push_back(data);
        self.bus.stats.latch_writes = self.bus.stats.latch_writes.saturating_add(1);
        self.bus.stats.latch_nonzero_writes = self
            .bus
            .stats
            .latch_nonzero_writes
            .saturating_add(u64::from(data != 0));
        self.bus.stats.last_latch_write = data;
    }

    pub fn assert_irq2(&mut self) {
        self.flush_batched_main_cycles();
        self.bus.irq2_pending = true;
        self.bus.stats.irq2_assertions = self.bus.stats.irq2_assertions.saturating_add(1);
        self.sync_sound_irq_line();
    }

    pub fn tick_main_cycles(&mut self, main_cycles: u64) {
        self.stats.main_cycles_seen = self.stats.main_cycles_seen.saturating_add(main_cycles);
        self.batched_main_cycles = self.batched_main_cycles.saturating_add(main_cycles);
        if self.batched_main_cycles < PS9805_MAIN_CYCLE_BATCH_THRESHOLD {
            self.stats.audio_cpu_backlog_cycles = self.effective_audio_cpu_backlog_cycles();
            return;
        }
        self.flush_batched_main_cycles();
    }

    fn flush_batched_main_cycles(&mut self) {
        let main_cycles = std::mem::take(&mut self.batched_main_cycles);
        if main_cycles == 0 {
            return;
        }
        self.advance_sound_from_main_cycles(main_cycles);
    }

    fn advance_sound_from_main_cycles(&mut self, main_cycles: u64) {
        let (sound_cycles, remainder) = scaled_ticks_with_remainder(
            main_cycles,
            u64::from(PS9805_AUDIO_CPU_CLOCK_HZ),
            u64::from(PSX_CPU_CLOCK_HZ),
            self.main_cycle_accumulator,
        );
        self.main_cycle_accumulator = remainder;
        self.pending_sound_cycles = self.pending_sound_cycles.saturating_add(sound_cycles);
        self.clamp_pending_sound_cycles();

        let ready_quanta = self.pending_sound_cycles / PS9805_EXECUTION_QUANTUM_CYCLES;
        let quanta_to_run = ready_quanta.min(PS9805_MAX_EXECUTION_QUANTA_PER_MAIN_TICK);
        if ready_quanta > quanta_to_run {
            self.stats.audio_cpu_throttled_ticks =
                self.stats.audio_cpu_throttled_ticks.saturating_add(1);
        }
        if quanta_to_run == 0 {
            self.stats.audio_cpu_backlog_cycles = self.pending_sound_cycles;
            return;
        }

        let mut rendered = std::mem::take(&mut self.render_scratch);
        rendered.clear();
        rendered.reserve(estimated_frames_for_sound_cycles(
            quanta_to_run.saturating_mul(PS9805_EXECUTION_QUANTUM_CYCLES),
        ));
        for _ in 0..quanta_to_run {
            self.run_cpu_for_sound_cycles(PS9805_EXECUTION_QUANTUM_CYCLES);
            self.render_for_sound_cycles_into(PS9805_EXECUTION_QUANTUM_CYCLES, &mut rendered);
            self.pending_sound_cycles = self
                .pending_sound_cycles
                .saturating_sub(PS9805_EXECUTION_QUANTUM_CYCLES);
        }
        self.push_rendered_audio(&mut rendered);
        self.top_up_realtime_audio(&mut rendered);
        self.render_scratch = rendered;
        self.stats.audio_cpu_backlog_cycles = self.pending_sound_cycles;
    }

    pub fn tick_sound_cycles(&mut self, target_cycles: u64) {
        if target_cycles == 0 {
            return;
        }
        self.flush_batched_main_cycles();

        let mut rendered = std::mem::take(&mut self.render_scratch);
        rendered.clear();
        rendered.reserve(estimated_frames_for_sound_cycles(target_cycles));
        self.run_cpu_for_sound_cycles(target_cycles);
        self.render_for_sound_cycles_into(target_cycles, &mut rendered);
        self.push_rendered_audio(&mut rendered);
        self.render_scratch = rendered;
    }

    fn run_cpu_for_sound_cycles(&mut self, target_cycles: u64) {
        let mut remaining = target_cycles;
        let mut consumed_total = 0u64;
        while remaining > 0 {
            self.sync_sound_irq_line();
            if self.cpu.is_stopped() && !self.cpu.check_interrupts() {
                self.stats.last_step_cycles = 0;
                self.stats.last_step_result = "stopped_waiting_irq";
                break;
            }

            let budget = remaining.min(i32::MAX as u64) as i32;
            let consumed = self.cpu.execute(&mut self.bus, budget).max(0) as u64;
            self.sync_sound_irq_line();
            self.stats.cpu_execute_batches = self.stats.cpu_execute_batches.saturating_add(1);
            self.stats.cpu_steps = self.stats.cpu_steps.saturating_add(1);
            self.stats.last_step_cycles = consumed.min(u64::from(u32::MAX)) as u32;
            self.stats.last_step_result = if self.cpu.is_stopped() {
                "batch_stopped"
            } else {
                "batch_execute"
            };
            consumed_total = consumed_total.saturating_add(consumed);
            if consumed == 0 || self.cpu.is_stopped() {
                break;
            }
            remaining = remaining.saturating_sub(consumed.min(remaining));
        }

        self.stats.cpu_cycles = self.stats.cpu_cycles.saturating_add(consumed_total);
    }

    fn sync_sound_irq_line(&mut self) {
        let ymf_irq_pending = self.bus.ymf.irq_pending();
        if ymf_irq_pending && !self.ymf_irq_line_asserted {
            self.stats.ymf_irq_assertions = self.stats.ymf_irq_assertions.saturating_add(1);
            self.ymf_irq_line_asserted = true;
        } else if !ymf_irq_pending && self.ymf_irq_line_asserted {
            self.stats.ymf_irq_clears = self.stats.ymf_irq_clears.saturating_add(1);
            self.ymf_irq_line_asserted = false;
        }

        let level = if self.bus.irq2_pending || ymf_irq_pending {
            2
        } else {
            0
        };
        self.cpu.set_irq(level);
    }

    pub fn start_coreaudio(&mut self) -> Result<CoreAudioOutput, String> {
        self.stats.coreaudio_start_attempts = self.stats.coreaudio_start_attempts.saturating_add(1);
        self.flush_batched_main_cycles();
        self.queue
            .prepare_for_playback(PS9805_AUDIO_PREBUFFER_FRAMES);
        let output = CoreAudioOutput::start(self.queue.clone(), YMF271_SAMPLE_RATE_HZ)?;
        self.stats.coreaudio_started = true;
        Ok(output)
    }

    pub fn queue(&self) -> SharedStereoQueue {
        self.queue.clone()
    }

    pub fn pcm_snapshot(&self, max_frames: usize) -> Vec<StereoSample> {
        self.queue.snapshot_latest_frames(max_frames)
    }

    pub fn stats(&self) -> Ps9805SoundStats {
        let mut stats = self.stats;
        stats.audio_cpu_backlog_cycles = self.effective_audio_cpu_backlog_cycles();
        stats.irq2_assertions = self.bus.stats.irq2_assertions;
        stats.irq2_acks = self.bus.stats.irq2_acks;
        stats.ymf_irq_state = self.bus.ymf.irq_state();
        stats.sound_irq_line = if self.bus.irq2_pending || self.bus.ymf.irq_pending() {
            2
        } else {
            0
        };
        stats.latch_writes = self.bus.stats.latch_writes;
        stats.latch_reads = self.bus.stats.latch_reads;
        stats.latch_nonzero_writes = self.bus.stats.latch_nonzero_writes;
        stats.latch_dropped_writes = self.bus.stats.latch_dropped_writes;
        stats.last_latch_write = self.bus.stats.last_latch_write;
        stats.last_latch_read = self.bus.stats.last_latch_read;
        stats.rom_reads = self.bus.stats.rom_reads;
        stats.ram_reads = self.bus.stats.ram_reads;
        stats.ram_writes = self.bus.stats.ram_writes;
        stats.ymf_reads = self.bus.stats.ymf_reads;
        stats.ymf_writes = self.bus.stats.ymf_writes;
        stats.unmapped_reads = self.bus.stats.unmapped_reads;
        stats.unmapped_writes = self.bus.stats.unmapped_writes;
        stats.generated_frames = self.bus.ymf.stats().generated_frames;
        stats.cpu_pc = self.cpu.pc;
        stats.cpu_stopped = self.cpu.is_stopped();
        stats
    }

    pub fn stats_json(&self) -> String {
        self.stats().json(self.bus.ymf.stats(), self.queue.stats())
    }

    pub fn audio_health(&self) -> AudioHealth {
        AudioHealth::from_stats(self.stats(), self.bus.ymf.stats(), self.queue.stats())
    }

    pub fn audio_health_json(&self) -> String {
        self.audio_health().json()
    }

    pub fn realtime_playback_needs_game_time(&self) -> bool {
        self.stats.coreaudio_started
            && self
                .queue
                .queued_frames()
                .is_none_or(|queued| queued < PS9805_AUDIO_PREBUFFER_FRAMES / 2)
    }

    fn render_for_sound_cycles_into(
        &mut self,
        sound_cycles: u64,
        rendered: &mut Vec<StereoSample>,
    ) {
        let (frames, remainder) = scaled_ticks_with_remainder(
            sound_cycles,
            u64::from(YMF271_SAMPLE_RATE_HZ),
            u64::from(PS9805_AUDIO_CPU_CLOCK_HZ),
            self.audio_frame_accumulator,
        );
        self.audio_frame_accumulator = remainder;
        if frames == 0 {
            return;
        }
        self.bus
            .ymf
            .render_stereo_into(usize::try_from(frames).unwrap_or(usize::MAX), rendered);
        self.stats.audio_render_batches = self.stats.audio_render_batches.saturating_add(1);
        self.sync_sound_irq_line();
    }

    fn push_rendered_audio(&mut self, rendered: &mut Vec<StereoSample>) {
        if rendered.is_empty() {
            return;
        }
        self.queue.push_slice(rendered);
        self.stats.audio_queue_push_batches = self.stats.audio_queue_push_batches.saturating_add(1);
        rendered.clear();
    }

    fn top_up_realtime_audio(&mut self, rendered: &mut Vec<StereoSample>) {
        if !self.stats.coreaudio_started {
            return;
        }

        let Some(queued_frames) = self.queue.queued_frames() else {
            self.stats.audio_realtime_catchup_lock_busy_ticks = self
                .stats
                .audio_realtime_catchup_lock_busy_ticks
                .saturating_add(1);
            return;
        };
        let target = PS9805_REALTIME_CATCHUP_TARGET_FRAMES.min(PS9805_AUDIO_QUEUE_FRAMES);
        let deficit = target.saturating_sub(queued_frames);
        if deficit < PS9805_REALTIME_CATCHUP_MIN_DEFICIT_FRAMES {
            return;
        }

        let catchup_frames = deficit.min(PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK);
        let catchup_cycles =
            sound_cycles_for_audio_frames(catchup_frames).min(self.pending_sound_cycles);
        if catchup_cycles == 0 {
            return;
        }

        rendered.clear();
        rendered.reserve(estimated_frames_for_sound_cycles(catchup_cycles));
        self.run_cpu_for_sound_cycles(catchup_cycles);
        self.render_for_sound_cycles_into(catchup_cycles, rendered);
        self.pending_sound_cycles = self.pending_sound_cycles.saturating_sub(catchup_cycles);
        let rendered_frames = rendered.len();
        if rendered_frames == 0 {
            return;
        }

        self.stats.audio_realtime_catchup_batches =
            self.stats.audio_realtime_catchup_batches.saturating_add(1);
        self.stats.audio_realtime_catchup_frames = self
            .stats
            .audio_realtime_catchup_frames
            .saturating_add(rendered_frames as u64);
        self.push_rendered_audio(rendered);
    }

    fn clamp_pending_sound_cycles(&mut self) {
        if self.pending_sound_cycles <= PS9805_MAX_PENDING_SOUND_CYCLES {
            return;
        }
        let dropped = self
            .pending_sound_cycles
            .saturating_sub(PS9805_MAX_PENDING_SOUND_CYCLES);
        self.pending_sound_cycles = PS9805_MAX_PENDING_SOUND_CYCLES;
        self.stats.audio_cpu_dropped_cycles =
            self.stats.audio_cpu_dropped_cycles.saturating_add(dropped);
    }

    fn effective_audio_cpu_backlog_cycles(&self) -> u64 {
        let (batched_sound_cycles, _) = scaled_ticks_with_remainder(
            self.batched_main_cycles,
            u64::from(PS9805_AUDIO_CPU_CLOCK_HZ),
            u64::from(PSX_CPU_CLOCK_HZ),
            self.main_cycle_accumulator,
        );
        self.pending_sound_cycles
            .saturating_add(batched_sound_cycles)
            .min(PS9805_MAX_PENDING_SOUND_CYCLES)
    }
}

fn estimated_frames_for_sound_cycles(sound_cycles: u64) -> usize {
    let (frames, _) = scaled_ticks_with_remainder(
        sound_cycles,
        u64::from(YMF271_SAMPLE_RATE_HZ),
        u64::from(PS9805_AUDIO_CPU_CLOCK_HZ),
        0,
    );
    usize::try_from(frames).unwrap_or(usize::MAX)
}

fn sound_cycles_for_audio_frames(frames: usize) -> u64 {
    let frames = u64::try_from(frames).unwrap_or(u64::MAX);
    frames.saturating_mul(u64::from(PS9805_AUDIO_CPU_CLOCK_HZ)) / u64::from(YMF271_SAMPLE_RATE_HZ)
}

fn scaled_ticks_with_remainder(
    input_ticks: u64,
    numerator: u64,
    denominator: u64,
    remainder: u64,
) -> (u64, u64) {
    debug_assert!(denominator > 0);
    debug_assert!(remainder < denominator);
    debug_assert!(numerator.checked_mul(denominator).is_some());

    let whole = input_ticks / denominator;
    let partial = input_ticks % denominator;
    let partial_scaled = partial.saturating_mul(numerator).saturating_add(remainder);
    let output = whole
        .saturating_mul(numerator)
        .saturating_add(partial_scaled / denominator);
    (output, partial_scaled % denominator)
}

#[derive(Clone, Debug)]
struct Ps9805Bus {
    audio_cpu_rom: Vec<u8>,
    ram: Vec<u8>,
    ymf: Ymf271,
    latch: u8,
    latch_pending: bool,
    latch_queue: VecDeque<u8>,
    irq2_pending: bool,
    stats: Ps9805BusStats,
}

impl Ps9805Bus {
    fn new(audio_cpu_rom: Vec<u8>, ymf_sample_rom: Vec<u8>) -> Self {
        Self {
            audio_cpu_rom,
            ram: vec![0; PS9805_RAM_BYTES],
            ymf: Ymf271::new(ymf_sample_rom),
            latch: 0,
            latch_pending: false,
            latch_queue: VecDeque::with_capacity(PS9805_LATCH_QUEUE_CAPACITY),
            irq2_pending: false,
            stats: Ps9805BusStats::default(),
        }
    }

    fn ymf_sample_rom(&self) -> Vec<u8> {
        self.ymf_sample_rom_slice().to_vec()
    }

    fn ymf_sample_rom_slice(&self) -> &[u8] {
        self.ymf.sample_rom()
    }
}

impl AddressBus for Ps9805Bus {
    fn read_byte(&mut self, address: u32) -> u8 {
        let address = ps9805_address(address);
        if let Some(offset) = rom_offset(address, 1, self.audio_cpu_rom.len()) {
            self.stats.rom_reads = self.stats.rom_reads.saturating_add(1);
            return self.audio_cpu_rom[offset];
        }
        if let Some(offset) = ram_offset(address, 1) {
            self.stats.ram_reads = self.stats.ram_reads.saturating_add(1);
            return self.ram[offset];
        }
        if address == PS9805_LATCH_READ {
            self.stats.latch_reads = self.stats.latch_reads.saturating_add(1);
            let value = self
                .latch_queue
                .pop_front()
                .unwrap_or(if self.latch_pending { self.latch } else { 0 });
            self.latch_pending = !self.latch_queue.is_empty();
            self.stats.last_latch_read = value;
            return value;
        }
        if let Some(offset) = ymf_low_lane_offset(address) {
            self.stats.ymf_reads = self.stats.ymf_reads.saturating_add(1);
            return self.ymf.read(offset);
        }
        if (PS9805_YMF_START..=PS9805_YMF_END).contains(&address) {
            return 0xff;
        }

        self.stats.unmapped_reads = self.stats.unmapped_reads.saturating_add(1);
        0xff
    }

    fn read_word(&mut self, address: u32) -> u16 {
        let mapped = ps9805_address(address);
        if let Some(offset) = rom_offset(mapped, 2, self.audio_cpu_rom.len()) {
            self.stats.rom_reads = self.stats.rom_reads.saturating_add(2);
            return u16::from_be_bytes([
                self.audio_cpu_rom[offset],
                self.audio_cpu_rom[offset + 1],
            ]);
        }
        if let Some(offset) = ram_offset(mapped, 2) {
            self.stats.ram_reads = self.stats.ram_reads.saturating_add(2);
            return u16::from_be_bytes([self.ram[offset], self.ram[offset + 1]]);
        }
        u16::from_be_bytes([
            self.read_byte(address),
            self.read_byte(address.wrapping_add(1)),
        ])
    }

    fn read_long(&mut self, address: u32) -> u32 {
        let mapped = ps9805_address(address);
        if let Some(offset) = rom_offset(mapped, 4, self.audio_cpu_rom.len()) {
            self.stats.rom_reads = self.stats.rom_reads.saturating_add(4);
            return u32::from_be_bytes([
                self.audio_cpu_rom[offset],
                self.audio_cpu_rom[offset + 1],
                self.audio_cpu_rom[offset + 2],
                self.audio_cpu_rom[offset + 3],
            ]);
        }
        if let Some(offset) = ram_offset(mapped, 4) {
            self.stats.ram_reads = self.stats.ram_reads.saturating_add(4);
            return u32::from_be_bytes([
                self.ram[offset],
                self.ram[offset + 1],
                self.ram[offset + 2],
                self.ram[offset + 3],
            ]);
        }
        u32::from_be_bytes([
            self.read_byte(address),
            self.read_byte(address.wrapping_add(1)),
            self.read_byte(address.wrapping_add(2)),
            self.read_byte(address.wrapping_add(3)),
        ])
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        let address = ps9805_address(address);
        if let Some(offset) = ram_offset(address, 1) {
            self.ram[offset] = value;
            self.stats.ram_writes = self.stats.ram_writes.saturating_add(1);
            return;
        }
        if let Some(offset) = ymf_low_lane_offset(address) {
            self.ymf.write(offset, value);
            self.stats.ymf_writes = self.stats.ymf_writes.saturating_add(1);
            return;
        }
        if (PS9805_ROM_START..=PS9805_ROM_END).contains(&address)
            || (PS9805_YMF_START..=PS9805_YMF_END).contains(&address)
        {
            return;
        }
        self.stats.unmapped_writes = self.stats.unmapped_writes.saturating_add(1);
    }

    fn write_word(&mut self, address: u32, value: u16) {
        let mapped = ps9805_address(address);
        if let Some(offset) = ram_offset(mapped, 2) {
            self.ram[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
            self.stats.ram_writes = self.stats.ram_writes.saturating_add(2);
            return;
        }
        if rom_offset(mapped, 2, self.audio_cpu_rom.len()).is_some() {
            return;
        }
        let [high, low] = value.to_be_bytes();
        self.write_byte(address, high);
        self.write_byte(address.wrapping_add(1), low);
    }

    fn write_long(&mut self, address: u32, value: u32) {
        let mapped = ps9805_address(address);
        if let Some(offset) = ram_offset(mapped, 4) {
            self.ram[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            self.stats.ram_writes = self.stats.ram_writes.saturating_add(4);
            return;
        }
        if rom_offset(mapped, 4, self.audio_cpu_rom.len()).is_some() {
            return;
        }
        let [b0, b1, b2, b3] = value.to_be_bytes();
        self.write_byte(address, b0);
        self.write_byte(address.wrapping_add(1), b1);
        self.write_byte(address.wrapping_add(2), b2);
        self.write_byte(address.wrapping_add(3), b3);
    }

    fn read_immediate_word(&mut self, address: u32) -> u16 {
        self.read_word(address)
    }

    fn read_immediate_long(&mut self, address: u32) -> u32 {
        self.read_long(address)
    }

    fn interrupt_acknowledge(&mut self, level: u8) -> u32 {
        if level == 2 && self.irq2_pending {
            self.irq2_pending = false;
            self.stats.irq2_acks = self.stats.irq2_acks.saturating_add(1);
        }
        0xffff_ffff
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Ps9805BusStats {
    irq2_assertions: u64,
    irq2_acks: u64,
    latch_writes: u64,
    latch_reads: u64,
    latch_nonzero_writes: u64,
    latch_dropped_writes: u64,
    last_latch_write: u8,
    last_latch_read: u8,
    rom_reads: u64,
    ram_reads: u64,
    ram_writes: u64,
    ymf_reads: u64,
    ymf_writes: u64,
    unmapped_reads: u64,
    unmapped_writes: u64,
}

const PS9805_ROM_MAPPED_BYTES: usize = (PS9805_ROM_END - PS9805_ROM_START + 1) as usize;

fn ps9805_address(address: u32) -> u32 {
    address & 0x00ff_ffff
}

fn rom_offset(address: u32, access_len: usize, rom_len: usize) -> Option<usize> {
    if !(PS9805_ROM_START..=PS9805_ROM_END).contains(&address) {
        return None;
    }
    let offset = (address - PS9805_ROM_START) as usize;
    (offset + access_len <= rom_len.min(PS9805_ROM_MAPPED_BYTES)).then_some(offset)
}

fn ram_offset(address: u32, access_len: usize) -> Option<usize> {
    if !(PS9805_RAM_START..=PS9805_RAM_END).contains(&address) {
        return None;
    }
    let offset = (address - PS9805_RAM_START) as usize;
    (offset + access_len <= PS9805_RAM_BYTES).then_some(offset)
}

fn ymf_low_lane_offset(address: u32) -> Option<u32> {
    if !(PS9805_YMF_START..=PS9805_YMF_END).contains(&address) || address & 1 == 0 {
        return None;
    }
    Some(((address - PS9805_YMF_START) >> 1) & 0x0f)
}

#[cfg(test)]
mod tests {
    use m68k::AddressBus;

    use crate::native::sound::health::AudioHealthState;
    use crate::native::sound::queue::{BoundedStereoQueue, StereoSample};

    use super::{
        PS9805_AUDIO_CPU_CLOCK_HZ, PS9805_AUDIO_PREBUFFER_FRAMES, PS9805_AUDIO_QUEUE_FRAMES,
        PS9805_EXECUTION_QUANTUM_CYCLES, PS9805_LATCH_QUEUE_CAPACITY,
        PS9805_MAIN_CYCLE_BATCH_THRESHOLD, PS9805_MAX_EXECUTION_QUANTA_PER_MAIN_TICK,
        PS9805_MAX_PENDING_SOUND_CYCLES, PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK,
        PS9805_REALTIME_CATCHUP_TARGET_FRAMES, PSX_CPU_CLOCK_HZ, Ps9805Bus, Ps9805SoundBoard,
    };

    fn minimal_loop_rom() -> Vec<u8> {
        let mut rom = vec![0xff; 0x100000];
        rom[0..4].copy_from_slice(&0x000f_0000u32.to_be_bytes());
        rom[4..8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
        rom[0x100..0x102].copy_from_slice(&0x4e71u16.to_be_bytes());
        rom[0x102..0x104].copy_from_slice(&0x60fcu16.to_be_bytes());
        rom
    }

    fn stop_and_irq_rom() -> Vec<u8> {
        let mut rom = vec![0xff; 0x100000];
        rom[0..4].copy_from_slice(&0x000f_0000u32.to_be_bytes());
        rom[4..8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
        rom[(26 * 4)..(26 * 4 + 4)].copy_from_slice(&0x0000_0200u32.to_be_bytes());
        rom[0x100..0x102].copy_from_slice(&0x4e72u16.to_be_bytes());
        rom[0x102..0x104].copy_from_slice(&0x2000u16.to_be_bytes());
        rom[0x104..0x106].copy_from_slice(&0x4e71u16.to_be_bytes());
        rom[0x106..0x108].copy_from_slice(&0x60feu16.to_be_bytes());
        rom[0x200..0x202].copy_from_slice(&0x4e73u16.to_be_bytes());
        rom
    }

    fn board_program_one_pcm_voice(board: &mut Ps9805SoundBoard) {
        board.bus.ymf.write(0x0c, 0x00);
        board.bus.ymf.write(0x0d, 0x03);
        board.bus.ymf.write(0x08, 0x00);
        board.bus.ymf.write(0x09, 0x00);
        board.bus.ymf.write(0x08, 0x30);
        board.bus.ymf.write(0x09, 0x20);
        board.bus.ymf.write(0x00, 0x30);
        board.bus.ymf.write(0x01, 0x01);
        board.bus.ymf.write(0x00, 0x40);
        board.bus.ymf.write(0x01, 0x00);
        board.bus.ymf.write(0x00, 0x50);
        board.bus.ymf.write(0x01, 0xff);
        board.bus.ymf.write(0x00, 0x80);
        board.bus.ymf.write(0x01, 0x0f);
        board.bus.ymf.write(0x00, 0xa0);
        board.bus.ymf.write(0x01, 0x40);
        board.bus.ymf.write(0x00, 0x90);
        board.bus.ymf.write(0x01, 0xff);
        board.bus.ymf.write(0x00, 0xb0);
        board.bus.ymf.write(0x01, 0x07);
        board.bus.ymf.write(0x00, 0xd0);
        board.bus.ymf.write(0x01, 0x00);
        board.bus.ymf.write(0x00, 0x00);
        board.bus.ymf.write(0x01, 0x01);
    }

    #[test]
    fn bus_maps_latch_irq2_and_ymf_low_byte_lane() {
        let mut bus = Ps9805Bus::new(minimal_loop_rom(), vec![0; 0x400000]);
        bus.latch = 0xa5;
        bus.latch_pending = true;
        assert_eq!(bus.read_byte(0x180009), 0xa5);
        bus.irq2_pending = true;
        assert_eq!(bus.interrupt_acknowledge(2), 0xffff_ffff);
        assert!(!bus.irq2_pending);

        bus.write_word(0x100000, 0x0000);
        bus.write_word(0x100002, 0x0003);
        assert_eq!(bus.stats.ymf_writes, 2);
        assert_eq!(bus.read_byte(0x100000), 0xff);
    }

    #[test]
    fn bus_wide_accesses_preserve_big_endian_rom_ram_and_io_lanes() {
        let mut bus = Ps9805Bus::new(minimal_loop_rom(), vec![0; 0x400000]);
        bus.audio_cpu_rom[0x120..0x124].copy_from_slice(&[0x12, 0x34, 0x56, 0x78]);
        assert_eq!(bus.read_word(0x120), 0x1234);
        assert_eq!(bus.read_long(0x120), 0x1234_5678);

        bus.write_long(0x080120, 0x89ab_cdef);
        assert_eq!(bus.read_word(0x080120), 0x89ab);
        assert_eq!(bus.read_long(0x080120), 0x89ab_cdef);

        bus.latch = 0xa5;
        bus.latch_pending = true;
        assert_eq!(bus.read_word(0x180008), 0xffa5);
        assert_eq!(bus.read_word(0x180009), 0x00ff);
    }

    #[test]
    fn latch_command_is_consumed_until_next_main_write() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);

        board.write_latch(0x7c);
        assert_eq!(board.bus.read_byte(0x180009), 0x7c);
        assert_eq!(board.bus.read_byte(0x180009), 0x00);

        board.write_latch(0x7c);
        assert_eq!(board.bus.read_byte(0x180009), 0x7c);

        let stats = board.stats();
        assert_eq!(stats.latch_writes, 2, "{stats:?}");
        assert_eq!(stats.latch_reads, 3, "{stats:?}");
        assert_eq!(stats.last_latch_read, 0x7c, "{stats:?}");
    }

    #[test]
    fn latch_burst_preserves_ordered_sfx_commands_when_audio_cpu_lags() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);

        board.write_latch(0x11);
        board.write_latch(0x22);
        board.write_latch(0x33);

        assert_eq!(board.bus.read_byte(0x180009), 0x11);
        assert_eq!(board.bus.read_byte(0x180009), 0x22);
        assert_eq!(board.bus.read_byte(0x180009), 0x33);
        assert_eq!(board.bus.read_byte(0x180009), 0x00);

        let stats = board.stats();
        assert_eq!(stats.latch_writes, 3, "{stats:?}");
        assert_eq!(stats.latch_reads, 4, "{stats:?}");
        assert_eq!(stats.latch_nonzero_writes, 3, "{stats:?}");
        assert_eq!(stats.latch_dropped_writes, 0, "{stats:?}");
        assert_eq!(stats.last_latch_write, 0x33, "{stats:?}");
        assert_eq!(stats.last_latch_read, 0x00, "{stats:?}");
    }

    #[test]
    fn latch_burst_is_bounded_and_keeps_latest_commands() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);

        for value in 0..(PS9805_LATCH_QUEUE_CAPACITY + 3) {
            board.write_latch(value as u8);
        }

        let stats = board.stats();
        assert_eq!(stats.latch_writes, (PS9805_LATCH_QUEUE_CAPACITY + 3) as u64);
        assert_eq!(stats.latch_dropped_writes, 3, "{stats:?}");
        assert_eq!(board.bus.read_byte(0x180009), 3);
        assert_eq!(board.bus.read_byte(0x180009), 4);
    }

    #[test]
    fn main_cycle_ticks_batch_below_threshold_but_report_effective_backlog() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);
        let deferred_main_cycles = PS9805_MAIN_CYCLE_BATCH_THRESHOLD - 1;

        for _ in 0..deferred_main_cycles {
            board.tick_main_cycles(1);
        }

        let stats = board.stats();
        let (expected_backlog, expected_remainder) = super::scaled_ticks_with_remainder(
            deferred_main_cycles,
            u64::from(PS9805_AUDIO_CPU_CLOCK_HZ),
            u64::from(PSX_CPU_CLOCK_HZ),
            0,
        );
        assert_eq!(board.batched_main_cycles, deferred_main_cycles);
        assert_eq!(board.pending_sound_cycles, 0);
        assert_eq!(board.main_cycle_accumulator, 0);
        assert_eq!(stats.main_cycles_seen, deferred_main_cycles);
        assert_eq!(stats.audio_cpu_backlog_cycles, expected_backlog);

        board.tick_main_cycles(1);
        let (flushed_sound_cycles, flushed_remainder) = super::scaled_ticks_with_remainder(
            PS9805_MAIN_CYCLE_BATCH_THRESHOLD,
            u64::from(PS9805_AUDIO_CPU_CLOCK_HZ),
            u64::from(PSX_CPU_CLOCK_HZ),
            0,
        );
        assert_eq!(board.batched_main_cycles, 0);
        assert_eq!(board.pending_sound_cycles, flushed_sound_cycles);
        assert_eq!(board.main_cycle_accumulator, flushed_remainder);
        assert_ne!(expected_remainder, flushed_remainder);
    }

    #[test]
    fn batched_main_cycle_conversion_matches_single_scaled_advance() {
        let sequence = [3_u64, 17, 29, 61, 127, 251, 509, 7, 43, 89, 233];
        let total_main_cycles = sequence.iter().sum::<u64>();
        let mut batched = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);
        let mut direct = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);

        for main_cycles in sequence {
            batched.tick_main_cycles(main_cycles);
        }
        batched.flush_batched_main_cycles();
        direct.advance_sound_from_main_cycles(total_main_cycles);

        assert_eq!(batched.batched_main_cycles, 0);
        assert_eq!(batched.pending_sound_cycles, direct.pending_sound_cycles);
        assert_eq!(
            batched.main_cycle_accumulator,
            direct.main_cycle_accumulator
        );
        assert_eq!(
            batched.stats.cpu_execute_batches,
            direct.stats.cpu_execute_batches
        );
    }

    #[test]
    fn latch_write_flushes_batched_main_cycles_before_command_becomes_visible() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);
        board.tick_main_cycles(PS9805_MAIN_CYCLE_BATCH_THRESHOLD - 1);
        assert!(board.batched_main_cycles > 0);

        board.write_latch(0x44);

        let stats = board.stats();
        assert_eq!(board.batched_main_cycles, 0);
        assert!(board.pending_sound_cycles > 0);
        assert_eq!(stats.latch_writes, 1);
        assert_eq!(board.bus.read_byte(0x180009), 0x44);
    }

    #[test]
    fn board_ticks_cpu_and_renders_real_pcm_queue() {
        let mut sample_rom = vec![0u8; 0x400000];
        for (index, byte) in sample_rom.iter_mut().take(256).enumerate() {
            *byte = (index as u8).wrapping_mul(11).wrapping_add(0x20);
        }
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), sample_rom);

        board.bus.ymf.write(0x0c, 0x00);
        board.bus.ymf.write(0x0d, 0x03);
        board.bus.ymf.write(0x08, 0x00);
        board.bus.ymf.write(0x09, 0x00);
        board.bus.ymf.write(0x08, 0x30);
        board.bus.ymf.write(0x09, 0x20);
        board.bus.ymf.write(0x00, 0x30);
        board.bus.ymf.write(0x01, 0x01);
        board.bus.ymf.write(0x00, 0x40);
        board.bus.ymf.write(0x01, 0x00);
        board.bus.ymf.write(0x00, 0x50);
        board.bus.ymf.write(0x01, 0xff);
        board.bus.ymf.write(0x00, 0x80);
        board.bus.ymf.write(0x01, 0x0f);
        board.bus.ymf.write(0x00, 0xa0);
        board.bus.ymf.write(0x01, 0x40);
        board.bus.ymf.write(0x00, 0x90);
        board.bus.ymf.write(0x01, 0xff);
        board.bus.ymf.write(0x00, 0xb0);
        board.bus.ymf.write(0x01, 0x07);
        board.bus.ymf.write(0x00, 0xd0);
        board.bus.ymf.write(0x01, 0x00);
        board.bus.ymf.write(0x00, 0x00);
        board.bus.ymf.write(0x01, 0x01);

        board.tick_sound_cycles(PS9805_AUDIO_CPU_CLOCK_HZ as u64 / 60);
        let stats = board.stats();
        assert!(stats.cpu_execute_batches > 0, "{stats:?}");
        assert!(board.bus.ymf.stats().last_rms_left > 0);
        assert!(board.bus.ymf.stats().last_rms_right > 0);
        assert!(board.queue.stats().pushed_frames > 0);
    }

    #[test]
    fn stopped_audio_cpu_resumes_for_irq2() {
        let mut board = Ps9805SoundBoard::new(stop_and_irq_rom(), vec![0; 0x400000]);
        board.tick_sound_cycles(128);
        assert!(board.cpu.is_stopped());

        let steps_before = board.stats().cpu_steps;
        board.assert_irq2();
        board.tick_sound_cycles(256);

        let stats = board.stats();
        assert_eq!(stats.irq2_assertions, 1);
        assert_eq!(stats.irq2_acks, 1);
        assert!(stats.cpu_steps > steps_before);
        assert!(!stats.cpu_stopped);
    }

    #[test]
    fn ymf_timer_irq_asserts_and_clears_ps9805_m68k_irq_line() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);
        board.bus.ymf.write(0x0c, 0x10);
        board.bus.ymf.write(0x0d, 0xff);
        board.bus.ymf.write(0x0c, 0x11);
        board.bus.ymf.write(0x0d, 0x03);
        board.bus.ymf.write(0x0c, 0x13);
        board.bus.ymf.write(0x0d, 0x05);

        board.tick_sound_cycles(PS9805_AUDIO_CPU_CLOCK_HZ as u64 / 60);
        let asserted = board.stats();
        assert_eq!(asserted.ymf_irq_state, 0x01, "{asserted:?}");
        assert_eq!(asserted.sound_irq_line, 2, "{asserted:?}");
        assert_eq!(asserted.ymf_irq_assertions, 1, "{asserted:?}");
        assert_eq!(board.cpu.int_level, 2);

        board.bus.ymf.write(0x0c, 0x13);
        board.bus.ymf.write(0x0d, 0x10);
        board.sync_sound_irq_line();
        let cleared = board.stats();
        assert_eq!(cleared.ymf_irq_state, 0, "{cleared:?}");
        assert_eq!(cleared.sound_irq_line, 0, "{cleared:?}");
        assert_eq!(cleared.ymf_irq_clears, 1, "{cleared:?}");
        assert_eq!(board.cpu.int_level, 0);
    }

    #[test]
    fn audio_health_reports_silent_render_and_active_pcm() {
        let mut silent = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0; 0x400000]);
        silent.tick_sound_cycles(PS9805_AUDIO_CPU_CLOCK_HZ as u64 / 60);
        let silent_health = silent.audio_health();
        assert_eq!(silent_health.state, AudioHealthState::SilentPcm);
        assert!(silent_health.render_progressing);
        assert!(!silent_health.pcm_nonzero);
        assert!(silent.audio_health_json().contains("\"silent_pcm\""));
        assert!(silent.stats_json().contains("\"health\""));

        let mut active_sample_rom = vec![0u8; 0x400000];
        for (index, byte) in active_sample_rom.iter_mut().take(256).enumerate() {
            *byte = (index as u8).wrapping_mul(11).wrapping_add(0x20);
        }
        let mut active = Ps9805SoundBoard::new(minimal_loop_rom(), active_sample_rom);
        board_program_one_pcm_voice(&mut active);

        active.tick_sound_cycles(PS9805_AUDIO_CPU_CLOCK_HZ as u64 / 60);
        let rendered_health = active.audio_health();
        assert_eq!(rendered_health.state, AudioHealthState::OutputIdle);
        assert!(!rendered_health.audible());

        active.stats.coreaudio_started = true;
        active.queue.record_coreaudio_start();
        active.queue.record_coreaudio_callback(512);
        let output_health = active.audio_health();
        assert_eq!(output_health.state, AudioHealthState::Active);
        assert!(output_health.audible());
    }

    #[test]
    fn main_cycle_tick_batches_audio_pushes_and_bounds_backlog() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0x7f; 0x400000]);
        board.tick_main_cycles(u64::from(PSX_CPU_CLOCK_HZ) * 2);
        let stats = board.stats();
        let queue_stats = board.queue.stats();

        assert_eq!(
            stats.audio_render_batches,
            PS9805_MAX_EXECUTION_QUANTA_PER_MAIN_TICK
        );
        assert_eq!(stats.audio_queue_push_batches, 1);
        assert_eq!(
            stats.audio_cpu_backlog_cycles,
            PS9805_MAX_PENDING_SOUND_CYCLES
                - PS9805_MAX_EXECUTION_QUANTA_PER_MAIN_TICK * PS9805_EXECUTION_QUANTUM_CYCLES
        );
        assert!(stats.audio_cpu_dropped_cycles > 0, "{stats:?}");
        assert_eq!(stats.audio_cpu_throttled_ticks, 1, "{stats:?}");
        assert!(queue_stats.pushed_frames > 0, "{queue_stats:?}");
        assert_eq!(queue_stats.producer_miss_frames, 0, "{queue_stats:?}");
    }

    #[test]
    fn scaled_tick_accumulator_matches_exact_fraction_across_batches() {
        let inputs = [1, 7, 31, 8_192, 65_537, u64::from(u32::MAX)];
        let numerator = u64::from(super::YMF271_SAMPLE_RATE_HZ);
        let denominator = u64::from(PS9805_AUDIO_CPU_CLOCK_HZ);
        let mut remainder = 0;
        let mut output = 0u64;
        let mut total_input = 0u128;

        for input in inputs {
            let (batch_output, batch_remainder) =
                super::scaled_ticks_with_remainder(input, numerator, denominator, remainder);
            output = output.saturating_add(batch_output);
            remainder = batch_remainder;
            total_input = total_input.saturating_add(u128::from(input));
        }

        let exact = total_input
            .saturating_mul(u128::from(numerator))
            .checked_div(u128::from(denominator))
            .unwrap();
        assert_eq!(u128::from(output), exact);
        assert_eq!(
            u128::from(remainder),
            total_input.saturating_mul(u128::from(numerator)) % u128::from(denominator)
        );
    }

    #[test]
    fn direct_sound_tick_uses_one_queue_push_for_large_render_block() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0x7f; 0x400000]);
        let sound_cycles = PS9805_EXECUTION_QUANTUM_CYCLES * 12;
        board.tick_sound_cycles(sound_cycles);
        let stats = board.stats();
        let queue_stats = board.queue.stats();

        assert_eq!(stats.audio_render_batches, 1);
        assert_eq!(stats.audio_queue_push_batches, 1);
        assert_eq!(
            queue_stats.pushed_frames as usize,
            super::estimated_frames_for_sound_cycles(sound_cycles)
        );
        assert_eq!(queue_stats.producer_miss_frames, 0, "{queue_stats:?}");
    }

    #[test]
    fn coreaudio_playback_buffering_is_bounded_below_one_second() {
        const {
            assert!(PS9805_AUDIO_PREBUFFER_FRAMES < PS9805_AUDIO_QUEUE_FRAMES);
            assert!(PS9805_AUDIO_PREBUFFER_FRAMES <= (super::YMF271_SAMPLE_RATE_HZ as usize) / 2);
            assert!(
                PS9805_AUDIO_PREBUFFER_FRAMES <= (super::YMF271_SAMPLE_RATE_HZ as usize * 3) / 8
            );
            assert!(PS9805_REALTIME_CATCHUP_TARGET_FRAMES <= PS9805_AUDIO_QUEUE_FRAMES);
            assert!(PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK <= 1024);
            assert!(PS9805_AUDIO_QUEUE_FRAMES <= super::YMF271_SAMPLE_RATE_HZ as usize);
        }
    }

    #[test]
    fn coreaudio_prebuffer_reports_startup_pressure_after_guard_exhausts() {
        let mut queue = BoundedStereoQueue::new(PS9805_AUDIO_QUEUE_FRAMES);
        let seed = vec![
            StereoSample {
                left: 1600,
                right: -1600,
            };
            PS9805_AUDIO_PREBUFFER_FRAMES
        ];
        queue.push_slice(&seed);
        queue.prepare_for_playback(PS9805_AUDIO_PREBUFFER_FRAMES);

        // CoreAudio primes hardware buffers before AudioQueueStart, then GUI
        // deep-capture work can delay producer refills for another burst.
        for callback_index in 0..64 {
            let mut output = vec![0i16; 512 * 2];
            queue.pop_interleaved_i16(&mut output);
            if callback_index < 32 {
                assert!(
                    output
                        .chunks_exact(2)
                        .all(|frame| frame[0] != 0 || frame[1] != 0),
                    "callback {callback_index}, stats={:?}",
                    queue.stats()
                );
            }
        }

        let stats = queue.stats();
        assert!(stats.underflow_frames > 0, "{stats:?}");
        assert!(stats.starvation_events > 0, "{stats:?}");
        assert!(
            stats.concealed_frames <= crate::native::sound::queue::MAX_CONCEALMENT_FRAMES as u64
        );
        assert!(stats.repeated_frames > 0, "{stats:?}");
        assert!(
            stats.repeated_frames <= stats.output_frames / 3,
            "{stats:?}"
        );
    }

    #[test]
    fn realtime_catchup_consumes_pending_sound_cpu_before_refilling_queue() {
        let mut sample_rom = vec![0u8; 0x400000];
        for (index, byte) in sample_rom.iter_mut().take(256).enumerate() {
            *byte = (index as u8).wrapping_mul(7).wrapping_add(0x30);
        }
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), sample_rom);
        board_program_one_pcm_voice(&mut board);
        board.stats.coreaudio_started = true;
        board.pending_sound_cycles =
            super::sound_cycles_for_audio_frames(PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK * 2);
        let pending_before = board.pending_sound_cycles;

        let mut rendered = Vec::new();
        board.top_up_realtime_audio(&mut rendered);

        let stats = board.stats();
        let queue = board.queue.stats();
        assert!(board.pending_sound_cycles < pending_before);
        assert_eq!(stats.audio_realtime_catchup_batches, 1, "{stats:?}");
        assert!(stats.cpu_execute_batches > 0, "{stats:?}");
        assert!(stats.audio_realtime_catchup_frames > 0, "{stats:?}");
        assert!(
            stats.audio_realtime_catchup_frames
                <= PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK as u64,
            "{stats:?}"
        );
        assert_eq!(stats.audio_realtime_freewheel_frames, 0, "{stats:?}");
        assert_eq!(queue.pushed_frames, stats.audio_realtime_catchup_frames);
        assert!(board.bus.ymf.stats().nonzero_frames > 0);
        assert_eq!(queue.producer_miss_frames, 0, "{queue:?}");
    }

    #[test]
    fn realtime_catchup_cpu_work_is_capped_for_attack_and_beast_hot_path() {
        let mut sample_rom = vec![0u8; 0x400000];
        for (index, byte) in sample_rom.iter_mut().take(256).enumerate() {
            *byte = (index as u8).wrapping_mul(3).wrapping_add(0x30);
        }
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), sample_rom);
        board_program_one_pcm_voice(&mut board);
        board.stats.coreaudio_started = true;
        board.pending_sound_cycles = super::sound_cycles_for_audio_frames(
            PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK.saturating_mul(12),
        );
        let pending_before = board.pending_sound_cycles;

        let mut rendered = Vec::new();
        board.top_up_realtime_audio(&mut rendered);

        let consumed = pending_before.saturating_sub(board.pending_sound_cycles);
        let max_cycles =
            super::sound_cycles_for_audio_frames(PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK);
        let stats = board.stats();
        assert!(consumed <= max_cycles, "{stats:?}");
        assert!(
            stats.audio_realtime_catchup_frames
                <= PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK as u64,
            "{stats:?}"
        );
        assert_eq!(stats.audio_realtime_catchup_batches, 1, "{stats:?}");
    }

    #[test]
    fn realtime_top_up_does_not_advance_sound_without_game_time() {
        let mut sample_rom = vec![0u8; 0x400000];
        for (index, byte) in sample_rom.iter_mut().take(256).enumerate() {
            *byte = (index as u8).wrapping_mul(5).wrapping_add(0x20);
        }
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), sample_rom);
        board_program_one_pcm_voice(&mut board);
        board.stats.coreaudio_started = true;

        let mut rendered = Vec::new();
        board.top_up_realtime_audio(&mut rendered);

        let stats = board.stats();
        let queue = board.queue.stats();
        assert_eq!(stats.audio_realtime_catchup_batches, 0, "{stats:?}");
        assert_eq!(stats.audio_realtime_catchup_frames, 0, "{stats:?}");
        assert_eq!(stats.audio_realtime_freewheel_batches, 0, "{stats:?}");
        assert_eq!(stats.audio_realtime_freewheel_frames, 0, "{stats:?}");
        assert_eq!(queue.pushed_frames, 0);
        assert_eq!(board.bus.ymf.stats().generated_frames, 0);
        assert_eq!(board.bus.ymf.stats().nonzero_frames, 0);
    }

    #[test]
    fn realtime_catchup_is_limited_to_pending_game_time() {
        let mut sample_rom = vec![0u8; 0x400000];
        for (index, byte) in sample_rom.iter_mut().take(256).enumerate() {
            *byte = (index as u8).wrapping_mul(9).wrapping_add(0x20);
        }
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), sample_rom);
        board_program_one_pcm_voice(&mut board);
        board.stats.coreaudio_started = true;
        board.pending_sound_cycles =
            super::sound_cycles_for_audio_frames(PS9805_REALTIME_CATCHUP_MAX_FRAMES_PER_TICK / 2);

        let mut rendered = Vec::new();
        board.top_up_realtime_audio(&mut rendered);

        let stats = board.stats();
        let queue = board.queue.stats();
        assert_eq!(stats.audio_realtime_catchup_batches, 1, "{stats:?}");
        assert_eq!(stats.audio_realtime_freewheel_batches, 0, "{stats:?}");
        assert!(stats.audio_realtime_catchup_frames > 0, "{stats:?}");
        assert_eq!(stats.audio_realtime_freewheel_frames, 0, "{stats:?}");
        assert_eq!(queue.pushed_frames, stats.audio_realtime_catchup_frames);
        assert_eq!(board.pending_sound_cycles, 0);
        assert!(board.bus.ymf.stats().nonzero_frames > 0);
    }

    #[test]
    fn realtime_top_up_skips_stopped_idle_board_without_pending_game_time() {
        let mut board = Ps9805SoundBoard::new(stop_and_irq_rom(), vec![0; 0x400000]);
        board.tick_sound_cycles(128);
        assert!(board.cpu.is_stopped());
        board.bus.ymf.write(0x0c, 0x10);
        board.bus.ymf.write(0x0d, 0xff);
        assert!(board.bus.ymf.stats().timer_writes > 0);
        assert!(!board.bus.ymf.timers_running());
        board.stats.coreaudio_started = true;

        let mut rendered = Vec::new();
        board.top_up_realtime_audio(&mut rendered);

        let stats = board.stats();
        let queue = board.queue.stats();
        assert_eq!(stats.audio_realtime_freewheel_batches, 0, "{stats:?}");
        assert_eq!(stats.audio_realtime_freewheel_frames, 0, "{stats:?}");
        assert_eq!(queue.pushed_frames, 0, "{queue:?}");
    }

    #[test]
    fn realtime_catchup_does_not_overfill_queue_near_target() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0x7f; 0x400000]);
        board.stats.coreaudio_started = true;
        let seed = vec![
            StereoSample {
                left: 200,
                right: -200,
            };
            PS9805_REALTIME_CATCHUP_TARGET_FRAMES
        ];
        board.queue.push_slice(&seed);

        let mut rendered = Vec::new();
        board.top_up_realtime_audio(&mut rendered);

        let stats = board.stats();
        let queue = board.queue.stats();
        assert_eq!(stats.audio_realtime_catchup_batches, 0, "{stats:?}");
        assert_eq!(
            queue.queued_frames, PS9805_REALTIME_CATCHUP_TARGET_FRAMES,
            "{queue:?}"
        );
    }

    #[test]
    fn realtime_playback_requests_game_time_only_below_low_water() {
        let mut board = Ps9805SoundBoard::new(minimal_loop_rom(), vec![0x7f; 0x400000]);
        assert!(!board.realtime_playback_needs_game_time());

        board.stats.coreaudio_started = true;
        assert!(board.realtime_playback_needs_game_time());

        let seed = vec![
            StereoSample {
                left: 200,
                right: -200,
            };
            PS9805_AUDIO_PREBUFFER_FRAMES
        ];
        board.queue.push_slice(&seed);
        assert!(!board.realtime_playback_needs_game_time());

        let mut output = vec![0i16; (PS9805_AUDIO_PREBUFFER_FRAMES / 2 + 1) * 2];
        board.queue.pop_interleaved_i16(&mut output);
        assert!(board.realtime_playback_needs_game_time());
    }
}
