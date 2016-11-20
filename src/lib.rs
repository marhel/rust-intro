pub mod constants;

struct Motherboard<'a> {
    int_ctrl: &'a mut InterruptController,
    core: Core,
}

struct Core {
    mask: u8,
    vector: Option<u8>
}
const SPURIOUS_INTERRUPT: u8 = 0x18;
impl Core {
    fn process_interrupt(&mut self, int_ctrl: &mut InterruptController) {
        let prio = int_ctrl.highest_priority();
        self.vector = if prio > self.mask {
            int_ctrl.acknowledge_interrupt(prio).or(Some(SPURIOUS_INTERRUPT))
            // Some(int_ctrl.acknowledge_interrupt(prio).unwrap_or(SPURIOUS_INTERRUPT))
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

struct VectoredInterruptController<'a>
{
    highest_priority: u8,
    asserted: [Option<&'a Peripheral>; 7]
}

fn priority_to_index(priority: u8) -> usize {
    7 - priority as usize
}
impl<'a> VectoredInterruptController<'a> {
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

impl<'a> InterruptController for VectoredInterruptController<'a> {
    fn highest_priority(&self) -> u8 {
        self.highest_priority
    }

    fn acknowledge_interrupt(&mut self, priority: u8) -> Option<u8> {
        let ip = priority_to_index(priority);
        self.asserted[ip].map(|peripheral|
                {
                    self.update_asserted(ip, None);
                    peripheral.vector
                }
            )
    }
}

struct Peripheral
{
    priority: u8,
    vector: u8
}

#[cfg(test)]
mod tests {
    use super::{Motherboard, Core, InterruptController, VectoredInterruptController, Peripheral};
    #[test]
    fn highest_priority_is_processed_first() {
        let rtc = Peripheral { priority : 7, vector: 12 };
        let disk = Peripheral { priority : 5, vector: 15 };
        let keyboard = Peripheral { priority : 2, vector: 17 };

        let mut int_ctrl = VectoredInterruptController {
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
        assert_eq!(Some(12), board.core.vector);

        assert_eq!(5, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(15), board.core.vector);

        assert_eq!(2, board.int_ctrl.highest_priority());
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(Some(17), board.core.vector);

        assert_eq!(0, board.int_ctrl.highest_priority());        
        board.core.process_interrupt(board.int_ctrl);
        assert_eq!(None, board.core.vector);
    }
}
