use core::mem::MaybeUninit;

use bitfield::bitfield;
use volatile::{
    VolatileFieldAccess,
    access::{NoAccess, ReadOnly, ReadWrite},
};

/// Based on:
/// - https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/software-developers-hpet-spec-1-0a.pdf
/// - https://wiki.osdev.org/HPET#HPET_registers
#[repr(C)]
#[derive(Debug, VolatileFieldAccess)]
pub(crate) struct HpetMemory {
    #[access(ReadOnly)]
    pub capabilities_and_id: HpetGeneralCapabilitiesAndIdRegister,
    #[access(NoAccess)]
    _reserved_008_00f: [MaybeUninit<u8>; 0x8],
    #[access(ReadWrite)]
    pub config: HpetGeneralConfigurationRegister,
    #[access(NoAccess)]
    _reserved_018_01f: [MaybeUninit<u8>; 0x8],
    #[access(ReadWrite)]
    pub interrupt_status: HpetGeneralInterruptStatusRegister,
    #[access(NoAccess)]
    _reserved_028_0ef: [MaybeUninit<u8>; 0xC8],
    /// Make sure that you enable the HPET first. This register increases monotonically. You can write to this if the HPET is halted. To get the actual amount of seconds you need to multiply this by the period.
    #[access(ReadWrite)]
    pub main_counter_value_register: u64,
    #[access(NoAccess)]
    _reserved_0f8_0ff: [MaybeUninit<u8>; 0x8],
    #[access(ReadWrite)]
    /// There is memory for 32 timers, but there are not always physically 32 timers. Check the number of timers before accessing a timer's memory.
    pub timers: [HpetTimerMemory; 32],
}

pub const HPET_MMIO_SIZE: usize = size_of::<HpetMemory>();

bitfield! {
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    pub struct HpetGeneralCapabilitiesAndIdRegister(u64);
    impl Debug;

    /// From the docs:
    /// `COUNTER_CLK_PERIOD`
    /// > Main Counter Tick Period: This read-only field indicates the period at which the counter increments in femtoseconds (10^-15 seconds). A value of 0 in this field is not permitted. The value in this field must be less than or equal to 05F5E100h (10^8 femptoseconds = 100 nanoseconds). The resolution must be in femptoseconds (rather than picoseconds) in order to achieve a resolution of 50 ppm.
    pub u32, get_counter_clk_period, _: 63, 32;
    /// From the docs:
    /// `VENDOR_ID`
    /// > This read-only field will be the same as what would be assigned if this logic was a PCI function.
    pub u16, get_vendor_id, _: 31, 16;
    /// From the docs:
    /// `LEG_RT_CAP`
    /// > LegacyReplacement Route Capable: If this bit is a 1, it indicates that the hardware supports the LegacyReplacement Interrupt Route option.
    pub bool, get_leg_rt_cap, _: 15;
    /// From the docs:
    /// `COUNT_SIZE_CAP`
    /// > Counter Size:
    /// > - This bit is a 0 to indicate that the main counter is 32 bits wide (and cannot operate in 64-bit mode).
    /// > - This bit is a 1 to indicate that the main counter is 64 bits wide (although this does not preclude it from being operated in a 32-bit mode).
    pub bool, get_count_size_cap, _: 13;
    /// From the docs:
    /// `NUM_TIM_CAP`
    /// > *Number of Timers:* This indicates the number of timers in this block. The number in this field indicates the last timer (i.e. if there are three timers, the value will be 02h, four timers will be 03h, five timers will be 04h, etc.).
    pub u8, get_num_tim_cap, _: 12, 8;
    /// From the docs:
    /// `REV_ID`
    /// > This indicates which revision of the function is implemented. The value must NOT be 00h.
    pub u8, get_rev_id, _: 7, 0;
}

bitfield! {
    #[repr(transparent)]
    #[derive(Copy, Clone)]
    pub(crate) struct HpetGeneralConfigurationRegister(u64);
    impl Debug;

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
    pub bool, get_legacy_replacement_cnf, set_legacy_replacement_cnf: 1;
    /// From the docs:
    /// `ENABLE_CNF`
    /// > Overall Enable: This bit must be set to enable any of the timers to generate interrupts. If this bit is 0, then the main counter will halt (will not increment) and no interrupts will be caused by any of these timers.
    /// > - 0 – Halt main count and disable all timer interrupts
    /// > - 1 – allow main counter to run, and allow timer interrupts if enabled
    pub bool, get_enable_cnf, set_enable_cnf: 0;
}

bitfield! {
    /// General Interrupt Status Register
    #[repr(transparent)]
    #[derive(Clone, Copy)]
    pub struct HpetGeneralInterruptStatusRegister(u64);
    impl Debug;

    /// `Tn_INT_STS` in the docs. Timer *n* Interrupt Active.
    ///
    /// If this timer is set to level-triggered mode: This bit will be set to `1` if the timer's interrupt is active. You can set this bit to `0` by writing `1` to it.
    ///
    /// If set to edge-triggered mode: Ignore this. Always write `0` to it if you write to it.
    pub get_t_n_int_sts, set_t_n_int_sts: 0, 0, 32;
}

