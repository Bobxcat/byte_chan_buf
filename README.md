# byte_chan_buf

This library contains a very simple implementation of an mpsc channel for raw byte streams, simply using a `Mutex<Vec<u8>>`

The important types are `ByteTx` and `ByteRx`, the sending and receiving sides of a byte stream. Creating a channel is done using the `byte_chan` function

## `ByteTx`

A `ByteTx` instance can be cloned, and implements `std::io::Write`. Each call to `write(buf)` will return `Ok(buf.len())` and write the whole buf in-order and uninterrupted to the stream, even when other writers are writing

## `ByteRx`

A `ByteRx` instance cannot be cloned, and implements `std::io::Read`. Even when all `ByteTx`s associated with this channel are dropped, all bytes that have been written will remain available to the `ByteRx`.