use core::mem::MaybeUninit;

use arbitrary_int::{u3, u5, u12};
use bitbybit::bitfield;
use volatile::{
    VolatileFieldAccess,
    access::{NoAccess, ReadOnly, ReadWrite},
};

/// Based on:
/// - https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/software-developers-hpet-spec-1-0a.pdf
/// - https://wiki.osdev.org/HPET#HPET_registers
#[repr(C)]
#[derive(Debug, VolatileFieldAccess)]
pub struct HpetMemory {
    #[access(ReadOnly)]
    pub(crate) capabilities_and_id: GeneralCapAndIdReg,
    #[access(NoAccess)]
    _reserved_008_00f: [MaybeUninit<u8>; 0x8],
    #[access(ReadWrite)]
    pub(crate) config: GeneralConfReg,
    #[access(NoAccess)]
    _reserved_018_01f: [MaybeUninit<u8>; 0x8],
    #[access(ReadWrite)]
    pub(crate) interrupt_status: GeneralIntStatusReg,
    #[access(NoAccess)]
    _reserved_028_0ef: [MaybeUninit<u8>; 0xC8],
    /// Make sure that you enable the HPET first. This register increases monotonically. You can write to this if the HPET is halted. To get the actual amount of seconds you need to multiply this by the period.
    #[access(ReadWrite)]
    pub(crate) main_counter_value_register: u64,
    #[access(NoAccess)]
    _reserved_0f8_0ff: [MaybeUninit<u8>; 0x8],
    #[access(ReadWrite)]
    /// There is memory for 32 timers, but there are not always physically 32 timers. Check the number of timers before accessing a timer's memory.
    pub(crate) timers: [HpetTimerMemory; 32],
}

#[bitfield(u64, debug)]
pub struct GeneralCapAndIdReg {
    /// From the docs:
    /// `REV_ID`
    /// > This indicates which revision of the function is implemented. The value must NOT be 00h.
    #[bits(0..=7, r)]
    rev_id: u8,
    /// From the docs:
    /// `NUM_TIM_CAP`
    /// > *Number of Timers:* This indicates the number of timers in this block. The number in this field indicates the last timer (i.e. if there are three timers, the value will be 02h, four timers will be 03h, five timers will be 04h, etc.).
    #[bits(8..=12, r)]
    num_tim_cap: u5,
    /// From the docs:
    /// `COUNT_SIZE_CAP`
    /// > Counter Size:
    /// > - This bit is a 0 to indicate that the main counter is 32 bits wide (and cannot operate in 64-bit mode).
    /// > - This bit is a 1 to indicate that the main counter is 64 bits wide (although this does not preclude it from being operated in a 32-bit mode).
    #[bit(13, r)]
    count_size_cap: bool,
    /// From the docs:
    /// `LEG_RT_CAP`
    /// > LegacyReplacement Route Capable: If this bit is a 1, it indicates that the hardware supports the LegacyReplacement Interrupt Route option.
    #[bit(15, r)]
    leg_rt_cap: bool,
    /// From the docs:
    /// `VENDOR_ID`
    /// > This read-only field will be the same as what would be assigned if this logic was a PCI function.
    #[bits(16..=31, r)]
    vendor_id: u16,
    #[bits(32..=63, r)]
    /// From the docs:
    /// `COUNTER_CLK_PERIOD`
    /// > Main Counter Tick Period: This read-only field indicates the period at which the counter increments in femtoseconds (10^-15 seconds). A value of 0 in this field is not permitted. The value in this field must be less than or equal to 05F5E100h (10^8 femptoseconds = 100 nanoseconds). The resolution must be in femptoseconds (rather than picoseconds) in order to achieve a resolution of 50 ppm.
    counter_clk_period: u32,
}

