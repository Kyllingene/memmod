# Memmod

This is a library to ease attaching to, reading from, and writing to a process. In the future, it will hopefully support more advanced procedures (such as injecting code, scanning, etc.).

Currently, it only supports Unix. I have access to a Windows dev machine, and plan on adding Windows support once I'm satisfied it's stable and ready for production.

## Attaching to a process

There are two ways to attach to a process: `Process::new` with a PID, and `Process::find[_strict]` with a name. Both of these methods require root priveleges, since they attach to the process immediately.

If you want to find a process by name, use `Process::find`. This will look for a process whose name includes the one provided. This can be dangerous: it will match `cat file.txt | grep <name>` if it comes across that first. To perform a strict equality check, use `Process::find_strict`. *Tip*: if you want to find the exact name of a process, try getting its PID, then running `cat /proc/<pid>/status`. The first line ends in the full name.

Here's an example of attaching to a process:
```rust
use memmod::Process;

fn main() {
    let proc = match Process::find("vscodium") {
        Ok(proc) => proc,
        Err(e) => {
            eprintln!("Failed to attach: {e}");
            return;
        }
    };

    // When the process gets dropped, it will detach. Detaching
    // fom a process automatically resumes it.
    //
    // To handle errrors when detaching, use `Process::detach`:
    if let Err(e) = proc.detach() {
        eprintln!("Failed to detach: {e}");
    }
}
```

## Reading/writing a process' memory

There are two ways to read a process' memory: using `Process::read_word[_offset]`, and using a `ProcessReader`.

The first method reads one word (an `isize`) from the process. The `_offset` variation adds the base address of the process to the address first. However, reading data this way can be clunky and annoying, so a `ProcessReader` type is also provided, which implements `Read` and handles individual bytes. You can create one using `Process::reader[_offset]`.

By default, the reader will advance through memory with each read; this can be disabled with the builder-pattern-like `ProcessReader::no_advance` method (this can also be called on an already-created reader). Afterwards, the reader will be "frozen" at its current address, and will always read from the same slice of memory.

Here's an example of reading from a process:
```rust
use memmod::Process;

fn main() {
    let mut proc = Process::new(1234)
        .expect("Failed to attach to the process");

    // Reads 4 bytes on a 32-bit machine,
    // and 8 bytes on a 64-bit machine.
    let word: isize = proc.read_word(0xdeadbeef)
        .expect("Failed to read word from process");

    // Readers contain a mutable reference to the
    // process, so they must be dropped after using.
    let data = {
        let mut data = Vec::new();

        let mut buf = [0u8; 8];
        let mut reader = proc.reader(0xbadf00d, 8);

        while buf[0] == 0 {
            // `buf` is the same size as the reader,
            // so we don't have to worry about how
            // much data was read.
            //
            // If `buf` had been 7 bytes, `reader` would
            // have read one word (two on 32-bit), and
            // discarded the last byte.
            reader.read_exact(&mut buf)
                .expect("Failed to read bytes from process");

            data.extend_from_slice(&buf);
        }
    
        // `reader` gets dropped and we can use `proc` again.
    };
}
```

To write to a process' memory, it's the exact same (substituting the proper methods, of course). There's a `ProcessWriter` struct that implements `Write` and has the same semantics as the reader. ***However,*** when you drop a `ProcessWriter`, it tries to flush the data you've written. ***This will cause a nasty panic if it fails!*** Always `flush` before dropping a writer.

## Following pointer chains

*If you don't know what pointer chains are, google `multi-level pointers cheat engine`.*

There's also a utility for following pointer chains, via `Process::pointer_chain`. This follows traditional semantics (deref the address, add an offset, repeat).

## Licensing and contribution

This is licensed under the very liberal MIT license. I would be only too glad if somebody took this project and expanded on it, even made money off of it.

As for contribution, any and all are welcome (but not necessarily accepted). Contributions are licensed under the MIT license unless otherwise specified. Contributions not under the MIT will probably be rejected (sorry, but multiple licenses for one project is too many).
