#![allow(dead_code)]
pub mod constants;

struct FakeCore<T: InterruptController> {
    irq_level: u8,
    irq_mask: u8,
    int_ctrl: T,
    interrupt_return_stack: Vec<u8>,
    vector: Option<u8>
}
const UNINITIALIZED_INTERRUPT: u8 = 0x0F;
const SPURIOUS_INTERRUPT: u8 = 0x18;
const AUTOVECTOR_BASE: u8 = 0x18;

impl<T: InterruptController> FakeCore<T> {
    fn new(irq_mask: u8, irq_level: u8, int_ctrl: T) -> FakeCore<T> {
        FakeCore {
            irq_level: irq_level,
            irq_mask: irq_mask,
            int_ctrl: int_ctrl,
            interrupt_return_stack: Vec::new(),
            vector: None
        }
    }
    fn return_from_interrupt(&mut self) {
        self.irq_mask = self.interrupt_return_stack.pop().unwrap();
    }
    fn process_interrupt(&mut self) {
        let old_level = self.irq_level;
        self.irq_level = self.int_ctrl.highest_priority();
        let edge_triggered_nmi = old_level != 7 && self.irq_level == 7;
        self.vector = if self.irq_level > self.irq_mask || edge_triggered_nmi {
            self.interrupt_return_stack.push(self.irq_mask);
            self.irq_mask = self.irq_level;
            self.int_ctrl.acknowledge_interrupt(self.irq_level).or(Some(SPURIOUS_INTERRUPT))
        } else {
            None
        }
    }
}
trait InterruptController
{
    fn highest_priority(&self) -> u8;
    fn acknowledge_interrupt(&mut self, priority: u8) -> Option<u8>;
}

struct PeriperhalInterruptController<'a>
{
    highest_priority: u8,
    asserted: [Option<&'a Peripheral>; 7]
}

fn priority_to_index(priority: u8) -> usize {
    7 - priority as usize
}
impl<'a> PeriperhalInterruptController<'a> {
    fn update_asserted(&mut self, index: usize, value: Option<&'a Peripheral>) -> u8 {
        self.asserted[index] = value;
        self.highest_priority = self.asserted.iter().position(|&x| x.is_some()).map(|i| 7-i as u8).unwrap_or(0u8);
        self.highest_priority
    }
    fn request_interrupt(&mut self, p: &'a Peripheral) -> u8
    {
        self.update_asserted(priority_to_index(p.priority), Some(p))
    }
}

impl<'a> InterruptController for PeriperhalInterruptController<'a> {
    fn highest_priority(&self) -> u8 {
        self.highest_priority
    }

    fn acknowledge_interrupt(&mut self, priority: u8) -> Option<u8> {
        let ip = priority_to_index(priority);
        self.asserted[ip].map(|peripheral|
                {
                    self.update_asserted(ip, None);
                    // use provided vector, or handle as auto vectored or uninitialized vector
                    peripheral.vector.unwrap_or_else(|| if peripheral.autovectored {AUTOVECTOR_BASE + priority} else {UNINITIALIZED_INTERRUPT})
                }
            )
    }
}

struct Peripheral
{
    priority: u8,
    autovectored: bool,
    vector: Option<u8>
}
impl Peripheral {
    fn vectored(priority: u8, vector: u8) -> Peripheral {
        assert!(priority > 0 && priority < 8);
        Peripheral {
            priority: priority,
            autovectored: false,
            vector: Some(vector)
        }
    }
    fn vectored_uninitialized(priority: u8) -> Peripheral {
        assert!(priority > 0 && priority < 8);
        Peripheral {
            priority: priority,
            autovectored: false,
            vector: None
        }
    }
    fn autovectored(priority: u8) -> Peripheral {
        assert!(priority > 0 && priority < 8);
        Peripheral {
            priority: priority,
            autovectored: true,
            vector: None
        }
    }
}

struct AutoInterruptController {
    level: u8
}
impl AutoInterruptController {
    fn request_interrupt(&mut self, irq: u8) -> u8
    {
        assert!(irq > 0 && irq < 8);
        self.level |= 1 << irq - 1;
        self.level
    }
}
impl InterruptController for AutoInterruptController {
    fn highest_priority(&self) -> u8 {
        (8 - self.level.leading_zeros()) as u8
    }

