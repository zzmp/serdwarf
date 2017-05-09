extern crate getopts;
extern crate memmap;
extern crate object;
extern crate dwarfdump;

mod serializable;

use object::Object;

use std::collections::BTreeMap;
use std::env;
use std::io;
use std::io::{ BufRead, BufReader, Write };
use std::fs;
use std::process;
use std::process::Command;

pub struct Flags {
    print_reserved: bool,
    print_summary: bool,
    omit_headers: bool,
    allow_char_str : bool,
    allow_void_str : bool,
    allow_basic_str : bool
}

fn print_usage(opts: &getopts::Options) -> ! {
    let usage = format!("Usage: {} [OPTION]... [FILE]...", env::args().next().unwrap());
    let description = "\
Print symbol, signature, and serializability for each public symbol in a
shared-object library. The library must have DWARF debugging symbols.";
    let brief = format!("{}\n{}", usage, description);
    write!(&mut io::stderr(), "{}", opts.usage(&brief)).ok();
    process::exit(1);
}

fn main() {
    let mut opts = getopts::Options::new();
    opts.optflag("C", "print-reserved", "print reserved symbols (_FOO)");
    opts.optflag("s", "summary", "print summary");
    opts.optflag("o", "omit-headers", "print multiple files' symbols with no separators");
    opts.optflag("c", "allow-char-str", "consider char* types to be serializable");
    opts.optflag("v", "allow-void-str", "consider void* types to be serializable");
    opts.optflag("p", "allow-basic-str", "consider pointers to basic types to be serializable");

    let matches = match opts.parse(env::args().skip(1)) {
        Ok(m) => m,
        Err(e) => {
            writeln!(&mut io::stderr(), "{:?}\n", e).ok();
            print_usage(&opts);
        }
    };
    if matches.free.is_empty() {
        print_usage(&opts);
    }

    let flags = Flags {
        print_reserved: matches.opt_present("C"),
        print_summary: matches.opt_present("s"),
        omit_headers: matches.opt_present("o"),
        allow_char_str: matches.opt_present("c"),
        allow_void_str: matches.opt_present("v"),
        allow_basic_str: matches.opt_present("p")
    };

    let mut counts = (0, 0);

    let mut first_file = true;
    for filepath in &matches.free {
        if matches.free.len() != 1 {
            if !flags.omit_headers && !flags.print_summary{
                if !first_file {
                    println!("");
                }
                println!("{}:", filepath);
            }

            if first_file {
                first_file = false;
            }
        }

        let file = fs::File::open(&filepath).expect("opening file");
        let file = memmap::Mmap::open(&file, memmap::Protection::Read).expect("mmapping file");
        let file = object::File::parse(unsafe { file.as_slice() }).expect("parsing file");

        // get the function symbols, signatures, and types from DWARF
        let dwarf_symbols: BTreeMap<String, Function> = dwarfdump::Symbols::from(file).functions.iter().map(|(k, v)| {
            (k.clone(), Function {
                signature: format!("{}", v),
                serializable: serializable::check(v, &flags)
            })
        }).collect();

        // get the global weak (W) and text (t, T) symbols from nm
        let (text_symbols, weak_symbols) = nm(filepath);

        // associate dwarf symbols with their addrs
        let mut dwarf_addrs: BTreeMap<&String, (&String, &Function)> = BTreeMap::new();
        for (symbol, addr) in text_symbols.iter() {
            if let Some(function) = dwarf_symbols.get(symbol) {
                dwarf_addrs.insert(addr, (symbol, function));
            }
        }

        // collect public symbols
        let mut symbols: BTreeMap<&String, (&String, &Function)> = BTreeMap::new();
        for (symbol, addr) in text_symbols.iter() {
            // skip versioned symbols except the default
            if symbol.contains("@") && !symbol.contains("@@") {
                continue;
            }

            if let Some(&(name, function)) = dwarf_addrs.get(addr) {
                symbols.insert(symbol, (&name, &function));
            }
        }
        for (symbol, addr) in weak_symbols.iter() {
            if let Some(&(name, function)) = dwarf_addrs.get(addr) {
                symbols.insert(symbol, (&name, &function));
            }
        }

        // print the results
        for (symbol, &(dwarf_name, function)) in symbols.iter() {
            // strip versioning from symbol
            let mut name = (*symbol).clone();
            match name.find("@@") {
                Some(i) => name.truncate(i),
                None => ()
            }

            // skip implementer-specific functions
            if !flags.print_reserved && name.starts_with("_") {
                continue
            }

            if flags.print_summary {
                counts = if function.serializable {
                    (counts.0 + 1, counts.1)
                } else {
                    (counts.0, counts.1 + 1)
                };
                continue
            }

            // replace original function name with symbol name
            let signature = if name == *dwarf_name {
                function.signature.clone()
            } else {
                function.signature.clone().replace(dwarf_name, name.as_str())
            };

            println!("{}\t{}\t{}", name, signature, function.serializable);
        }
    }

    if flags.print_summary {
        println!("total\tnot\tserializable\n{}\t{}\t{}", counts.0 + counts.1, counts.1, counts.0);
    }
}

struct Function {
    signature: String,
    serializable: bool
}

fn nm(filepath: &str) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!("nm {}", filepath))
        .output()
        .expect("Should call nm")
        .stdout;

    let reader = BufReader::new(stdout.as_slice());
    let mut text_symbols = BTreeMap::new();
    let mut weak_symbols = BTreeMap::new();

    for line in reader.lines() {
        let line = line.unwrap();
        let mut cols = line.split(" ");
        let addr = cols.next().expect("Should have nm addr");
        match cols.next().expect("Should have nm type") {
            "T" | "t" => {
                let symbol = cols.next().expect("Should have text symbol");
                let addr = String::from(addr);
                let symbol = String::from(symbol);
                text_symbols.insert(symbol, addr);
            },
            "W" => {
                let symbol = cols.next().expect("Should have text symbol");
                let addr = String::from(addr);
                let symbol = String::from(symbol);
                weak_symbols.insert(symbol, addr);
            },
            _ => ()
        };
    }

    (text_symbols, weak_symbols)
}