#[bitfield(u64, debug)]
pub struct GeneralConfReg {
    /// From the docs:
    /// `ENABLE_CNF`
    /// > Overall Enable: This bit must be set to enable any of the timers to generate interrupts. If this bit is 0, then the main counter will halt (will not increment) and no interrupts will be caused by any of these timers.
    /// > - 0 – Halt main count and disable all timer interrupts
    /// > - 1 – allow main counter to run, and allow timer interrupts if enabled
    #[bit(0, rw)]
    enable_cnf: bool,
    /// From the docs:
    /// > **LegacyReplacement Route:**
    /// > - 0 – Doesn’t support **LegacyReplacement Route**
    /// > - 1 – Supports **LegacyReplacement Route**
    /// > If the ENABLE_CNF bit and the LEG_RT_CNF bit are both set, then the interrupts will be routed as follows:
    /// > Timer 0 will be routed to IRQ0 in Non-APIC or IRQ2 in the I/O APIC
    /// > Timer 1 will be routed to IRQ8 in Non-APIC or IRQ8 in the I/O APIC
    /// > Timer 2-n will be routed as per the routing in the timer n config registers.
    /// >
    /// > If the LegacyReplacement Route bit is set, the individual routing bits for timers 0 and 1 (APIC or FSB) will have no impact.
    /// >
    /// > If the LegacyReplacement Route bit is not set, the individual routing bits for each of the timers are used.
    #[bit(1, rw)]
    legacy_replacement_cnf: bool,
}

#[bitfield(u64, debug)]
pub struct GeneralIntStatusReg {
    /// `Tn_INT_STS` in the docs. Timer *n* Interrupt Active.
    ///
    /// If this timer is set to level-triggered mode: This bit will be set to `1` if the timer's interrupt is active. You can set this bit to `0` by writing `1` to it.
    ///
    /// If set to edge-triggered mode: Ignore this. Always write `0` to it if you write to it.
    #[bits(0..=31, rw)]
    t_n_int_sts: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, VolatileFieldAccess)]
pub(crate) struct HpetTimerMemory {
    pub configuration_and_capability_register: TimerNFConfAndCapReg,
    pub comparator_register: u64,
    pub fsb_interrupt_route_register: TimerNFsbIntRouteReg,
    _reserved: MaybeUninit<u64>,
}

#[bitfield(u64, debug)]
pub struct TimerNFConfAndCapReg {
    /// > Timer n Interrupt Type: (where n is the timer number: 00 to 31)
    /// > - 0 = The timer interrupt is edge triggered. This means that an edge-type interrupt is generated.
    /// >   If another interrupt occurs, another edge will be generated.
    /// > - 1 = The timer interrupt is level triggered. This means that a level-triggered interrupt is generated.
    /// >   The interrupt will be held active until it is cleared by writing to the bit in the General Interrupt Status Register.
    /// >   If another interrupt occurs before the interrupt is cleared, the interrupt will remain active.
    #[bit(1, rw)]
    int_type_cnf: bool,
    /// > This read/write bit must be set to enable timer n to cause an interrupt when the timer
    /// > event fires.
    /// > Note: If this bit is 0, the timer will still operate and generate appropriate status
    /// > bits, but will not cause an interrupt.
    #[bit(2, rw)]
    int_enable: bool,
    /// > If the corresponding Tn_PER_INT_CAP bit is 0, then this bit will always
    /// > return 0 when read and writes will have no impact.
    /// > If the corresponding Tn_PER_INT_CAP bit is 1, then this bit is read/write, and
    /// > can be used to enable the timer to generate a periodic interrupt.
    /// > Writing a 1 to this bit enables the timer to generate a periodic interrupt.
    /// > Writing a 0 to this bit enables the timer to generate a non-periodic interrupt.
    #[bit(3, rw)]
    _type_cnf: bool,
    /// > If this read-only bit is 1, then the hardware supports a periodic mode for
    /// > this timer’s interrupt.
    #[bit(4, r)]
    periodic_mode_supported: bool,
    /// > (where n is the timer number: 00 to 31). This read-only field
    /// > indicates the size of the timer. 1 = 64-bits, 0 = 32-bits.
    #[bit(5, r)]
    size_cap: bool,
    /// > Timer n Value Set: (where n is the timer number: 00 to 31). Software uses this
    /// > read/write bit only for timers that have been set to periodic mode. By writing
    /// > this bit to a 1, the software is then allowed to directly set a periodic timer’s
    /// > accumulator.
    /// > Software does NOT have to write this bit back to 0 (it automatically clears).
    #[bit(6, rw)]
    val_set_cnf: bool,
    /// > Software can set this read/write bit to force a 64-bit timer to behave as a 32-bit timer.
    /// > This is typically needed if the software is not willing to halt the main counter to read or write a particular timer, and the software is not capable of doing an atomic 64-bit read to the timer.
    /// > If the timer is not 64 bits wide, then this bit will always be read as 0 and writes will have no effect.
    #[bit(8, rw)]
    _32_mode_cnf: bool,
    /// The I/O APIC IRQ number that interrupts will be sent to
    #[bits(9..=13, rw)]
    int_route_cnf: u5,
    /// > If the Tn_FSB_INT_DEL_CAP bit is set for this timer, then the software can set the Tn_FSB_EN_CNF bit to force the interrupts to be delivered directly as FSB messages, rather than using the I/O (x) APIC. In this case, the Tn_INT_ROUTE_CNF field in this register will be ignored. The Tn_FSB_ROUTE register will be used instead.}
    #[bit(14, rw)]
    fsb_en_cnf: bool,
    #[bit(15, r)]
    fsb_int_supported: bool,
    /// `Tn_INT_ROUTE_CAP` in the docs. Each bit represents a IO APIC interrupt. If a bit is 1, that means that this timer supports sending interrupt to the corresponding IO APIC interrupt based on the bit index, where bit 0 is the rightmost.
    #[bits(32..=63, r)]
    int_route_cap: u32,
}

