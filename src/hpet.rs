use core::{fmt::Debug, num::NonZero, ptr::NonNull};

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
    pub unsafe fn new(addr: NonZero<usize>) -> Self {
        Self {
            mmio: {
                let pointer = NonNull::new(addr.get() as *mut HpetMemory).expect("ptr is not null");
                unsafe { VolatileRef::new(pointer) }
            },
        }
    }

    pub fn vendor_id(&self) -> u16 {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .get_vendor_id()
    }

    pub fn timers_count(&self) -> u8 {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .get_num_tim_cap()
            + 1
    }

    /// Get the main counter tick period in femtoseconds
    pub fn main_counter_tick_period(&self) -> u32 {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .get_counter_clk_period()
    }

    pub fn legacy_replacement_capable(&self) -> bool {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .get_leg_rt_cap()
    }

    pub fn supports_64_bit_mode(&self) -> bool {
        self.mmio
            .as_ptr()
            .capabilities_and_id()
            .read()
            .get_count_size_cap()
    }

    pub fn revision_id(&self) -> u8 {
        self.mmio.as_ptr().capabilities_and_id().read().get_rev_id()
    }

    pub fn get_enable(&self) -> bool {
        self.mmio.as_ptr().config().read().get_enable_cnf()
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
        if self.get_enable() {
            panic!("Tried to set the main counter value while the HPET was enabled");
        }
        self.mmio
            .as_mut_ptr()
            .main_counter_value_register()
            .write(main_counter_value);
    }

    pub fn get_legacy_replacement_enabled(&self) -> bool {
        self.mmio
            .as_ptr()
            .config()
            .read()
            .get_legacy_replacement_cnf()
    }

    pub fn timers(&self) -> HpetTimersIterator {
        HpetTimersIterator {
            mmio: self,
            index: 0,
        }
    }

    pub fn timer(&self, index: u8) -> HpetTimer {
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
}

impl Debug for Hpet<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HPET")
            .field("Supports 64-bit", &self.supports_64_bit_mode())
            .field("Tick Period (10^-15 s)", &self.main_counter_tick_period())
            .field("Counter Value", &self.main_counter_value())
            .field("Enabled", &self.get_enable())
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
    fn hpet_timer(&self) -> VolatilePtr<HpetTimerMemory, ReadOnly> {
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
    fn hpet_timer(&self) -> VolatilePtr<HpetTimerMemory, ReadOnly> {
        self.hpet
            .as_ptr()
            .timers()
            .as_slice()
            .index(self.index as usize)
    }
}

pub enum InterruptConfig {
    IoApic(u8),
    Fsb(TimerNFsbInterruptRouteRegister),
}

impl HpetTimerMut<'_> {
    fn timer_mut(&mut self) -> VolatilePtr<HpetTimerMemory> {
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
                    .update(|mut reg| {
                        reg.set_fsb_en_cnf(false);
                        if reg.get_int_route_cap() & (1 << irq) == 0 {
                            panic!("Unsupported IRQ");
                        }
                        reg.set_int_route_cnf(irq);
                        reg
                    });
            }
            InterruptConfig::Fsb(fsb) => {
                self.timer_mut()
                    .configuration_and_capability_register()
                    .update(|mut reg| {
                        if !reg.get_fsb_int_del_cap() {
                            panic!("FSB interrupts not supported by this timer");
                        }
                        reg.set_fsb_en_cnf(true);
                        reg
                    });
                self.timer_mut().fsb_interrupt_route_register().write(fsb);
            }
        }
    }

    pub fn set_interrupt_enable(&mut self, enable: bool) {
        self.timer_mut()
            .configuration_and_capability_register()
            .update(|mut reg| {
                reg.set_int_enb_cnf(enable);
                reg
            });
    }

    pub fn set_comparator_value(&mut self, comparator_value: u64) {
        self.timer_mut()
            .comparator_register()
            .write(comparator_value);
    }
}

pub trait HpetTimerRef {
    #[allow(private_interfaces)]
    fn hpet_timer(&self) -> VolatilePtr<HpetTimerMemory, ReadOnly>;

    fn supported_io_apic_interrupts(&self) -> u32 {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .get_int_route_cap()
    }

    fn supports_fsb_interrupts(&self) -> bool {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .get_fsb_int_del_cap()
    }

    fn supports_64_bit_mode(&self) -> bool {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .get_size_cap()
    }

    fn supports_periodic_mode(&self) -> bool {
        self.hpet_timer()
            .configuration_and_capability_register()
            .read()
            .get_per_int_cp()
    }

    fn interrupt_mode(&self) -> InterruptMode {
        if self
            .hpet_timer()
            .configuration_and_capability_register()
            .read()
            .get_fsb_en_cnf()
        {
            InterruptMode::Fsb
        } else {
            InterruptMode::IoApic
        }
    }
}

#[derive(Debug)]
pub enum InterruptMode {
    /// Interrupts are sent through an I/O APIC, which can then route that interrupt to a Local APIC.
    IoApic,
    /// Interrupts are directly sent to a Local APIC
    Fsb,
}
