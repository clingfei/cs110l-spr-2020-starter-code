use crate::debugger_command::DebuggerCommand;
use crate::inferior::{Inferior, Status};
use crate::dwarf_data::{DwarfData, Error as DwarfError};
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct BreakPoint {
    id: usize,
    addr: usize,
    orig_byte: u8,
}

impl BreakPoint {
    fn new(id: usize, addr: usize) -> Self {
        BreakPoint {
            id,
            addr,
            orig_byte: 0,
        }
    }

    pub fn addr(&self) -> usize {
        self.addr
    }

    pub fn set_byte(&mut self, orig_byte: u8) {
        self.orig_byte = orig_byte
    }
}

impl std::fmt::Display for BreakPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ID: {} ", self.id).unwrap();
        write!(f, "ADDR: {:#x} ", self.addr).unwrap();
        write!(f, "ORIN_BYTE: {} ", self.orig_byte)
    }
}

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<()>,
    inferior: Option<Inferior>,
    debug_data: DwarfData,
    breakpoints: HashMap<usize, BreakPoint>,
}

enum BreakPointType<'a> {
    Raw(&'a str),
    Line(usize),
    Func(&'a str),
}

fn get_breakpoint_type(breakpoint: &str) -> BreakPointType {
    if breakpoint.starts_with('*') {
        return BreakPointType::Raw(&breakpoint[1..]);
    }
    match usize::from_str_radix(breakpoint, 10) {
        Ok(line) => BreakPointType::Line(line),
        Err(_) => BreakPointType::Func(breakpoint),
    }
}

impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        // TODO (milestone 3): initialize the DwarfData
        let debug_data = match DwarfData::from_file(target) {
            Ok(val) => val,
            Err(DwarfError::ErrorOpeningFile) => {
                println!("Could not open file {}", target);
                std::process::exit(1);
            }
            Err(DwarfError::DwarfFormatError(err)) => {
                println!("Could not debugging symbols from {}: {:?}", target, err);
                std::process::exit(1);
            }
        };
        debug_data.print();

        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<()>::new();
        // Attempt to load history from ~/.deet_history if it exists
        let _ = readline.load_history(&history_path);

        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            inferior: None,
            debug_data: debug_data,
            breakpoints: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {
                DebuggerCommand::Run(args) => {
                    if let Some(inferior) = Inferior::new(&self.target, &args, &self.breakpoints) {
                        // Create the inferior
                        self.inferior = Some(inferior);
                        // TODO (milestone 1): make the inferior run
                        // You may use self.inferior.as_mut().unwrap() to get a mutable reference
                        // to the Inferior object
                        match self.inferior.as_mut().unwrap().continue_run(None).unwrap() {
                            Status::Exited(status) => {
                                println!("Child exited (status {})", status);
                            },
                            Status::Signaled(signal) => {
                                println!("Child exited with {}", signal);
                            },
                            Status::Stopped(signal, curr_addr) => {
                                println!("Child stopped (signal {})", signal);
                                let func = DwarfData::get_function_from_addr(&self.debug_data, curr_addr);
                                let line = DwarfData::get_line_from_addr(&self.debug_data, curr_addr);
                                match (func, line) {
                                    (Some(func), Some(line)) => 
                                        println!("Stopped at {} {}", func, line),
                                    (_, _) => {
                                        println!("Fail to resolve stopping function and line")
                                    }
                                }
                            }
                        }
                    } else {
                        println!("Error starting subprocess");
                    }
                }
                DebuggerCommand::Continue => {
                    if self.inferior.is_none() {
                        println!("The process is not running");
                        continue;
                    }
                    let rip = self.inferior.as_ref().unwrap().get_previous_ins().unwrap();
                    if self.breakpoints.contains_key(&rip) {
                        println!(
                            "Previously Stopped at breakpoint: {}\n",
                            self.breakpoints.get(&rip).unwrap()
                        );
                        if !self
                            .inferior
                            .as_mut()
                            .unwrap()
                            .step_breakpoint(rip, self.breakpoints.get(&rip).unwrap().orig_byte)
                        {
                            println!("Failed to step by the breakpoint");
                            continue;
                        }
                    }
                    // let rip = self.inferior.as_ref().unwrap().get_previous_ins().unwrap();
                    match self.inferior.as_mut().unwrap().continue_run(None).unwrap() {
                        Status::Stopped(signal,  curr_addr) => {
                            println!("Child stopped (signal {})", signal);
                            let func = DwarfData::get_function_from_addr(&self.debug_data, curr_addr);
                            let line = DwarfData::get_line_from_addr(&self.debug_data, curr_addr);
                            match (func, line) {
                                (Some(func), Some(line)) => 
                                    println!("Stopped at {} {}", func, line),
                                (_, _) => {
                                        println!("Fail to resolve stopping function and line")
                                }
                            }
                        }
                        Status::Signaled(signal) => {
                            println!("Child exited with {}", signal);
                            self.inferior = None;
                        }
                        Status::Exited(exit_code) => {
                            println!("Child exited (status {})", exit_code);
                            self.inferior = None;
                        }
                    }
                    self.inferior.as_mut().unwrap().continue_run(None).unwrap();
                }
                DebuggerCommand::Quit => {
                    self.inferior.as_mut().unwrap().kill();
                    return;
                }
                DebuggerCommand::Backtrace => {
                    if self.inferior.is_some() {
                        self.inferior.as_ref().unwrap().print_backtrace(&self.debug_data).ok();
                    }   
                }
                DebuggerCommand::Break(args) => {
                    let breakpoint = match get_breakpoint_type(&args) {
                        BreakPointType::Raw(address) => parse_address(address).unwrap(),
                        // unable to get lines info in dwarf file, don't know why
                        BreakPointType::Line(line) => {
                            match self.debug_data.get_addr_for_line(None, line) {
                                Some(addr) => addr,
                                None => {
                                    println!("Failed to find the address of line {}", line);
                                    continue;
                                }
                            }
                        }
                        BreakPointType::Func(func) => {
                            match self.debug_data.get_addr_for_function(None, func) {
                                Some(addr) => addr,
                                None => {
                                    println!("Failed to find the address of function {}", func);
                                    continue;
                                }
                            }
                        }
                    };
                    
                    if !self.breakpoints.contains_key(&breakpoint) {
                        // add breakpoint to global Hashmap, without knowing the orig_byte
                        self.breakpoints.insert(
                            breakpoint,
                            BreakPoint::new(self.breakpoints.len() + 1, breakpoint),
                        );
                        // add breakpoint when process is stopped
                        if self.inferior.is_some() {
                            match self.inferior.as_mut().unwrap().install_breakpoints(breakpoint) {
                                Ok(orig_byte) => self
                                    .breakpoints
                                    .get_mut(&breakpoint)
                                    .unwrap()
                                    .set_byte(orig_byte),
                                Err(_) => {
                                    println!("Fail to insert breakpoint at {:#x}", breakpoint);
                                    continue;
                                }
                            }
                        }
                    }
                        println!(
                            "Set breakpoint {} at {}",
                            self.breakpoints.get(&breakpoint).as_ref().unwrap().id,
                            args
                        )
                    }
                                    
                }
            }
    }

    /// This function prompts the user to enter a command, and continues re-prompting until the user
    /// enters a valid command. It uses DebuggerCommand::from_tokens to do the command parsing.
    ///
    /// You don't need to read, understand, or modify this function.
    fn get_next_command(&mut self) -> DebuggerCommand {
        loop {
            // Print prompt and get next line of user input
            match self.readline.readline("(deet) ") {
                Err(ReadlineError::Interrupted) => {
                    // User pressed ctrl+c. We're going to ignore it
                    println!("Type \"quit\" to exit");
                }
                Err(ReadlineError::Eof) => {
                    // User pressed ctrl+d, which is the equivalent of "quit" for our purposes
                    return DebuggerCommand::Quit;
                }
                Err(err) => {
                    panic!("Unexpected I/O error: {:?}", err);
                }
                Ok(line) => {
                    if line.trim().len() == 0 {
                        continue;
                    }
                    self.readline.add_history_entry(line.as_str());
                    if let Err(err) = self.readline.save_history(&self.history_path) {
                        println!(
                            "Warning: failed to save history file at {}: {}",
                            self.history_path, err
                        );
                    }
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if let Some(cmd) = DebuggerCommand::from_tokens(&tokens) {
                        return cmd;
                    } else {
                        println!("Unrecognized command.");
                    }
                }
            }
        }
    }
}

fn parse_address(addr: &str) -> Option<usize> {
    let addr_without_0x = if addr.to_lowercase().starts_with("0x") {
        &addr[2..]
    } else {
        &addr
    };
    usize::from_str_radix(addr_without_0x, 16).ok()
}