use std::env;
use std::process;

use std::fs::File;
use std::io::{self, BufRead};

fn read_file_lines(filename: &String) -> Result<Vec<String>, io::Error> {
    let mut vec = Vec::new();
    let file = File::open(filename)?;
    for line in io::BufReader::new(file).lines() {
        let line_str = line?;
        vec.push(line_str);
    }
    return Ok(vec);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Too few arguments.");
        process::exit(1);
    }
    let filename = &args[1];
    // Your code here :)
    let lines = read_file_lines(filename).expect(format!("invalid filename: {}", filename).as_str());
    let mut count = 0;
    for (_, line) in lines.into_iter().enumerate() {
        count += line.split(' ').into_iter().count();
    }
    println!("total words in {}: {}", filename, count);
}
