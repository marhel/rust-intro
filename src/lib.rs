pub mod constants;

struct Motherboard<'a> {
    ipe: InterruptPriorityEncoder<'a>,
    mask: u8,
    vector: u8
}

struct Core {
    mask: u8,
    vector: u8
}

impl<'a> Motherboard<'a> {
    fn interrupt_requested_from(&mut self, p: &'a Peripheral)
    {
        self.ipe.assert_interrupt(p);
        let prio = self.ipe.highest_priority;
        if prio > self.mask {            
            self.vector = self.ipe.acknowledge_interrupt(prio);
        }
    }
}

struct InterruptPriorityEncoder<'a>
{
    highest_priority: u8,
    asserted: [Option<&'a Peripheral>; 7]
}

impl<'a> InterruptPriorityEncoder<'a> {
    fn assert_interrupt(&mut self, p: &'a Peripheral)
    {
        self.asserted[7 - p.priority as usize] = Some(p);
        //self.highest_priority = self.asserted.iter().filter_map(|&x| x).map(|x| x.priority).max().unwrap_or(0u8);
        self.highest_priority = self.asserted.iter().enumerate().find(|&(i, &x)| x.is_some()).map(|(i, &x)| 7-i as u8).unwrap_or(0u8);
    }
    fn acknowledge_interrupt(&mut self, priority: u8) -> u8 {
        let ip = 7 - priority as usize;
        match self.asserted[ip] {
            None => 0x18, // spurious interrupt
            Some(peripheral) => {
                self.asserted[ip] = None;
                self.highest_priority = self.asserted.iter().enumerate().find(|&(i, &x)| x.is_some()).map(|(i, &x)| 7-i as u8).unwrap_or(0u8);
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
    use super::{Motherboard, InterruptPriorityEncoder, Peripheral};
    #[test]
    fn highest_priority_is_set() {
        println!("Hello");
        let disk = Peripheral { priority : 5, vector: 15 };
        let keyboard = Peripheral { priority : 2, vector: 17 };
        let ipe = InterruptPriorityEncoder {
            highest_priority: 0, 
            asserted: [None, None, None, None, None, None, None]
        };
        let mut board = Motherboard { mask: 0, ipe: ipe, vector: 0 };
        board.ipe.assert_interrupt(&disk);
        board.ipe.assert_interrupt(&keyboard);
        assert_eq!(5, board.ipe.highest_priority);
    }
}
