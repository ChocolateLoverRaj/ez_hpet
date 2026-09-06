use core::{fmt::Debug, ptr::NonNull};

use arbitrary_int::u5;
use volatile::VolatilePtr;

use crate::*;

pub struct Hpet<'a> {
    mmio: VolatilePtr<'a, HpetMemory>,
}

// Safety: Hpet doesn't have a destructor so moving it between threads is safe.
unsafe impl Send for Hpet<'_> {}
// Safety: the HPET is safe to access concurrently from multiple threads.
unsafe impl Sync for Hpet<'_> {}

impl Hpet<'_> {
    /// To call this function:
    /// - Use the `acpi` crate to parse ACPI tables
    /// - Find the `HPET` table with `acpi::HpetInfo::new`
    /// - Find the physical base address of the HPET
    /// - Map the HPET, using [`HPET_MMIO_SIZE`]
    ///
    /// # Safety
    /// - The address must be a virtual address mapped to HPET memory as un-cacheable (UC).
    /// - This struct now has exclusive access to the HPET for the duration if its lifetime
    /// - This library lets you get mutable access to timer memory, including multiple mutable references to the same timer. It is the caller's responsibility to follow reference rules for each timer.
    pub unsafe fn new(ptr: NonNull<HpetMemory>) -> Self {
        Self {
            mmio: {
                // Safety: the pointer is valid and we have exclusive access to the HPET
                unsafe { VolatilePtr::new(ptr) }
            },
        }
    }

    pub fn vendor_id(&self) -> u16 {
        self.mmio.capabilities_and_id().read().vendor_id()
    }

    pub fn timers_count(&self) -> u8 {
        self.mmio.capabilities_and_id().read().num_tim_cap().value() + 1
    }

    /// Get the main counter tick period in femtoseconds
    pub fn main_counter_tick_period(&self) -> u32 {
        self.mmio.capabilities_and_id().read().counter_clk_period()
    }

    pub fn legacy_replacement_capable(&self) -> bool {
        self.mmio.capabilities_and_id().read().leg_rt_cap()
    }

    pub fn supports_64_bit_mode(&self) -> bool {
        self.mmio.capabilities_and_id().read().count_size_cap()
    }

    pub fn revision_id(&self) -> u8 {
        self.mmio.capabilities_and_id().read().rev_id()
    }

    pub fn is_enabled(&self) -> bool {
        self.mmio.config().read().enable_cnf()
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.mmio.config().update(|mut reg| {
            reg.set_enable_cnf(enable);
            reg
        });
    }

    /// Note that if the HPET doesn't support 64-bit mode, then the maximum value returned by this function will be `u32::MAX`.
    pub fn main_counter_value(&self) -> u64 {
        self.mmio.main_counter_value_register().read()
    }

    /// **Note**: you are not allowed to write to the main counter register while the HPET is enabled.
    pub fn set_main_counter_value(&mut self, main_counter_value: u64) {
        if self.is_enabled() {
            panic!("Tried to set the main counter value while the HPET was enabled");
        }
        self.mmio
            .main_counter_value_register()
            .write(main_counter_value);
    }

    pub fn get_legacy_replacement_enabled(&self) -> bool {
        self.mmio.config().read().legacy_replacement_cnf()
    }

    pub fn set_legacy_replacement_enabled(&mut self, enabled: bool) {
        self.mmio
            .config()
            .update(|reg| reg.with_legacy_replacement_cnf(enabled));
    }

    pub fn timers(&self) -> HpetTimersIterator<'_> {
        HpetTimersIterator {
            hpet: self,
            index: 0,
        }
    }

    pub fn timer(&self, index: u8) -> HpetTimerMut<'_> {
        if index >= self.timers_count() {
            panic!("Tried to access timer {index}, which is not supported by this HPET");
        }
        HpetTimerMut {
            hpet: self.mmio,
            index,
        }
    }

    /// Returns a bit map where 1 means interrupt pending.
    /// For level interrupts, it's cleared by writing a 1 (currently unimplemented in this libraray).
    /// For edge interrupts you don't clear it.
    pub fn pending_interrupts(&self) -> u32 {
        self.mmio.interrupt_status().read().t_n_int_sts()
    }

    /// Level interrupts are cleared by writing 1 to the bit corresponding to the timer.
    /// This function takes in the u32 bit mask of which interrupts you want to clear
    pub fn clear_interrutps(&mut self, interrupts_mask: u32) {
        self.mmio
            .interrupt_status()
            .update(|reg| reg.with_t_n_int_sts(interrupts_mask));
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
    hpet: &'a Hpet<'a>,
    index: u8,
}

impl<'a> Iterator for HpetTimersIterator<'a> {
    type Item = HpetTimerMut<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.hpet.timers_count() {
            let hpet_timer = HpetTimerMut {
                hpet: self.hpet.mmio,
                index: self.index,
            };
            self.index += 1;
            Some(hpet_timer)
        } else {
            None
        }
    }
}

impl Debug for HpetTimerMut<'_> {
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

pub struct HpetTimerMut<'a> {
    hpet: VolatilePtr<'a, HpetMemory>,
    index: u8,
}

