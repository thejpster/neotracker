# NeoTracker

A `no_std` ProTracker 4-channel MOD file reader.

You could use it to decode MOD files on your favourite microcontroller, and make
a tiny MOD tracker program.

## Components

* [`./neotracker`](./neotracker/) - a `#![no_std]` MOD file parser, with test cases
* [`./genpattern`](./genpattern/) - a program which uses the third-party [`modfile`](https://crates.io/crates/modfile) crate to parse a MOD file and print the contents as text.
  * This is used to generate test cases for the neotracker tests