#[repr(C)]
#[derive(Debug, VolatileFieldAccess)]
pub(crate) struct HpetTimerMemory {
    pub configuration_and_capability_register: TimerNConfigurationAndCapabilityRegister,
    pub comparator_register: u64,
    pub fsb_interrupt_route_register: TimerNFsbInterruptRouteRegister,
    _reserved: MaybeUninit<u64>,
}

bitfield! {
    /// Timer N Configuration and Capability Register
    #[repr(transparent)]
    #[derive(Clone, Copy)]
    pub struct TimerNConfigurationAndCapabilityRegister(u64);
    impl Debug;

    /// `Tn_INT_ROUTE_CAP` in the docs. Each bit represents a IO APIC interrupt. If a bit is 1, that means that this timer supports sending interrupt to the corresponding IO APIC interrupt based on the bit index, where bit 0 is the rightmost.
    pub u32, get_int_route_cap, _: 63, 32;
    pub bool, get_fsb_int_del_cap, _: 15;
    /// > If the Tn_FSB_INT_DEL_CAP bit is set for this timer, then the software can set the Tn_FSB_EN_CNF bit to force the interrupts to be delivered directly as FSB messages, rather than using the I/O (x) APIC. In this case, the Tn_INT_ROUTE_CNF field in this register will be ignored. The Tn_FSB_ROUTE register will be used instead.
    pub bool, get_fsb_en_cnf, set_fsb_en_cnf: 14;
    /// The I/O APIC IRQ number that interrupts will be sent to
    pub u8, get_int_route_cnf, set_int_route_cnf: 13, 9;
    /// > Software can set this read/write bit to force a 64-bit timer to behave as a 32-bit timer.
    /// > This is typically needed if the software is not willing to halt the main counter to read or write a particular timer, and the software is not capable of doing an atomic 64-bit read to the timer.
    /// > If the timer is not 64 bits wide, then this bit will always be read as 0 and writes will have no effect.
    pub bool, get_32_mode_cnf, set_32_mode_cnf: 8;
    /// > Timer n Value Set: (where n is the timer number: 00 to 31). Software uses this
    /// > read/write bit only for timers that have been set to periodic mode. By writing
    /// > this bit to a 1, the software is then allowed to directly set a periodic timer’s
    /// > accumulator.
    /// > Software does NOT have to write this bit back to 0 (it automatically clears).
    pub bool, get_val_set_cnf, set_val_set_cnf: 6;
    /// > (where n is the timer number: 00 to 31). This read-only field
    /// > indicates the size of the timer. 1 = 64-bits, 0 = 32-bits.
    pub bool, get_size_cap, _: 5;
    /// > If this read-only bit is 1, then the hardware supports a periodic mode for
    /// > this timer’s interrupt.
    pub bool, get_per_int_cp, _: 4;
    /// > If the corresponding Tn_PER_INT_CAP bit is 0, then this bit will always
    /// > return 0 when read and writes will have no impact.
    /// > If the corresponding Tn_PER_INT_CAP bit is 1, then this bit is read/write, and
    /// > can be used to enable the timer to generate a periodic interrupt.
    /// > Writing a 1 to this bit enables the timer to generate a periodic interrupt.
    /// > Writing a 0 to this bit enables the timer to generate a non-periodic interrupt.
    pub bool, get_type_cnf, set_type_cnf: 3;
    /// > This read/write bit must be set to enable timer n to cause an interrupt when the timer
    /// > event fires.
    /// > Note: If this bit is 0, the timer will still operate and generate appropriate status
    /// > bits, but will not cause an interrupt.
    pub bool, get_int_enb_cnf, set_int_enb_cnf: 2;
    /// > Timer n Interrupt Type: (where n is the timer number: 00 to 31)
    /// > - 0 = The timer interrupt is edge triggered. This means that an edge-type interrupt is generated.
    /// >   If another interrupt occurs, another edge will be generated.
    /// > - 1 = The timer interrupt is level triggered. This means that a level-triggered interrupt is generated.
    /// >   The interrupt will be held active until it is cleared by writing to the bit in the General Interrupt Status Register.
    /// >   If another interrupt occurs before the interrupt is cleared, the interrupt will remain active.
    pub bool, get_int_type_cnf, set_int_type_cnf: 1;
}

bitfield! {
    /// Timer N FSB Interrupt Route Register
    #[derive(Clone, Copy)]
    pub struct TimerNFsbInterruptRouteRegister(u64);
    impl Debug;

    u32;
    /// > Software sets this 32-bit field to indicate the location that the FSB interrupt
    /// > message should be written to.
    pub fsb_int_addr, set_fsb_int_addr: 63, 32;

    u32;
    /// > Software sets this 32-bit field to indicate the value that is written during the FSB interrupt message.
    pub fsb_int_val, set_fsb_int_val: 31, 0;
}
