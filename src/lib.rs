pub mod constants;

struct Motherboard<'a> {
    int_ctrl: &'a mut InterruptController,
    core: Core,
}

struct Core {
    mask: u8,
    vector: Option<u8>
}
const UNINITIALIZED_INTERRUPT: u8 = 0x0F;
const SPURIOUS_INTERRUPT: u8 = 0x18;
const AUTOVECTOR_BASE: u8 = 0x18;

impl Core {
    fn process_interrupt(&mut self, int_ctrl: &mut InterruptController) {
        let prio = int_ctrl.highest_priority();
        self.vector = if prio > self.mask {
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
    fn assert_interrupt(&mut self, p: &'a Peripheral) -> u8
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
#[cfg(test)]
mod tests {
    use super::{Motherboard, Core, InterruptController, PeriperhalInterruptController, Peripheral, 
        AUTOVECTOR_BASE, UNINITIALIZED_INTERRUPT};

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
        int_ctrl.assert_interrupt(&rtc);
        int_ctrl.assert_interrupt(&keyboard);
        int_ctrl.assert_interrupt(&disk);

        let core = Core { mask: 0, vector: None };
        let mut board = Motherboard { int_ctrl: &mut int_ctrl, core: core };

        assert_eq!(7, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(rtc_vector), board.core.vector);

        assert_eq!(5, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(AUTOVECTOR_BASE + disk.priority), board.core.vector);

        assert_eq!(2, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(UNINITIALIZED_INTERRUPT), board.core.vector);

        assert_eq!(0, board.int_ctrl.highest_priority());        
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(None, board.core.vector);
    }

    struct FakeInterruptController {
        level: u8
    }
    impl FakeInterruptController {
        fn assert_interrupt(&mut self, irq: u8) -> u8
        {
            self.level |= 1 << irq - 1;
            self.level
        }
    }
    impl InterruptController for FakeInterruptController {
        fn highest_priority(&self) -> u8 {
            (8 - self.level.leading_zeros()) as u8
        }

        fn acknowledge_interrupt(&mut self, priority: u8) -> Option<u8> {
            self.level &= !(1 << priority - 1);
            println!("{:b}, {}", self.level, priority);
            Some(AUTOVECTOR_BASE + priority)
        }
    }

    #[test]
    fn fake_controller() {
        let mut test_ctrl = FakeInterruptController { level: 0 };
        test_ctrl.assert_interrupt(2);
        test_ctrl.assert_interrupt(7);
        test_ctrl.assert_interrupt(5);
        let core = Core { mask: 0, vector: None };
        let mut board = Motherboard { int_ctrl: &mut test_ctrl, core: core };

        assert_eq!(7, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(AUTOVECTOR_BASE + 7), board.core.vector);

        assert_eq!(5, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(AUTOVECTOR_BASE + 5), board.core.vector);

        assert_eq!(2, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(AUTOVECTOR_BASE + 2), board.core.vector);

        assert_eq!(0, board.int_ctrl.highest_priority());        
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(None, board.core.vector);

    }
}
