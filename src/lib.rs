pub mod constants;

struct Motherboard<'a> {
    ipe: InterruptPriorityEncoder<'a>,
    core: Core,
}

struct Core {
    mask: u8,
    vector: u8
}

impl Core {
    fn process_interrupt(&mut self, ipe: &mut InterruptPriorityEncoder) {
        let prio = ipe.highest_priority;
        if prio > self.mask {            
            self.vector = ipe.acknowledge_interrupt(prio);
        }        
    }
}

struct InterruptPriorityEncoder<'a>
{
    highest_priority: u8,
    asserted: [Option<&'a Peripheral>; 7]
}

fn priority_to_index(priority: u8) -> usize {
    7 - priority as usize
}

impl<'a> InterruptPriorityEncoder<'a> {
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
    use super::{Motherboard, Core, InterruptPriorityEncoder, Peripheral};
    #[test]
    fn highest_priority_is_processed_first() {
        let rtc = Peripheral { priority : 7, vector: 12 };
        let disk = Peripheral { priority : 5, vector: 15 };
        let keyboard = Peripheral { priority : 2, vector: 17 };

        let ipe = InterruptPriorityEncoder {
            highest_priority: 0, 
            asserted: [None, None, None, None, None, None, None]
        };
        let core = Core { mask: 0, vector: 0 };
        let mut board = Motherboard { ipe: ipe, core: core };

        board.ipe.assert_interrupt(&rtc);
        board.ipe.assert_interrupt(&keyboard);
        board.ipe.assert_interrupt(&disk);

        assert_eq!(7, board.ipe.highest_priority);
        board.core.process_interrupt(&mut board.ipe);
        assert_eq!(12, board.core.vector);

        assert_eq!(5, board.ipe.highest_priority);
        board.core.process_interrupt(&mut board.ipe);
        assert_eq!(15, board.core.vector);

        assert_eq!(2, board.ipe.highest_priority);
        board.core.process_interrupt(&mut board.ipe);
        assert_eq!(17, board.core.vector);

        assert_eq!(0, board.ipe.highest_priority);
    }
}
