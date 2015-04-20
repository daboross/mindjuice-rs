//! mindjuice
//! =========
//!
//! Mindjuice is a simple and easy-to-use brainfuck interpreter!
//!
//! Usage
//! =====
//!
//! Mindjuice parses and runs brainfuck programs in two stages. First, it converts an input string into a `Vec<Instruction>`, then it can run that instruction vector to produce output.
//!
//! You can pass anything which implements `Iterator<char>` to the parse function.
//!
//! For example, parsing and executing static string:
//!
//! ```rust
//! extern crate mindjuice;
//!
//! use std::io;
//!
//! // parse_instructions will return Err() if there are any unmatched left or right brackets.
//! // Because we know this program doesn't have any unmatched brackets, using `.unwrap()` is fine.
//! let instructions = mindjuice::parse_instructions("++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.".chars()).unwrap();
//!
//! let mut buffer = Vec::new();
//!
//! // Execute the instructions!
//! mindjuice::execute_brainfuck(instructions, // Instructions vec
//!                             &mut buffer, // io::Write to send output to
//!                             io::empty(), // io::Read to get input from
//!                             30000000u64 // Maximum program iterations to run before returning
//!                             ).unwrap();
//!
//! assert_eq!(&buffer[..], b"Hello World!\n");
//! ```
//!
//! Note: Because the hello world program example doesn't use the `,` input command, we can use
//! `io::empty()` as the input. However, if we provided `io::empty()` for a program which did use
//! `,`, `execute_brainfuck()` would loop indefinitely waiting for input.

use std::fmt;
use std::iter;
use std::io;

const MEMORY_SIZE: usize = 32768usize;

#[derive(Debug)]
pub enum Error {
    /// A right bracket was found with no unmatched left brackets preceding it.
    UnbalancedRightBracket,
    /// The input ended before right brackets were found to match all left brackets.
    UnbalancedLeftBracket,
}

#[derive(Debug)]
pub enum ExecutionTerminationCondition {
    /// The maximum number of iterations was reached.
    MaximumIterationsReached,
    /// The program finished executing all instructions
    AllInstructionsFinished,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            &Error::UnbalancedRightBracket => {
                write!(formatter, "Expected matching `[` before `]`, found lone `]` first.")
            },
            &Error::UnbalancedLeftBracket => {
                write!(formatter, "Unbalanced `[`. Expected matching `]`, found end of file.")
            },
        }
    }
}

impl fmt::Display for ExecutionTerminationCondition {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            &ExecutionTerminationCondition::MaximumIterationsReached => {
                write!(formatter, "Maximum iterations reached.")
            },
            &ExecutionTerminationCondition::AllInstructionsFinished => {
                write!(formatter, "Finished normally.")
            },
        }
    }
}

#[derive(Debug)]
pub enum Instruction {
    /// Increment the memory pointer by one
    MoveRight,
    /// Decrement the memory pointer by one
    MoveLeft,
    /// Increment the memory value at the memory pointer by one
    Increment,
    /// Decrement the memory value at the memory pointer by one
    Decrement,
    /// Output the value of the current memory pointer as a char
    Output,
    /// Set the memory value at the current memory pointer to a char read from stdin.
    Input,
    /// This is the left side of a loop.
    /// If the memory value at the memory pointer is zero, set the next instruction to the
    /// contained value.
    JumpToLeft(usize),
    /// This is the right side of a loop.
    /// If the memory value at the memory pointer is non-zero, set the next instruction to the
    /// contained value.
    JumpToRight(usize),
}

