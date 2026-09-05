use core::{fmt::Debug, ptr::NonNull};

use arbitrary_int::u5;
use volatile::{VolatilePtr, VolatileRef, access::ReadOnly};

use crate::*;

pub struct Hpet<'a> {
    mmio: VolatileRef<'a, HpetMemory>,
}

impl Hpet<'_> {
    /// To call this function:
    /// - Use the `acpi` crate to parse ACPI tables
    /// - Find the `HPET` table with `acpi::HpetInfo::new`
    /// - Find the physical base address of the HPET
    /// - Map the HPET, using [`HPET_MMIO_SIZE`]
    ///
    /// # Safety
    /// The address must be a virtual address mapped to HPET memory as un-cacheable (UC).
    pub unsafe fn new(ptr: NonNull<HpetMemory>) -> Self {
        Self {
            mmio: { unsafe { VolatileRef::new(ptr) } },
        }
    }

    pub fn vendor_id(&self) -> u16 {
        self.mmio.as_ptr().capabilities_and_id().read().vendor_id()
    }

    pub fn timers_count(&self) -> u8 {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .num_tim_cap()
            .value()
            + 1
    }

    /// Get the main counter tick period in femtoseconds
    pub fn main_counter_tick_period(&self) -> u32 {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .counter_clk_period()
    }

    pub fn legacy_replacement_capable(&self) -> bool {
        self.mmio.as_ptr().capabilities_and_id().read().leg_rt_cap()
    }

    pub fn supports_64_bit_mode(&self) -> bool {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .count_size_cap()
    }

    pub fn revision_id(&self) -> u8 {
        self.mmio.as_ptr().capabilities_and_id().read().rev_id()
    }

    pub fn is_enabled(&self) -> bool {
        self.mmio.as_ptr().config().read().enable_cnf()
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.mmio.as_mut_ptr().config().update(|mut reg| {
            reg.set_enable_cnf(enable);
            reg
        });
    }

    /// Note that if the HPET doesn't support 64-bit mode, then the maximum value returned by this function will be `u32::MAX`.
    pub fn main_counter_value(&self) -> u64 {
        self.mmio.as_ptr().main_counter_value_register().read()
    }

    /// **Note**: you are not allowed to write to the main counter register while the HPET is enabled.
    pub fn set_main_counter_value(&mut self, main_counter_value: u64) {
        if self.is_enabled() {
            panic!("Tried to set the main counter value while the HPET was enabled");
        }
        self.mmio
            .as_mut_ptr()
            .main_counter_value_register()
            .write(main_counter_value);
    }

    pub fn get_legacy_replacement_enabled(&self) -> bool {
        self.mmio.as_ptr().config().read().legacy_replacement_cnf()
    }

    pub fn set_legacy_replacement_enabled(&mut self, enabled: bool) {
        self.mmio
            .as_mut_ptr()
            .config()
            .update(|reg| reg.with_legacy_replacement_cnf(enabled));
    }

    pub fn timers(&self) -> HpetTimersIterator<'_> {
        HpetTimersIterator {
            mmio: self,
            index: 0,
        }
    }

    pub fn timer(&self, index: u8) -> HpetTimer<'_> {
        if index >= self.timers_count() {
            panic!("Tried to access timer {index}, which is not supported by this HPET");
        }
        HpetTimer { hpet: self, index }
    }

    pub fn timer_mut<'a>(&'a mut self, index: u8) -> HpetTimerMut<'a> {
        if index >= self.timers_count() {
            panic!("Tried to access timer {index}, which is not supported by this HPET");
        }
        HpetTimerMut {
            hpet: self.mmio.borrow_mut(),
            index,
        }
    }

    /// Returns a bit map where 1 means interrupt pending.
    /// For level interrupts, it's cleared by writing a 1 (currently unimplemented in this libraray).
    /// For edge interrupts you don't clear it.
    pub fn pending_interrupts(&self) -> u32 {
        self.mmio
            .borrow()
            .as_ptr()
            .interrupt_status()
            .read()
            .t_n_int_sts()
    }
}

impl Debug for Hpet<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HPET")
            .field("Supports 64-bit", &self.supports_64_bit_mode())
            .field("Tick Period (10^-15 s)", &self.main_counter_tick_period())
            .field("Counter Value", &self.main_counter_value())
            .field("Enabled", &self.is_enabled())
            .field_with("Timers", |f| f.debug_list().entries(self.timers()).finish())
            .finish()
    }
}

pub struct HpetTimersIterator<'a> {
    mmio: &'a Hpet<'a>,
    index: u8,
}

impl<'a> Iterator for HpetTimersIterator<'a> {
    type Item = HpetTimer<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.mmio.timers_count() {
            let hpet_timer = HpetTimer {
                hpet: self.mmio,
                index: self.index,
            };
            self.index += 1;
            Some(hpet_timer)
        } else {
            None
        }
    }
}

pub struct HpetTimer<'a> {
    hpet: &'a Hpet<'a>,
    index: u8,
}