/// Timer N FSB Interrupt Route Register
#[bitfield(u64, debug)]
pub struct TimerNFsbIntRouteReg {
    /// > Software sets this 32-bit field to indicate the value that is written during the FSB interrupt message.
    #[bits(0..=31, rw)]
    pub fsb_int_val: u32,
    /// > Software sets this 32-bit field to indicate the location that the FSB interrupt
    /// > message should be written to.
    #[bits(32..=63, rw)]
    pub fsb_int_addr: u32,
}

#[bitfield(u32, debug)]
pub struct FsbApicIntValue {
    #[bits(0..=7, rw)]
    pub interrupt_vector: u8,
    #[bits(8..=10, rw)]
    pub delivery_mode: u3,
    #[bit(14, rw)]
    pub level: bool,
    #[bit(15, rw)]
    pub trigger_mode: bool,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeliveryMode {
    Fixed,
    LowestPriority,
    Smi,
    Nmi,
    Init,
    ExInit,
}
impl From<DeliveryMode> for u3 {
    fn from(value: DeliveryMode) -> Self {
        Self::new(value as u8)
    }
}

#[repr(u8)]
pub enum ApicIntLevel {
    Low,
    High,
}
impl From<ApicIntLevel> for bool {
    fn from(value: ApicIntLevel) -> Self {
        match value {
            ApicIntLevel::Low => false,
            ApicIntLevel::High => true,
        }
    }
}

#[repr(u8)]
pub enum FsbIntTriggerMode {
    Edge,
    Level,
}
impl From<FsbIntTriggerMode> for bool {
    fn from(value: FsbIntTriggerMode) -> Self {
        match value {
            FsbIntTriggerMode::Edge => false,
            FsbIntTriggerMode::Level => true,
        }
    }
}

#[bitfield(u32, debug)]
pub struct FsbApicIntAddr {
    /// 0: physical APIC ID.
    /// 1: logical APIC ID.
    #[bit(2, rw)]
    destination_mode: bool,
    /// 0: directed to desination ID.
    /// 1:Lowest priority CPU in destination group.
    #[bit(3, rw)]
    redirection_hint: bool,
    #[bits(12..=19, rw)]
    destination_id: u8,
    #[bits(20..=31, rw)]
    fixed_value: u12,
}

impl FsbApicIntAddr {
    pub const APIC_FIXED_VALUE: u12 = u12::new(0xFEE);
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApicDestMode {
    Physical,
    Logical,
}
impl From<ApicDestMode> for bool {
    fn from(value: ApicDestMode) -> Self {
        match value {
            ApicDestMode::Physical => false,
            ApicDestMode::Logical => true,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RedirectionHint {
    DestId,
    LowestPriorityCpu,
}
impl From<RedirectionHint> for bool {
    fn from(value: RedirectionHint) -> Self {
        match value {
            RedirectionHint::DestId => false,
            RedirectionHint::LowestPriorityCpu => true,
        }
    }
}
