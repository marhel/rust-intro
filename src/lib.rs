pub mod constants;

struct FakeCore {
    mask: u8,
    interrupt_return_stack: Vec<u8>,
    vector: Option<u8>
}
const UNINITIALIZED_INTERRUPT: u8 = 0x0F;
const SPURIOUS_INTERRUPT: u8 = 0x18;
const AUTOVECTOR_BASE: u8 = 0x18;

impl FakeCore {
    fn return_from_interrupt(&mut self) {
        self.mask = self.interrupt_return_stack.pop().unwrap();
    }
    fn process_interrupt(&mut self, int_ctrl: &mut InterruptController) {
        let prio = int_ctrl.highest_priority();
        self.vector = if prio > self.mask {
            self.interrupt_return_stack.push(self.mask);
            self.mask = prio;
            int_ctrl.acknowledge_interrupt(prio).or(Some(SPURIOUS_INTERRUPT))
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

    fn assert_auto(int_ctrl: &mut InterruptController, core: &mut FakeCore, prio: u8) {
        assert_next(int_ctrl, core, prio, if prio > 0 {Some(AUTOVECTOR_BASE + prio)} else {None})
    }

    fn assert_next(int_ctrl: &mut InterruptController, core: &mut FakeCore, prio: u8, vector: Option<u8>) {
        assert_eq!(prio, int_ctrl.highest_priority());
        // assume still in higher priority handler
        core.process_interrupt(int_ctrl);
        assert_eq!(None, core.vector);
        // return from higher priority handler, allow lower prio interrupts
        core.return_from_interrupt();
        core.process_interrupt(int_ctrl);
        assert_eq!(vector, core.vector);
    }

    #[test]
    fn highest_priority_is_processed_first() {
        let rtc_vector = 64;
        let rtc = Peripheral::vectored(7, rtc_vector);
        let disk = Peripheral::autovectored(5);
        let keyboard = Peripheral::vectored_uninitialized(2);

        let mut int_ctrl = PeriperhalInterruptController {
            highest_priority: 0, 
            asserted: [None, None, None, None, None, None, None]
        };
        int_ctrl.request_interrupt(&rtc);
        int_ctrl.request_interrupt(&keyboard);
        int_ctrl.request_interrupt(&disk);

        let mut core = FakeCore { interrupt_return_stack: Vec::new(), mask: 0, vector: None };

        assert_eq!(7, int_ctrl.highest_priority());
        core.process_interrupt(&mut int_ctrl);
        assert_eq!(Some(rtc_vector), core.vector);

        assert_auto(&mut int_ctrl, &mut core, 5);
        assert_next(&mut int_ctrl, &mut core, 2, Some(UNINITIALIZED_INTERRUPT));
        assert_auto(&mut int_ctrl, &mut core, 0);
    }

    #[test]
    fn auto_controller() {
        let mut auto_ctrl = AutoInterruptController { level: 0 };
        auto_ctrl.request_interrupt(2);
        auto_ctrl.request_interrupt(7);
        auto_ctrl.request_interrupt(5);
        let mut core = FakeCore { interrupt_return_stack: Vec::new(), mask: 0, vector: None };

        assert_eq!(7, auto_ctrl.highest_priority());
        core.process_interrupt(&mut auto_ctrl);
        assert_eq!(Some(AUTOVECTOR_BASE + 7), core.vector);

        assert_auto(&mut auto_ctrl, &mut core, 5);
        assert_auto(&mut auto_ctrl, &mut core, 2);
        assert_auto(&mut auto_ctrl, &mut core, 0);
    }
}
