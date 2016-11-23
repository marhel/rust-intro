#![allow(dead_code)]
#![allow(unused_variables)]

use std::result;
pub type Result<T> = result::Result<T, Exception>;
pub type Handler = fn(&mut Core) -> Result<Cycles>;
pub type InstructionSet = Vec<Handler>;

pub const EXCEPTION_ADDRESS_ERROR: u8           =  3;
pub const EXCEPTION_ILLEGAL_INSTRUCTION: u8     =  4;
pub const EXCEPTION_ZERO_DIVIDE: u8             =  5;
pub const EXCEPTION_CHK: u8                     =  6;
pub const EXCEPTION_TRAPV: u8                   =  7;
pub const EXCEPTION_PRIVILEGE_VIOLATION: u8     =  8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cycles(i32);

use std::ops::{Sub, SubAssign};
impl Sub for Cycles {
    type Output = Cycles;

    fn sub(self, other: Cycles) -> Cycles {
        Cycles(self.0 - other.0)
    }
}
impl SubAssign for Cycles {
    fn sub_assign(&mut self, other: Cycles) {
        self.0 -= other.0;
    }
}

impl Cycles {
    fn any(self) -> bool {
        self.0 > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProcessingState {
    Normal,             // Executing instructions
    Group2Exception,    // TRAP(V), CHK, ZeroDivide
    Group1Exception,    // Trace, Interrupt, IllegalInstruction, PrivilegeViolation
    Group0Exception,    // AddressError, BusError, ExternalReset
    Stopped,            // Trace, Interrupt or ExternalReset needed to resume
    Halted,             // ExternalReset needed to resume
}

impl ProcessingState {
    // The processor is processing an instruction if it is in the normal
    // state or processing a group 2 exception; the processor is not
    // processing an instruction if it is processing a group 0 or a group 1
    // exception. This info goes into a Group0 stack frame
    fn instruction_processing(self) -> bool {
        match self {
            ProcessingState::Normal => true,
            ProcessingState::Group2Exception => true,
            _ => false
        }
    }
    fn running(self) -> bool {
        match self {
            ProcessingState::Stopped => false,
            ProcessingState::Halted => false,
            _ => true
        }
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct AddressSpace(Mode, Segment);

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
enum Segment {
    Program, Data
}
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
enum Mode {
    User, Supervisor
}

pub const SUPERVISOR_PROGRAM: AddressSpace = AddressSpace(Mode::Supervisor, Segment::Program);
pub const SUPERVISOR_DATA: AddressSpace = AddressSpace(Mode::Supervisor, Segment::Data);
pub const USER_PROGRAM: AddressSpace = AddressSpace(Mode::User, Segment::Program);
pub const USER_DATA: AddressSpace = AddressSpace(Mode::User, Segment::Data);

impl AddressSpace {
    pub fn fc(&self) -> u32 {
        match *self {
            USER_DATA => 1,
            USER_PROGRAM => 2,
            SUPERVISOR_DATA => 5,
            SUPERVISOR_PROGRAM => 6,
        }
    }
}
use std::fmt;
impl fmt::Debug for AddressSpace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AddressSpace(mode, segment) => write!(f, "[{:?}/{:?}]", mode, segment),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AccessType {Read, Write}

#[derive(Debug)]
pub enum Exception {
    AddressError { address: u32, access_type: AccessType, processing_state: ProcessingState, address_space: AddressSpace},
    IllegalInstruction(u16, u32), // ir, pc
    Trap(u8, i32),                // trap no, exception cycles
    PrivilegeViolation(u16, u32), // ir, pc
    UnimplementedInstruction(u16, u32, u8), // ir, pc, vector no
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Exception::AddressError {
                address, access_type, processing_state, address_space
                } => write!(f, "Address Error: {:?} {:?} at {:08x} during {:?} processing", access_type, address_space, address, processing_state),
            Exception::IllegalInstruction(ir, pc) => write!(f, "Illegal Instruction {:04x} at {:08x}", ir, pc),
            Exception::Trap(num, ea_cyc) => write!(f, "Trap: {:04x} (ea cyc {})", num, ea_cyc),
            Exception::PrivilegeViolation(ir, pc) => write!(f, "Privilege Violation {:04x} at {:08x}", ir, pc),
            Exception::UnimplementedInstruction(ir, pc, _) => write!(f, "Unimplemented Instruction {:04x} at {:08x}", ir, pc),
        }
    }
}

pub struct Core {
    ir: u16,
    pc: u32,
    s_flag: u32,
    processing_state: ProcessingState,
    ophandlers: InstructionSet,
}

impl Core {
    fn read_word(&self, space: AddressSpace, address: u32) -> u16 {
        // totally fake
        address as u16
    }
    pub fn read_imm_u16(&mut self) -> Result<u16> {
        let address_space = if self.s_flag != 0 {SUPERVISOR_PROGRAM} else {USER_PROGRAM};
        if self.pc & 1 > 0 {
            return Err(Exception::AddressError{address: self.pc, access_type: AccessType::Read, address_space: address_space, processing_state: self.processing_state})
        }
        let memory_content = self.read_word(address_space, self.pc);

        self.pc = self.pc.wrapping_add(2);
        Ok(memory_content)
    }

    pub fn execute(&mut self, cycles: i32) -> Cycles {
        let cycles = Cycles(cycles);
        let mut remaining_cycles = cycles;
        while remaining_cycles.any() && self.processing_state.running() {
            // Read an instruction from PC (increments PC by 2)
            let result = self.read_imm_u16().and_then(|opcode| {
                    self.ir = opcode;
                    // Call instruction handler to mutate Core accordingly
                    self.ophandlers[opcode as usize](self)
                });
            remaining_cycles -= match result {
                Ok(cycles_used) => cycles_used,
                Err(err) => {
                    println!("Exception {}", err);
                    match err {
                        Exception::AddressError { address, access_type, processing_state, address_space } =>
                            self.handle_address_error(address, access_type, processing_state, address_space),
                        Exception::IllegalInstruction(_, pc) =>
                            self.handle_illegal_instruction(pc),
                        Exception::UnimplementedInstruction(_, pc, vector) =>
                            self.handle_unimplemented_instruction(pc, vector),
                        Exception::Trap(num, ea_calculation_cycles) =>
                            self.handle_trap(num, ea_calculation_cycles),
                        Exception::PrivilegeViolation(_, pc) =>
                            self.handle_privilege_violation(pc),
                    }
                }
            };
        }
        if self.processing_state.running() {
            cycles - remaining_cycles
        } else {
            // if not running, consume all available cycles
            // including overconsumed cycles
            let adjust = if remaining_cycles.0 < 0 { remaining_cycles } else { Cycles(0) };
            cycles - adjust
        }
    }
    pub fn handle_address_error(&mut self, bad_address: u32, access_type: AccessType, processing_state: ProcessingState, address_space: AddressSpace) -> Cycles {
        self.handle_exception(ProcessingState::Group1Exception, bad_address, EXCEPTION_ADDRESS_ERROR, 50)
    }
    pub fn handle_unimplemented_instruction(&mut self, pc: u32, vector: u8) -> Cycles {
        self.handle_exception(ProcessingState::Group2Exception, pc, vector, 34)
    }
    pub fn handle_illegal_instruction(&mut self, pc: u32) -> Cycles {
        self.handle_exception(ProcessingState::Group1Exception, pc, EXCEPTION_ILLEGAL_INSTRUCTION, 34)
    }
    pub fn handle_privilege_violation(&mut self, pc: u32) -> Cycles {
        self.handle_exception(ProcessingState::Group1Exception, pc, EXCEPTION_PRIVILEGE_VIOLATION, 34)
    }
    pub fn handle_trap(&mut self, trap: u8, cycles: i32) -> Cycles {
        let pc = self.pc;
        self.handle_exception(ProcessingState::Group2Exception, pc, trap, cycles)
    }

    pub fn handle_exception(&mut self, new_state: ProcessingState, pc: u32, vector: u8, cycles: i32) -> Cycles {
        self.processing_state = new_state;
        // completely fake
        self.pc = (vector * 4) as u32;
        Cycles(cycles)
    }
}


#[cfg(test)]
mod tests {
    use super::{Core, Cycles, Result, InstructionSet};
    use super::Exception::*;
    use super::ProcessingState;
    pub fn illegal_instruction(core: &mut Core) -> Result<Cycles> {
        let illegal_exception = IllegalInstruction(core.ir, core.pc.wrapping_sub(2));
        // println!("Exception: {}", illegal_exception);
        Err(illegal_exception)
    }

    pub fn jump_away(core: &mut Core) -> Result<Cycles> {
        core.pc = 0xbad;
        Ok(Cycles(20))
    }

    pub fn jump_home(core: &mut Core) -> Result<Cycles> {
        core.pc = 0x0;
        Ok(Cycles(16))
    }

    fn fake_instructions() -> InstructionSet
    {
        // Covers all possible IR values (64k entries)
        let mut handler: InstructionSet = Vec::with_capacity(0x10000);
        for _ in 0..0x10000 { handler.push(illegal_instruction); }
        handler[0x0] = jump_away;
        handler[0xc] = jump_home;
        // and so on....
        handler
    }

    #[test]
    fn example_cpu_works() {
        let mut f10c = Core { ir:0, pc: 0, ophandlers: fake_instructions(), processing_state: ProcessingState::Normal, s_flag: 0};

        // execute at least 10 cycles
        let actual_cycles = f10c.execute(10);
        assert_eq!(Cycles(20), actual_cycles);
        assert_eq!(0xbad, f10c.pc);

        let actual_cycles = f10c.execute(10);
        assert_eq!(Cycles(50), actual_cycles);
        assert_eq!(0x0c, f10c.pc);

        let actual_cycles = f10c.execute(10);
        assert_eq!(Cycles(16), actual_cycles);
        assert_eq!(0x00, f10c.pc);

        let ten_laps = (20 + 50 + 16) * 10;
        let actual_cycles = f10c.execute(ten_laps);
        assert_eq!(Cycles(ten_laps), actual_cycles);
        assert_eq!(0x00, f10c.pc);
    }
}
