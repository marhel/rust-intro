pub mod constants;

struct Motherboard<'a> {
    int_ctrl: InterruptController<'a>,
    core: Core,
}

struct Core {
    mask: u8,
    vector: Option<u8>
}

impl Core {
    fn process_interrupt(&mut self, int_ctrl: &mut InterruptController) {
        let prio = int_ctrl.highest_priority;
        self.vector = if prio > self.mask {            
            Some(int_ctrl.acknowledge_interrupt(prio))
        } else {
            None
        }
    }
}

struct InterruptController<'a>
{
    highest_priority: u8,
    asserted: [Option<&'a Peripheral>; 7]
}

fn priority_to_index(priority: u8) -> usize {
    7 - priority as usize
}

impl<'a> InterruptController<'a> {
    fn update_asserted(&mut self, index: usize, value: Option<&'a Peripheral>) -> u8 {
        self.asserted[index] = value;
        self.highest_priority = self.asserted.iter().position(|&x| x.is_some()).map(|i| 7-i as u8).unwrap_or(0u8);
        self.highest_priority
    }
    fn assert_interrupt(&mut self, p: &'a Peripheral) -> u8
    {
        self.update_asserted(priority_to_index(p.priority), Some(p))
    }
    fn acknowledge_interrupt(&mut self, priority: u8) -> u8 {
        let ip = priority_to_index(priority);
        match self.asserted[ip] {
            None => 0x18, // spurious interrupt
            Some(peripheral) => {
                self.update_asserted(ip, None);
                peripheral.vector
            }
        }
    }
}

struct Peripheral
{
    priority: u8,
    vector: u8
}

#[cfg(test)]
mod tests {
    use super::{Motherboard, Core, InterruptController, Peripheral};
    #[test]
    fn highest_priority_is_processed_first() {
        let rtc = Peripheral { priority : 7, vector: 12 };
        let disk = Peripheral { priority : 5, vector: 15 };
        let keyboard = Peripheral { priority : 2, vector: 17 };

        let int_ctrl = InterruptController {
            highest_priority: 0, 
            asserted: [None, None, None, None, None, None, None]
        };
        let core = Core { mask: 0, vector: None };
        let mut board = Motherboard { int_ctrl: int_ctrl, core: core };

        board.int_ctrl.assert_interrupt(&rtc);
        board.int_ctrl.assert_interrupt(&keyboard);
        board.int_ctrl.assert_interrupt(&disk);

        assert_eq!(7, board.int_ctrl.highest_priority);
        board.core.process_interrupt(&mut board.int_ctrl);
        assert_eq!(Some(12), board.core.vector);

        assert_eq!(5, board.int_ctrl.highest_priority);
        board.core.process_interrupt(&mut board.int_ctrl);
        assert_eq!(Some(15), board.core.vector);

        assert_eq!(2, board.int_ctrl.highest_priority);
        board.core.process_interrupt(&mut board.int_ctrl);
        assert_eq!(Some(17), board.core.vector);

        assert_eq!(0, board.int_ctrl.highest_priority);        
        board.core.process_interrupt(&mut board.int_ctrl);
        assert_eq!(None, board.core.vector);
    }
}
