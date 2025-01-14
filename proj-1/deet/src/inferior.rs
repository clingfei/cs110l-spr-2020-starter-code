use nix::sys::ptrace;
use nix::sys::signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::process::{Child, Command};
use std::os::unix::process::CommandExt;
use std::mem::size_of;
use std::collections::HashMap;
use crate::debugger::BreakPoint;
use crate::dwarf_data::DwarfData;

pub enum Status {
    /// Indicates inferior stopped. Contains the signal that stopped the process, as well as the
    /// current instruction pointer that it is stopped at.
    Stopped(signal::Signal, usize),

    /// Indicates inferior exited normally. Contains the exit status code.
    Exited(i32),

    /// Indicates the inferior exited due to a signal. Contains the signal that killed the
    /// process.
    Signaled(signal::Signal),
}

/// This function calls ptrace with PTRACE_TRACEME to enable debugging on a process. You should use
/// pre_exec with Command to call this in the child process.
fn child_traceme() -> Result<(), std::io::Error> {
    ptrace::traceme().or(Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "ptrace TRACEME failed",
    )))
}

pub struct Inferior {
    child: Child,
}



fn align_addr_to_word(addr: usize) -> usize {
    addr & (-(size_of::<usize>() as isize) as usize)
}

impl Inferior {
    /// Attempts to start a new inferior process. Returns Some(Inferior) if successful, or None if
    /// an error is encountered.
    pub fn new(target: &str, args: &Vec<String>, breakpoints: &HashMap<usize, BreakPoint>) -> Option<Inferior> {
        // TODO: implement me!
        // println!(
        //     "Inferior::new not implemented! target={}, args={:?}",
        //     target, args
        // );
        let mut command = Command::new(target);
        command.args(args);
        unsafe{
            command.pre_exec(child_traceme);
        }
        let child = command.spawn().ok()?;
        let mut inferior = Inferior{child};
        for (breakpoint, _) in breakpoints {
            inferior.install_breakpoints(*breakpoint);
        }
        Some(inferior)
    }

    /// Returns the pid of this inferior.
    pub fn pid(&self) -> Pid {
        nix::unistd::Pid::from_raw(self.child.id() as i32)
    }

    /// Calls waitpid on this inferior and returns a Status to indicate the state of the process
    /// after the waitpid call.
    pub fn wait(&self, options: Option<WaitPidFlag>) -> Result<Status, nix::Error> {
        Ok(match waitpid(self.pid(), options)? {
            WaitStatus::Exited(_pid, exit_code) => Status::Exited(exit_code),
            WaitStatus::Signaled(_pid, signal, _core_dumped) => Status::Signaled(signal),
            WaitStatus::Stopped(_pid, signal) => {
                let regs = ptrace::getregs(self.pid())?;
                Status::Stopped(signal, regs.rip as usize)
            }
            other => panic!("waitpid returned unexpected status: {:?}", other),
        })
    }

    pub fn continue_run(&self, signal: Option<signal::Signal>) -> Result<Status, nix::Error> {
        ptrace::cont(self.pid(), None)?;
        self.wait(None)
    }

    pub fn kill(&mut self) {  
        match self.child.kill().ok() {
            Some(_) => {
                println!("Killing running inferior (pid {})", self.pid());
                self.wait(None).unwrap();
            },
            None => {} 
        }
    }

    pub fn print_backtrace(&self, debug_data: &DwarfData) -> Result<(), nix::Error> {
        match ptrace::getregs(self.pid()) {
            Ok(regs) => {
                // Ok(println!("%rip register: {:#x}", regs.rip))
                let mut instruction_ptr = regs.rip as usize;
                let mut base_ptr = regs.rbp as usize;
                loop {
                    let line = DwarfData::get_line_from_addr(debug_data, instruction_ptr).unwrap();
                    let function =  DwarfData::get_function_from_addr(debug_data, instruction_ptr).unwrap();
                    println!("{} ({})", function, line);
                    if function == "main" {
                        break;
                    }
                    instruction_ptr = ptrace::read(self.pid(), (base_ptr + 8) as ptrace::AddressType)? as usize;
                    base_ptr = ptrace::read(self.pid(), base_ptr as ptrace::AddressType)? as usize;
                }
                Ok(())
            },
            Err(err) => {
                Err(err)
            }
        }
    }

    pub fn install_breakpoints(&mut self, breakpoint: usize) -> Result<u8, nix::Error> {
        self.write_byte(breakpoint, 0xcc)
    }

    pub fn get_previous_ins(&self) -> Result<usize, nix::Error> {
        let regs = ptrace::getregs(self.pid()).unwrap();
        Ok(regs.rip as usize - 1)
    }

    pub fn write_byte(&mut self, addr: usize, val: u8) -> Result<u8, nix::Error> {
        let aligned_addr = align_addr_to_word(addr);
        let byte_offset = addr - aligned_addr;
        let word = ptrace::read(self.pid(), aligned_addr as ptrace::AddressType)? as u64;
        let orig_byte = (word >> 8 * byte_offset) & 0xff;
        let masked_word = word & !(0xff << 8 * byte_offset);
        let updated_word = masked_word | ((val as u64) << 8 * byte_offset);
        ptrace::write(
            self.pid(),
            aligned_addr as ptrace::AddressType,
            updated_word as *mut std::ffi::c_void,
        )?;
        Ok(orig_byte as u8)
    }

    pub fn step_breakpoint(&mut self, rip: usize, orin_byte: u8) -> bool {
        // restore instruction
        self.write_byte(rip, orin_byte).unwrap();
        // rewind rip to the stopped instruction
        let mut regs = ptrace::getregs(self.pid()).unwrap();
        regs.rip = rip as u64;
        ptrace::setregs(self.pid(), regs).unwrap();
        // step one the original instruction
        ptrace::step(self.pid(), None).unwrap();
        // restore the breakpoint and return to resume the normal execution
        match self.wait(None).unwrap() {
            Status::Stopped(s, _) if s == signal::Signal::SIGTRAP => {
                self.install_breakpoints(rip).unwrap();
                true
            }
            _ => false,
        }
    }
}