impl Debug for HpetTimer<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HPET Timer")
            .field("Supports 64-bit mode", &self.supports_64_bit_mode())
            .field("Supports FSB Interrupts", &self.supports_fsb_interrupts())
            .field("Supports Periodic Mode", &self.supports_periodic_mode())
            .field(
                "Supported I/O APIC Interrupts",
                &format_args!("{:b}", self.supported_io_apic_interrupts()),
            )
            .finish()
    }
}

impl HpetTimerRef for HpetTimer<'_> {
    #[allow(private_interfaces)]
    fn hpet_timer(&self) -> VolatilePtr<'_, HpetTimerMemory, ReadOnly> {
        self.hpet
            .mmio
            .as_ptr()
            .timers()
            .as_slice()
            .index(self.index as usize)
    }
}

pub struct HpetTimerMut<'a> {
    hpet: VolatileRef<'a, HpetMemory>,
    index: u8,
}

impl HpetTimerRef for HpetTimerMut<'_> {
    #[allow(private_interfaces)]
    fn hpet_timer(&self) -> VolatilePtr<'_, HpetTimerMemory, ReadOnly> {
        self.hpet
            .as_ptr()
            .timers()
            .as_slice()
            .index(self.index as usize)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InterruptConfig {
    /// Interrupts are sent through an I/O APIC, which can then route that interrupt to a Local APIC.
    IoApic(u5),
    /// Interrupts are directly sent to a Local APIC
    Fsb(TimerNFsbIntRouteReg),
}

impl HpetTimerMut<'_> {
    fn timer_mut(&mut self) -> VolatilePtr<'_, HpetTimerMemory> {
        self.hpet
            .as_mut_ptr()
            .timers()
            .as_slice()
            .index(self.index as usize)
    }

    /// **Note**
    /// - Not all I/O APIC irqs are guaranteed to be supported.
    /// - FSB is not guaranteed to be supported.
    pub fn configure_interrupt(&mut self, interrupt_config: InterruptConfig) {
        match interrupt_config {
            InterruptConfig::IoApic(irq) => {
                self.timer_mut()
                    .configuration_and_capability_register()
                    .update(|reg| {
                        if reg.int_route_cap() & (1 << irq.value()) == 0 {
                            panic!("Unsupported IRQ");
                        }
                        reg.with_fsb_en_cnf(false).with_int_route_cnf(irq)
                    });
            }
            InterruptConfig::Fsb(fsb) => {
                self.timer_mut()
                    .configuration_and_capability_register()
                    .update(|reg| {
                        if !reg.fsb_int_supported() {
                            panic!("FSB interrupts not supported by this timer");
                        }
                        reg.with_fsb_en_cnf(true)
                    });
                self.timer_mut().fsb_interrupt_route_register().write(fsb);
            }
        }
    }

    pub fn set_interrupt_enable(&mut self, enable: bool) {
        self.timer_mut()
            .configuration_and_capability_register()
            .update(|reg| reg.with_int_enable(enable));
    }

    pub fn set_comparator_value(&mut self, comparator_value: u64) {
        self.timer_mut()
            .comparator_register()
            .write(comparator_value);
    }

    pub fn set_trigger(&mut self, trigger: InterruptTrigger) {
        self.timer_mut()
            .configuration_and_capability_register()
            .update(|reg| {
                reg.with_int_type_cnf(match trigger {
                    InterruptTrigger::Edge => false,
                    InterruptTrigger::Level => true,
                })
            });
    }

    pub fn set_mode(&mut self, interrupt_type: TimerMode) {
        self.timer_mut()
            .configuration_and_capability_register()
            .update(|reg| {
                reg.with__type_cnf(match interrupt_type {
                    TimerMode::Oneshot => false,
                    TimerMode::Periodic => true,
                })
            });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptTrigger {
    Level,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimerMode {
    Oneshot,
    Periodic,
}

pub trait HpetTimerRef {
    #[allow(private_interfaces)]
    fn hpet_timer(&self) -> VolatilePtr<'_, HpetTimerMemory, ReadOnly>;

    fn supported_io_apic_interrupts(&self) -> u32 {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .int_route_cap()
    }

    fn supports_fsb_interrupts(&self) -> bool {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .fsb_int_supported()
    }

    fn supports_64_bit_mode(&self) -> bool {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .size_cap()
    }

    fn supports_periodic_mode(&self) -> bool {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .periodic_mode_supported()
    }

    fn interrupt_cfg(&self) -> InterruptConfig {
        if self
            .hpet_timer()
            .configuration_and_capability_register()
            .read()
            .fsb_en_cnf()
        {
            InterruptConfig::Fsb(self.hpet_timer().fsb_interrupt_route_register().read())
        } else {
            InterruptConfig::IoApic(
                self.hpet_timer()
                    .configuration_and_capability_register()
                    .read()
                    .int_route_cnf(),
            )
        }
    }

    fn comparator_value(&self) -> u64 {
        self.hpet_timer().comparator_register().read()
    }
}