    fn acknowledge_interrupt(&mut self, priority: u8) -> Option<u8> {
        self.level &= !(1 << priority - 1);
        Some(AUTOVECTOR_BASE + priority)
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeCore, InterruptController, PeriperhalInterruptController, AutoInterruptController, Peripheral, 
        AUTOVECTOR_BASE, UNINITIALIZED_INTERRUPT};

    fn assert_auto<T: InterruptController>(core: &mut FakeCore<T>, prio: u8) {
        assert_next(core, prio, if prio > 0 {Some(AUTOVECTOR_BASE + prio)} else {None})
    }

    fn assert_next<T: InterruptController>(core: &mut FakeCore<T>, prio: u8, vector: Option<u8>) {
        assert_eq!(prio, core.int_ctrl.highest_priority());
        // assume still in higher priority handler
        core.process_interrupt();
        assert_eq!(None, core.vector);
        // return from higher priority handler, allow lower prio interrupts
        core.return_from_interrupt();
        core.process_interrupt();
        assert_eq!(vector, core.vector);
    }

    #[test]
    fn highest_priority_is_processed_first() {
        let rtc_vector = 64;
        let rtc = Peripheral::vectored(7, rtc_vector);
        let disk = Peripheral::autovectored(5);
        let keyboard = Peripheral::vectored_uninitialized(2);

        let int_ctrl = PeriperhalInterruptController {
            highest_priority: 0, 
            asserted: [None, None, None, None, None, None, None]
        };
        let mut core = FakeCore::new(0,0, int_ctrl);
        core.int_ctrl.request_interrupt(&rtc);
        core.int_ctrl.request_interrupt(&keyboard);
        core.int_ctrl.request_interrupt(&disk);

        assert_eq!(7, core.int_ctrl.highest_priority());
        core.process_interrupt();
        assert_eq!(Some(rtc_vector), core.vector);

        assert_auto(&mut core, 5);
        assert_next(&mut core, 2, Some(UNINITIALIZED_INTERRUPT));
        assert_auto(&mut core, 0);
    }

    #[test]
    fn auto_controller() {
        let auto_ctrl = AutoInterruptController { level: 0 };
        let mut core = FakeCore::new(0,0, auto_ctrl);
        core.int_ctrl.request_interrupt(2);
        core.int_ctrl.request_interrupt(7);
        core.int_ctrl.request_interrupt(5);

        assert_eq!(7, core.int_ctrl.highest_priority());
        core.process_interrupt();
        assert_eq!(Some(AUTOVECTOR_BASE + 7), core.vector);

        assert_auto(&mut core, 5);
        assert_auto(&mut core, 2);
        assert_auto(&mut core, 0);
    }

    #[test]
    fn maskable_interrupts() {
        let auto_ctrl = AutoInterruptController { level: 0 };
        let mut core = FakeCore::new(6,0, auto_ctrl);
        core.int_ctrl.request_interrupt(2);
        core.int_ctrl.request_interrupt(5);

        assert_eq!(5, core.int_ctrl.highest_priority());
        core.process_interrupt();
        assert_eq!(None, core.vector);
    }

    #[test]
    fn nonmaskable_interrupts() {
        let auto_ctrl = AutoInterruptController { level: 0 };
        let mut core = FakeCore::new(7,0, auto_ctrl);
        core.int_ctrl.request_interrupt(2);
        core.int_ctrl.request_interrupt(7);

        assert_eq!(7, core.int_ctrl.highest_priority());
        core.process_interrupt();
        assert_eq!(Some(AUTOVECTOR_BASE + 7), core.vector);
    }
    #[test]
    fn nonmaskable_interrupts_in_progress() {
        let auto_ctrl = AutoInterruptController { level: 0 };
        let mut core = FakeCore::new(7,7, auto_ctrl);
        core.int_ctrl.request_interrupt(2);
        core.int_ctrl.request_interrupt(7);

        assert_eq!(7, core.int_ctrl.highest_priority());
        core.process_interrupt();
        assert_eq!(None, core.vector);
    }
}