pub fn parse_instructions<T>(input: T) -> Result<Vec<Instruction>, Error>
        where T: iter::Iterator<Item=char> {
    // Vec of opening jumps waiting for a closing jump to find
    // each u16 is a position in the instructions vec.
    let mut waiting_opening_jumps = Vec::new();
    // Output vec of instructions
    let mut instructions = Vec::new();

    // Main loop to parse
    for c in input {
        // Match on the next character
        let instruction = match c {
            '>' => Instruction::MoveRight,
            '<' => Instruction::MoveLeft,
            '+' => Instruction::Increment,
            '-' => Instruction::Decrement,
            '.' => Instruction::Output,
            ',' => Instruction::Input,
            '[' => {
                // instructions.len() is the position where the JumpToLeft is going to be inserted
                // in the instructions vec.
                waiting_opening_jumps.push(instructions.len());
                // This is a placeholder, this is guaranteed to be replaced when the
                // corresponding `]` is found, or an error will be thrown.
                Instruction::JumpToLeft(0usize)
            },
            ']' => {
                // This pops of the position of the last `[` found.
                match waiting_opening_jumps.pop() {
                    Some(left_jump) => {
                        // instructions.len() is the position where the JumpToRight is going to be
                        // inserted in the instructions vec.
                        instructions[left_jump] = Instruction::JumpToLeft(instructions.len());
                        // We can then just construct our JumpToRight using the position of the
                        // JumpToLeft in the instructions vec.
                        Instruction::JumpToRight(left_jump)
                    },
                    None => {
                        // We found a right bracket where no left brackets were waiting!
                        return Err(Error::UnbalancedRightBracket);
                    }
                }
            },
            // If we find a character we don't know about, just ignore it and continue.
            _ => continue,
        };

        instructions.push(instruction);
    }

    // Check to make sure there are no more opening left brackets which didn't find a matching
    // right bracket.
    if !waiting_opening_jumps.is_empty() {
        // If there are, throw an error.
        return Err(Error::UnbalancedLeftBracket);
    }

    // Return the instructions generated!
    return Ok(instructions);
}

pub fn execute_brainfuck<O, I>(instructions: Vec<Instruction>, mut output: O, mut input: I,
        maximum_iterations: u64) -> io::Result<ExecutionTerminationCondition>
        where O: io::Write, I: io::Read {

    // Program memory, max size is 2^15
    let mut memory = [0u8; MEMORY_SIZE];
    // Current position in memory
    let mut memory_position = 0usize;
    // Next instruction to run
    let mut next_instruction = 0usize;
    // Buffer used for reading input
    let mut read_buf = [0u8; 1];

    // u32::MAX as a limit to the number of iterations to run for a single program.
    for _ in 0..maximum_iterations {
        if next_instruction >= instructions.len() {
            // We've reached the end of the instructions
            return Ok(ExecutionTerminationCondition::AllInstructionsFinished);
        }
        match instructions[next_instruction] {
            Instruction::MoveRight => {
                // Increment the position by one, and make sure it still fits into memory_size
                memory_position += 1;
                memory_position %= MEMORY_SIZE;
            },
            Instruction::MoveLeft => {
                // Decrement the position by one, and make sure it still fits into memory_size
                memory_position -= 1;
                memory_position %= MEMORY_SIZE;
            },
            // Increment the memory value at the current position
            Instruction::Increment => memory[memory_position] += 1,
            // Decrement the memory value at the current position
            Instruction::Decrement => memory[memory_position] -= 1,
            // Writ the memory value at the current position to the given output
            Instruction::Output => try!(write!(&mut output, "{}", &(memory[memory_position] as char))),
            Instruction::Input => {
                // TODO: More efficient implementation of this perhaps?
                loop {
                    if try!(input.read(&mut read_buf)) >= 1 {
                        // If we've read at least 1 character, break.
                        break;
                    }
                }
                memory[memory_position] = read_buf[0];
            }
            Instruction::JumpToLeft(target_position) => {
                if memory[memory_position as usize] == 0 {
                    next_instruction = target_position;
                    continue; // this avoids the automatic incrementing of next_instruction below.
                }
            },
            Instruction::JumpToRight(target_position) => {
                if memory[memory_position as usize] != 0 {
                    next_instruction = target_position;
                    continue; // this avoids the automatic incrementing of next_instruction below.
                }
            },
        }
        next_instruction += 1;
    }

    // We reached the maximum iteration count
    return Ok(ExecutionTerminationCondition::MaximumIterationsReached);
}