impl HpetTimerMut<'_> {
    fn timer_mem(&self) -> VolatilePtr<'_, HpetTimerMemory> {
        self.hpet.timers().as_slice().index(self.index as usize)
    }

    /// **Note**
    /// - Not all I/O APIC irqs are guaranteed to be supported.
    /// - FSB is not guaranteed to be supported.
    pub fn configure_interrupt(&mut self, interrupt_config: InterruptConfig) {
        match interrupt_config {
            InterruptConfig::LegacyReplacment { trigger } => {
                self.timer_mem()
                    .configuration_and_capability_register()
                    .update(|reg| reg.with_int_type_cnf(trigger.into()));
            }
            InterruptConfig::IoApic {
                io_apic_irq,
                trigger,
            } => {
                self.timer_mem()
                    .configuration_and_capability_register()
                    .update(|reg| {
                        if reg.int_route_cap() & (1 << io_apic_irq.value()) == 0 {
                            panic!("Unsupported IRQ");
                        }
                        reg.with_fsb_en_cnf(false)
                            .with_int_route_cnf(io_apic_irq)
                            .with_int_type_cnf(trigger.into())
                    });
            }
            InterruptConfig::Fsb {
                destination_id,
                destination_mode,
                redirection_hint: redirection_int,
                interrupt_vector,
                delivery_mode,
            } => {
                self.timer_mem()
                    .configuration_and_capability_register()
                    .update(|reg| {
                        if !reg.fsb_int_supported() {
                            panic!("FSB interrupts not supported by this timer");
                        }
                        reg.with_fsb_en_cnf(true)
                    });
                self.timer_mem().fsb_interrupt_route_register().write(
                    TimerNFsbIntRouteReg::new_with_raw_value(0)
                        .with_fsb_int_addr(
                            FsbApicIntAddr::new_with_raw_value(0)
                                .with_destination_mode(destination_mode.into())
                                .with_destination_id(destination_id)
                                .with_redirection_hint(redirection_int.into())
                                .with_fixed_value(FsbApicIntAddr::APIC_FIXED_VALUE)
                                .raw_value(),
                        )
                        .with_fsb_int_val(
                            FsbApicIntValue::new_with_raw_value(0)
                                .with_delivery_mode(delivery_mode.into())
                                .with_trigger_mode(FsbIntTriggerMode::Edge.into())
                                .with_interrupt_vector(interrupt_vector)
                                .raw_value(),
                        ),
                );
            }
        }
    }

    pub fn set_interrupt_enable(&mut self, enable: bool) {
        self.timer_mem()
            .configuration_and_capability_register()
            .update(|reg| reg.with_int_enable(enable));
    }

    pub fn set_comparator_value(&mut self, comparator_value: u64) {
        self.timer_mem()
            .comparator_register()
            .write(comparator_value);
    }

    pub fn set_trigger(&mut self, trigger: InterruptTrigger) {
        self.timer_mem()
            .configuration_and_capability_register()
            .update(|reg| {
                reg.with_int_type_cnf(match trigger {
                    InterruptTrigger::Edge => false,
                    InterruptTrigger::Level => true,
                })
            });
    }

    pub fn set_mode(&mut self, interrupt_type: TimerMode) {
        self.timer_mem()
            .configuration_and_capability_register()
            .update(|reg| {
                reg.with__type_cnf(match interrupt_type {
                    TimerMode::Oneshot => false,
                    TimerMode::Periodic => true,
                })
            });
    }

    pub fn supported_io_apic_interrupts(&self) -> u32 {
        self.timer_mem()
            .configuration_and_capability_register()
            .read()
            .int_route_cap()
    }

    pub fn supports_fsb_interrupts(&self) -> bool {
        self.timer_mem()
            .configuration_and_capability_register()
            .read()
            .fsb_int_supported()
    }

    pub fn supports_64_bit_mode(&self) -> bool {
        self.timer_mem()
            .configuration_and_capability_register()
            .read()
            .size_cap()
    }

    pub fn supports_periodic_mode(&self) -> bool {
        self.timer_mem()
            .configuration_and_capability_register()
            .read()
            .periodic_mode_supported()
    }

    pub fn comparator_value(&self) -> u64 {
        self.timer_mem().comparator_register().read()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptTrigger {
    Level,
    Edge,
}
impl From<InterruptTrigger> for bool {
    fn from(value: InterruptTrigger) -> Self {
        match value {
            InterruptTrigger::Edge => false,
            InterruptTrigger::Level => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimerMode {
    Oneshot,
    Periodic,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyReplacementRoute {
    pub pic8259_mapping: u8,
    pub apic_mapping: u8,
}

/// Array with legacy routes for timer 0 (at index 0) and timer 1 (at index 1).
pub const LEGACY_REPLACEMENT_ROUTES: [LegacyReplacementRoute; 2] = [
    LegacyReplacementRoute {
        pic8259_mapping: 0,
        apic_mapping: 2,
    },
    LegacyReplacementRoute {
        pic8259_mapping: 8,
        apic_mapping: 8,
    },
];

/// Represents only valid combinations of interrupt configuration of an HPET timer.
#[derive(Debug, Clone, Copy)]
pub enum InterruptConfig {
    /// When legacy replacement is enabled (not supported on all machines), timer 0 is routed to I/O IRQ 2
    /// and timer 1 is routed to I/O IRQ 8. Other timers are still routed by specifying the number (choose
    /// one of the IRQs the timer supports).
    ///
    /// To use this option, you must specify timer 0 or 1 and legacy replacement must be enabled in the HPET's config.
    LegacyReplacment { trigger: InterruptTrigger },
    /// Interrupts are sent through an I/O APIC, which can then route that interrupt to a Local APIC.
    ///
    /// To use this option on timers 0 or 1, legacy replacement must be disabled for the HPET overall. You can always use this option on timers 2+.
    IoApic {
        io_apic_irq: u5,
        trigger: InterruptTrigger,
    },
    /// Interrupts are directly sent to a Local APIC.
    ///
    /// To use this option, the HPET must support FSB. Even when legacy replacement is enabled, you can override the interrupt route for timers 0 and 1 to use FSB instead. FSB interrupts are always edge triggered.
    Fsb {
        destination_mode: ApicDestMode,
        redirection_hint: RedirectionHint,
        destination_id: u8,
        interrupt_vector: u8,
        delivery_mode: DeliveryMode,
    },
}
